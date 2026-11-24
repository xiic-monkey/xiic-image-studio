import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../ipc";
import { parseBatch } from "../lib/batch-parse.ts";
import { isSeedEnabled } from "../lib/capabilities.ts";
import type { DraftForm } from "../lib/draft.ts";
import { errorMessage } from "../lib/error";
import { buildGenerateRequest } from "../lib/request.ts";
import { canRetry, retryRequest } from "../lib/retry.ts";
import {
  buildScanSubmissions,
  buildSubmissions,
  parseSeedBase,
  remainingText,
} from "../lib/submission.ts";
import type { Capabilities, GenerateRequest, ProviderListItem, RefImage, TaskUpsert, TaskView } from "../types";

export interface SubmissionContext {
  providers: ProviderListItem[];
  providerId: string;
  /** 当前选中模型的能力（由 WorkbenchView 按选中模型解析，不再取 provider.capabilities） */
  caps: Capabilities | undefined;
  form: DraftForm;
  refs: RefImage[];
  mask: RefImage | null;
  sessionId: string;
  /** 草稿会话是否就绪；未就绪时 sessionId 为空，不能提交（否则 session_id 落 NULL） */
  ready: boolean;
  upsert: (p: TaskUpsert) => void;
  patch: (p: Partial<DraftForm>) => void;
  clearRefs: () => void;
  setNotice: (msg: string | null) => void;
}

