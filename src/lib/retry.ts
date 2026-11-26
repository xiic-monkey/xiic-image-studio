import type { Capabilities, GenerateRequest, RefImage } from "../types.ts";
import { buildGenerateRequest } from "./request.ts";

/** 任务卡里用于判断是否可重试的最小形状（不依赖 React 组件类型）。 */
export interface RetryableTask {
  status: string;
  prompt: string;
  providerId: string;
  modelId?: string | null;
  n: number;
  size: string | null;
  quality: string | null;
  seed: number | null;
  refCount: number;
  /** 参考图快照路径；有值就能复现图生图，旧任务为空 */
  refPaths?: string[];
  sessionId: string | null;
}

/** 只有没跑成的任务才叫"重试"；成功的任务不挂重试按钮。 */
export const RETRYABLE_STATUS = new Set(["failed", "canceled", "completed_with_errors"]);

export function canRetry(task: RetryableTask): boolean {
  return (
    RETRYABLE_STATUS.has(task.status) &&
    // 图生图有参考图快照就能重试；旧任务 refCount>0 但没有快照，仍不提供入口
    (task.refCount === 0 || (task.refPaths?.length ?? 0) > 0) &&
    task.prompt.trim().length > 0 &&
    task.providerId.length > 0
  );
}

/** 重试必须复现原参数（含 seed）与参考图，并保留原会话归属；旧数据无归属时回落到当前会话。 */
export function retryRequest(
  task: RetryableTask,
  currentSessionId: string,
  caps?: Capabilities,
  refs: RefImage[] = [],
): GenerateRequest {
  return buildGenerateRequest(
    {
      providerId: task.providerId,
      modelId: task.modelId,
      prompt: task.prompt,
      n: task.n,
      size: task.size ?? "",
      quality: task.quality ?? "",
      seed: task.seed === null ? "" : String(task.seed),
      refs,
      mask: null,
      sessionId: task.sessionId ?? currentSessionId,
    },
    caps,
  );
}
