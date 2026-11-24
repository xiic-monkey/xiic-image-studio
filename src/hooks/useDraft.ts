import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../ipc";
import {
  draftFromForm,
  draftSignature,
  formFromDraft,
  EMPTY_DRAFT,
  type DraftForm,
} from "../lib/draft.ts";
import type { SessionItem } from "../types";

/**
 * 单会话草稿 + 自动保存。
 * 启动取第一个会话（没有就建「默认会话」）作为唯一草稿槽，只灌一次；
 * ready 之前 sessionId 为空，提交层据此拦截，避免任务落库时 session_id 为 NULL
 * 被后端的遗留任务判定当成无头 CLI 任务。
 */
export function useDraft() {
  const [sessionId, setSessionId] = useState("");
  const [ready, setReady] = useState(false);
  const [form, setForm] = useState<DraftForm>(EMPTY_DRAFT);

  // StrictMode 会把挂载 effect 跑两遍；booted 保证只加载一次
  const booted = useRef(false);
  const formRef = useRef(form);
  formRef.current = form;
  const baseline = useRef(draftSignature(EMPTY_DRAFT));
  const pending = useRef<{ sessionId: string; form: DraftForm } | null>(null);

  const flush = useCallback(() => {
    const p = pending.current;
    if (!p) return;
    pending.current = null;
    baseline.current = draftSignature(p.form);
    void api.sessionSave({ id: p.sessionId, draft: draftFromForm(p.form) }).catch(() => {});
  }, []);

  // 卸载（切 tab / 关窗口）时把 debounce 窗口内的最后一次编辑落库
  useEffect(() => () => flush(), [flush]);

  // 启动：取第一个会话（或新建），灌一次草稿
  useEffect(() => {
    if (booted.current) return;
    booted.current = true;
    (async () => {
      let list: SessionItem[] = [];
      try {
        list = await api.sessionList();
      } catch {
        // 预览模式（无 Tauri 桥）或读库失败：保持空表单，下面仍会置 ready 让界面可用
      }
      let id = list[0]?.id;
      if (!id) {
        try {
          id = await api.sessionSave({ name: "默认会话" });
        } catch {
          // 写库失败：sessionId 留空，提交层会提示，不带着 NULL 落库
        }
      }
      if (id) {
        setSessionId(id);
        const saved = list.find((s) => s.id === id);
        const current = formRef.current;
        const loaded = formFromDraft(saved?.draft, {
          ...EMPTY_DRAFT,
          providerId: current.providerId,
        });
        // 加载期间用户已填了提示词就不覆盖（画廊回填由 ready 门控，不会在这里抢）
        const next = current.prompt ? { ...loaded, prompt: current.prompt } : loaded;
        baseline.current = draftSignature(next);
        setForm(next);
      }
      setReady(true);
    })();
  }, []);

  // 指纹变化才写库，避免回填/重渲染造成无意义保存
  useEffect(() => {
    if (!ready || !sessionId) return;
    if (draftSignature(form) === baseline.current) return;
    pending.current = { sessionId, form };
    const timer = setTimeout(flush, 800);
    return () => clearTimeout(timer);
  }, [form, sessionId, ready, flush]);

  const patch = useCallback((p: Partial<DraftForm>) => setForm((prev) => ({ ...prev, ...p })), []);

  return { form, setForm, patch, sessionId, ready };
}
