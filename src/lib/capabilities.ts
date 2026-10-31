import type { Capabilities } from "../types.ts";

/** 当前尺寸是否被能力表允许；不允许就回落到表内第一个（表空 → undefined）。 */
export function normalizeSize(caps: Capabilities, current: string): string | undefined {
  if (caps.sizes.includes(current)) return current;
  return caps.sizes[0];
}

/** 质量同理；该协议/模型不支持质量（表空）→ undefined。 */
export function normalizeQuality(caps: Capabilities, current: string): string | undefined {
  if (caps.quality.includes(current)) return current;
  return caps.quality[0];
}

/** 种子是否可用（不支持时前端置灰并说明原因，而不是静默忽略）。 */
export function isSeedEnabled(caps: Capabilities): boolean {
  return caps.seed;
}