/** 三条提交路径（生成 / 批量导入 / 一键重试）共用同一个请求组装点与乐观任务卡。 */
export function useSubmissions(ctx: SubmissionContext) {
  const [submitting, setSubmitting] = useState(false);

  const optimistic = (taskId: string, req: GenerateRequest, prompt: string) => {
    // 乐观卡片直接把重试要用的字段带全：等 3 秒轮询补齐太脆弱，
    // 任何一次“指纹相同就跳过”的判断都会让卡片长期缺 provider/model。
    const card: TaskUpsert = {
      id: taskId,
      status: "running",
      done: 0,
      total: req.n,
      errors: [],
      images: [],
      createdAt: new Date().toISOString(),
      prompt,
      providerId: req.provider_id,
      size: req.size ?? null,
      quality: req.quality ?? null,
      n: req.n,
      refCount: req.refs.length,
      seed: req.seed ?? null,
      sessionId: req.session_id ?? null,
      modelId: req.model_id ?? null,
      modelName: null,
    };
    ctx.upsert(card);
  };

  const submit = async () => {
    if (!ctx.ready || !ctx.sessionId) {
      ctx.setNotice("正在初始化，请稍候");
      return;
    }
    const provider = ctx.providers.find((p) => p.id === ctx.providerId);
    if (!provider) {
      ctx.setNotice("请先在「供应商」页添加并保存供应商");
      return;
    }
    const { form } = ctx;
    const caps = ctx.caps ?? provider.capabilities; // 旧数据兜底：未解析到模型时用 provider 能力
    // 扫描模式按能力表判定：模型不支持 seed 时明确拒绝，
    // 不能静默按「无种子」提交一串重复任务（那是白花钱）。
    if (form.scanMode && !isSeedEnabled(caps)) {
      ctx.setNotice("当前模型不支持种子，无法使用种子扫描");
      return;
    }
    const submissions = form.scanMode
      ? buildScanSubmissions(form.prompt, parseSeedBase(form.seed), form.scanCount)
      : buildSubmissions(form.prompt, form.batchMode, form.count);
    if (submissions.length === 0) {
      ctx.setNotice("请输入提示词");
      return;
    }
    setSubmitting(true);
    ctx.setNotice(null);
    try {
      let submitted = 0;
      let failMsg: string | null = null;

      for (const s of submissions) {
        try {
          const req = buildGenerateRequest(
            {
              providerId: provider.id,
              modelId: form.modelId,
              prompt: s.prompt,
              n: s.n,
              size: form.size,
              quality: form.quality,
              // 扫描模式下每条任务带自己的种子；其余场景沿用表单里的种子
              seed: s.seed !== undefined ? String(s.seed) : form.seed,
              refs: ctx.refs,
              mask: ctx.mask,
              sessionId: ctx.sessionId,
            },
            caps,
          );
          const taskId = await api.generateSubmit(req);
          optimistic(taskId, req, s.prompt);
          submitted += 1;
        } catch (e) {
          failMsg = errorMessage(e);
          break;
        }
      }

      // 界面状态必须和"实际提交了什么"严格对齐：
      // 已提交的行从输入框移除，失败那行连同它后面的原样留着（用户改完可以接着发）；
      // 参考图只在全部成功时才清——部分失败时，剩下的行还要继续用它。
      if (failMsg === null) {
        ctx.patch({ prompt: "" });
        ctx.clearRefs();
        const multiline = form.prompt.trim().includes("\n");
        ctx.setNotice(
          submissions.length > 1
            ? `已提交 ${submitted} 个任务`
            : multiline
              ? "已提交 1 个任务（整段文本按一条提示词处理；要每行一个任务请打开「按行批量」）"
              : null,
        );
      } else {
        // 扫描模式下每条任务的提示词都相同，remainingText 会拼出 N 份重复文本。
        // 失败时保留原始提示词即可——重扫一遍比看一堆重复行有用。
        ctx.patch({ prompt: form.scanMode ? form.prompt : remainingText(submissions, submitted) });
        ctx.setNotice(`已提交 ${submitted}/${submissions.length} 条，第 ${submitted + 1} 条提交失败：${failMsg}`);
      }
    } catch (e) {
      ctx.setNotice(errorMessage(e));
    } finally {
      setSubmitting(false);
    }
  };

  const importBatch = async () => {
    if (!ctx.ready || !ctx.sessionId) {
      ctx.setNotice("正在初始化，请稍候");
      return;
    }
    const provider = ctx.providers.find((p) => p.id === ctx.providerId);
    if (!provider) {
      ctx.setNotice("请先在「供应商」页添加并保存供应商");
      return;
    }
    const picked = await open({
      multiple: false,
      filters: [{ name: "批量", extensions: ["jsonl", "csv", "txt"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    setSubmitting(true);
    try {
      const text = await api.readTextFile(picked);
      const items = parseBatch(text, picked);
      if (items.length === 0) {
        ctx.setNotice("文件为空或无有效行");
        return;
      }
      let ok = 0;
      for (const it of items) {
        try {
          const req = buildGenerateRequest(
            {
              providerId: provider.id,
              modelId: ctx.form.modelId,
              prompt: it.prompt,
              n: it.n ? Number(it.n) : 1,
              size: it.size || ctx.form.size,
              quality: it.quality ?? ctx.form.quality,
              seed: it.seed ?? "",
              refs: [],
              mask: null,
              sessionId: ctx.sessionId,
            },
            ctx.caps ?? provider.capabilities,
          );
          const taskId = await api.generateSubmit(req);
          optimistic(taskId, req, it.prompt);
          ok += 1;
        } catch (e) {
          ctx.setNotice(`批量提交在第 ${ok + 1} 行失败：${errorMessage(e)}（已提交 ${ok} 条）`);
          break;
        }
      }
      if (ok === items.length) ctx.setNotice(`已提交 ${ok} 个批量任务`);
    } catch (e) {
      ctx.setNotice(`读取/解析失败：${errorMessage(e)}`);
    } finally {
      setSubmitting(false);
    }
  };

  const retry = async (t: TaskView) => {
    if (!ctx.ready || !ctx.sessionId) {
      ctx.setNotice("正在初始化，请稍候");
      return;
    }
    if (!canRetry(t)) {
      ctx.setNotice("该任务缺少可重试的参数（供应商或提示词为空）");
      return;
    }
    try {
      // 参考图本体没入库，只有 refCount === 0 的任务才会走到这里。
      // 不传 caps：重试要按任务原模型复现参数，由后端用原 modelId 的能力裁剪；
      // 拿当前选中模型的能力会把 size/quality 换成另一张表的值，甚至丢掉 seed。
      const req = retryRequest(t, ctx.sessionId);
      const taskId = await api.generateSubmit(req);
      optimistic(taskId, req, t.prompt);
      ctx.setNotice("已按原参数重新提交");
    } catch (e) {
      ctx.setNotice(`重试失败：${errorMessage(e)}`);
    }
  };

  return { submit, importBatch, retry, submitting };
}
