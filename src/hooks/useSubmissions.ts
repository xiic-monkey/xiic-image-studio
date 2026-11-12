import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../ipc";
import { parseBatch } from "../lib/batch-parse.ts";
import type { DraftForm } from "../lib/draft.ts";
import { buildGenerateRequest } from "../lib/request.ts";
import { canRetry, retryRequest } from "../lib/retry.ts";
import { buildSubmissions, remainingText } from "../lib/submission.ts";
import type { ProviderListItem, RefImage, TaskUpsert, TaskView } from "../types";

export interface SubmissionContext {
  providers: ProviderListItem[];
  providerId: string;
  form: DraftForm;
  refs: RefImage[];
  mask: RefImage | null;
  sessionId: string;
  upsert: (p: TaskUpsert) => void;
  patch: (p: Partial<DraftForm>) => void;
  clearRefs: () => void;
  setNotice: (msg: string | null) => void;
}

/** 三条提交路径（生成 / 批量导入 / 一键重试）共用同一个请求组装点与乐观任务卡。 */
export function useSubmissions(ctx: SubmissionContext) {
  const [submitting, setSubmitting] = useState(false);

  const optimistic = (taskId: string, prompt: string, n: number, seed: number | null, sessionId: string | null) => {
    const card: TaskUpsert = {
      id: taskId,
      status: "running",
      done: 0,
      total: n,
      errors: [],
      images: [],
      createdAt: new Date().toISOString(),
      prompt,
      seed,
      sessionId,
    };
    ctx.upsert(card);
  };

  const submit = async () => {
    const provider = ctx.providers.find((p) => p.id === ctx.providerId);
    if (!provider) {
      ctx.setNotice("请先在「供应商」页添加并保存供应商");
      return;
    }
    const { form } = ctx;
    const submissions = buildSubmissions(form.prompt, form.batchMode, form.count);
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
              prompt: s.prompt,
              n: s.n,
              size: form.size,
              quality: form.quality,
              seed: form.seed,
              refs: ctx.refs,
              mask: ctx.mask,
              sessionId: ctx.sessionId,
            },
            provider.capabilities,
          );
          const taskId = await api.generateSubmit(req);
          optimistic(taskId, s.prompt, req.n, req.seed ?? null, req.session_id ?? null);
          submitted += 1;
        } catch (e) {
          failMsg = String(e);
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
        ctx.patch({ prompt: remainingText(submissions, submitted) });
        ctx.setNotice(`已提交 ${submitted}/${submissions.length} 条，第 ${submitted + 1} 条提交失败：${failMsg}`);
      }
    } catch (e) {
      ctx.setNotice(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  const importBatch = async () => {
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
              prompt: it.prompt,
              n: it.n ? Number(it.n) : 1,
              size: it.size || ctx.form.size,
              quality: it.quality ?? ctx.form.quality,
              seed: it.seed ?? "",
              refs: [],
              mask: null,
              sessionId: ctx.sessionId,
            },
            provider.capabilities,
          );
          const taskId = await api.generateSubmit(req);
          optimistic(taskId, it.prompt, req.n, req.seed ?? null, req.session_id ?? null);
          ok += 1;
        } catch (e) {
          ctx.setNotice(`批量提交在第 ${ok + 1} 行失败：${String(e)}（已提交 ${ok} 条）`);
          break;
        }
      }
      if (ok === items.length) ctx.setNotice(`已提交 ${ok} 个批量任务`);
    } catch (e) {
      ctx.setNotice(`读取/解析失败：${String(e)}`);
    } finally {
      setSubmitting(false);
    }
  };

  const retry = async (t: TaskView) => {
    if (!canRetry(t)) {
      ctx.setNotice("该任务缺少可重试的参数（供应商或提示词为空）");
      return;
    }
    try {
      // 参考图本体没入库，只有 refCount === 0 的任务才会走到这里。
      const caps = ctx.providers.find((p) => p.id === t.providerId)?.capabilities;
      const req = retryRequest(t, ctx.sessionId, caps);
      const taskId = await api.generateSubmit(req);
      optimistic(taskId, t.prompt, req.n, req.seed ?? null, req.session_id ?? null);
      ctx.setNotice("已按原参数重新提交");
    } catch (e) {
      ctx.setNotice(`重试失败：${String(e)}`);
    }
  };

  return { submit, importBatch, retry, submitting };
}
