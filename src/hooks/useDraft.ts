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
 * 草稿表单 + 自动保存。
 * 用「已落库内容的指纹」而不是 skipSave 单布尔判断要不要写：布尔会被任何一次
 * 无关的重渲染消费掉，导致切会话时把上一个会话的草稿写进新会话。
 */
export function useDraft(sessionId: string, sessions: SessionItem[]) {
  const [form, setForm] = useState<DraftForm>(EMPTY_DRAFT);
  const sessionsRef = useRef(sessions);
  sessionsRef.current = sessions;
  const formRef = useRef(form);
  formRef.current = form;

  const baseline = useRef(draftSignature(EMPTY_DRAFT));
  const pending = useRef<{ sessionId: string; form: DraftForm } | null>(null);
  const appliedFor = useRef<string | null>(null);

  const flush = useCallback(() => {
    const p = pending.current;
    if (!p) return;
    pending.current = null;
    baseline.current = draftSignature(p.form);
    // 会话已删时不能写草稿：后端 session_save 对不存在的 id 会 INSERT，
    // 会把刚删掉的会话复活。
    if (!sessionsRef.current.some((s) => s.id === p.sessionId)) return;
    void api.sessionSave({ id: p.sessionId, draft: draftFromForm(p.form) }).catch(() => {});
  }, []);

  // 卸载（切 tab / 关窗口）时把最后一次编辑落库：debounce 的 timer 会被 cleanup 清掉，
  // 不 flush 就会丢最后 800ms 内敲的字。
  useEffect(() => () => flush(), [flush]);

  useEffect(() => {
    if (!sessionId) return;
    if (appliedFor.current !== sessionId) {
      appliedFor.current = sessionId;
      flush(); // 切会话前先把上个会话的待写草稿落库
      const s = sessionsRef.current.find((x) => x.id === sessionId);
      // providerId 沿用当前值：旧草稿没存供应商时不把用户的选择冲掉
      const next = formFromDraft(s?.draft, { ...EMPTY_DRAFT, providerId: formRef.current.providerId });
      pending.current = null;
      baseline.current = draftSignature(next);
      setForm(next);
      return; // 本轮 form 还是旧会话的，绝不能拿它去写新会话
    }
    if (draftSignature(form) === baseline.current) return;
    pending.current = { sessionId, form };
    const timer = setTimeout(flush, 800);
    return () => clearTimeout(timer);
  }, [sessionId, form, flush]);

  const patch = useCallback((p: Partial<DraftForm>) => setForm((prev) => ({ ...prev, ...p })), []);

  return { form, setForm, patch, flush };
}
