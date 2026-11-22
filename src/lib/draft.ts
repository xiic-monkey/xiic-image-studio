import type { SessionItem } from "../types.ts";

/** 草稿只存可文本化的表单值：参考图 base64 太大，会话 id 由 sessionId 派生。 */
export interface DraftForm {
  providerId: string;
  /** 当前选中的模型 id（供应商×模型关联） */
  modelId: string;
  prompt: string;
  size: string;
  quality: string;
  count: number;
  seed: string;
  batchMode: boolean;
  /** 种子扫描：把一条提示词按连续 seed 拆成多个任务 */
  scanMode: boolean;
  /** 扫描产出的张数（= 任务数，每任务 1 张） */
  scanCount: number;
}

export const EMPTY_DRAFT: DraftForm = {
  providerId: "",
  modelId: "",
  prompt: "",
  size: "1024x1024",
  quality: "auto",
  count: 1,
  seed: "",
  batchMode: false,
  scanMode: false,
  scanCount: 4,
};

export type SessionDraft = SessionItem["draft"];

export function draftFromForm(form: DraftForm): SessionDraft {
  return {
    providerId: form.providerId,
    modelId: form.modelId,
    prompt: form.prompt,
    size: form.size,
    quality: form.quality,
    count: form.count,
    seed: form.seed,
    batchMode: form.batchMode,
    scanMode: form.scanMode,
    scanCount: form.scanCount,
  };
}

export function formFromDraft(draft: SessionDraft | undefined, fallback: DraftForm = EMPTY_DRAFT): DraftForm {
  return {
    providerId: draft?.providerId ?? fallback.providerId,
    modelId: draft?.modelId ?? fallback.modelId,
    prompt: draft?.prompt ?? "",
    size: draft?.size ?? fallback.size,
    quality: draft?.quality ?? fallback.quality,
    count: draft?.count ?? fallback.count,
    seed: draft?.seed ?? "",
    batchMode: draft?.batchMode ?? false,
    scanMode: draft?.scanMode ?? false,
    scanCount: draft?.scanCount ?? fallback.scanCount,
  };
}

/** 内容指纹：灌草稿引起的对象重建不该触发一次无意义的写库。 */
export function draftSignature(form: DraftForm): string {
  return [
    form.providerId,
    form.modelId,
    form.prompt,
    form.size,
    form.quality,
    String(form.count),
    form.seed,
    form.batchMode ? "1" : "0",
    form.scanMode ? "1" : "0",
    String(form.scanCount),
  ].join("|");
}
