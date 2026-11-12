import type { Capabilities, GenerateRequest, RefImage } from "../types.ts";
import { isSeedEnabled, normalizeQuality, normalizeSize } from "./capabilities.ts";

/** 工作台表单 → 提交请求。唯一组装点，批量导入与重试都复用它。 */
export interface FormInput {
  providerId: string;
  prompt: string;
  n: number;
  size: string;
  quality: string;
  seed: string;
  refs: RefImage[];
  mask: RefImage | null;
  sessionId?: string;
}

export const MAX_N = 8;

export function buildGenerateRequest(input: FormInput, caps?: Capabilities): GenerateRequest {
  const seed = caps && !isSeedEnabled(caps) ? undefined : parseSeed(input.seed);
  const request: GenerateRequest = {
    provider_id: input.providerId,
    prompt: input.prompt,
    n: Math.min(MAX_N, Math.max(1, Math.trunc(input.n) || 1)),
    refs: input.refs,
  };
  const size = caps ? normalizeSize(caps, input.size) : input.size || undefined;
  const quality = caps ? normalizeQuality(caps, input.quality) : input.quality || undefined;
  if (size) request.size = size;
  if (quality) request.quality = quality;
  if (seed !== undefined) request.seed = seed;
  if (input.mask && (!caps || caps.mask)) request.mask = input.mask;
  if (input.sessionId) request.session_id = input.sessionId;
  return request;
}

function parseSeed(raw: string): number | undefined {
  const trimmed = raw.trim();
  if (!trimmed) return undefined;
  const value = Number(trimmed);
  return Number.isSafeInteger(value) && value >= 0 ? value : undefined;
}
