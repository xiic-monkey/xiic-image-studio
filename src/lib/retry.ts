import type { Capabilities, GenerateRequest } from "../types.ts";
import { buildGenerateRequest } from "./request.ts";

/** 任务卡里用于判断是否可重试的最小形状（不依赖 React 组件类型）。 */
export interface RetryableTask {
  status: string;
  prompt: string;
  providerId: string;
  n: number;
  size: string | null;
  quality: string | null;
  seed: number | null;
  refCount: number;
  sessionId: string | null;
}

/** 只有没跑成的任务才叫"重试"；成功的任务不挂重试按钮。 */
export const RETRYABLE_STATUS = new Set(["failed", "canceled", "completed_with_errors"]);

export function canRetry(task: RetryableTask): boolean {
  return (
    RETRYABLE_STATUS.has(task.status) &&
    task.refCount === 0 &&
    task.prompt.trim().length > 0 &&
    task.providerId.length > 0
  );
}

/** 重试必须复现原参数（含 seed），并保留原会话归属；旧数据无归属时回落到当前会话。 */
export function retryRequest(
  task: RetryableTask,
  currentSessionId: string,
  caps?: Capabilities,
): GenerateRequest {
  return buildGenerateRequest(
    {
      providerId: task.providerId,
      prompt: task.prompt,
      n: task.n,
      size: task.size ?? "",
      quality: task.quality ?? "",
      seed: task.seed === null ? "" : String(task.seed),
      refs: [],
      mask: null,
      sessionId: task.sessionId ?? currentSessionId,
    },
    caps,
  );
}
