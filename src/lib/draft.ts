import type { SessionItem } from "../types.ts";

/** 草稿只存可文本化的表单值：参考图 base64 太大，会话 id 由 sessionId 派生。 */
export interface DraftForm {
  providerId: string;
  prompt: string;
  size: string;
  quality: string;
  count: number;
  seed: string;
  batchMode: boolean;
}

export const EMPTY_DRAFT: DraftForm = {
  providerId: "",
  prompt: "",
  size: "1024x1024",
  quality: "auto",
  count: 1,
  seed: "",
  batchMode: false,
};

export type SessionDraft = SessionItem["draft"];

export function draftFromForm(form: DraftForm): SessionDraft {
  return {
    providerId: form.providerId,
    prompt: form.prompt,
    size: form.size,
    quality: form.quality,
    count: form.count,
    seed: form.seed,
    batchMode: form.batchMode,
  };
}

export function formFromDraft(draft: SessionDraft | undefined, fallback: DraftForm = EMPTY_DRAFT): DraftForm {
  return {
    providerId: draft?.providerId ?? fallback.providerId,
    prompt: draft?.prompt ?? "",
    size: draft?.size ?? fallback.size,
    quality: draft?.quality ?? fallback.quality,
    count: draft?.count ?? fallback.count,
    seed: draft?.seed ?? "",
    batchMode: draft?.batchMode ?? false,
  };
}

/** 内容指纹：灌草稿引起的对象重建不该触发一次无意义的写库。 */
export function draftSignature(form: DraftForm): string {
  return [
    form.providerId,
    form.prompt,
    form.size,
    form.quality,
    String(form.count),
    form.seed,
    form.batchMode ? "1" : "0",
  ].join("|");
}
