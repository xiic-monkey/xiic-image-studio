/**
 * 工作台提交语义。抽成纯函数是因为这三条规则过去全靠组件里的
 * `split("\n").length > 1` 隐式判断，改一行文案就会悄悄改变"提交几个任务"。
 */

export interface Submission {
  prompt: string;
  n: number;
  /** 扫描模式下每条任务带自己的种子；普通提交为 undefined（沿用表单里的 seed）。 */
  seed?: number;
}

/**
 * `batchMode` 必须由用户显式打开：一段跨行的完整提示词（长描述、模板追加）
 * 过去会被拆成 N 个互不相关的任务，并把用户选的「数量」静默改成 1。
 * 批量模式下每行 1 张是协议限制之外的产品选择（每行内容不同，n 张同 prompt 没意义）。
 */
export function buildSubmissions(text: string, batchMode: boolean, count: number): Submission[] {
  const trimmed = text.trim();
  if (!trimmed) return [];
  if (!batchMode) return [{ prompt: trimmed, n: count }];
  return trimmed
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean)
    .map((prompt) => ({ prompt, n: 1 }));
}

/** 种子扫描一次最多产出多少张。再多就分批提交，别把上游打爆。 */
export const MAX_SCAN = 32;

/**
 * 扫描的起始种子。留空或非法时从 0 开始——扫描必须可复现，
 * 不能悄悄换成随机基准（那样用户没法回头复现同一批）。
 */
export function parseSeedBase(raw: string): number {
  const trimmed = raw.trim();
  if (!trimmed) return 0;
  const value = Number(trimmed);
  return Number.isSafeInteger(value) && value >= 0 ? value : 0;
}

/**
 * 种子扫描：同一条提示词，按连续种子拆成多个任务。
 *
 * 每条任务 n=1——同 seed 要 n 张，多数上游会返回几乎一样的图，
 * 扫描要的是「同一个提示词在不同 seed 下的差异」，重复出图没意义。
 */
export function buildScanSubmissions(text: string, baseSeed: number, count: number): Submission[] {
  const trimmed = text.trim();
  if (!trimmed) return [];
  const n = Math.min(MAX_SCAN, Math.max(1, Math.trunc(count) || 1));
  return Array.from({ length: n }, (_, i) => ({
    prompt: trimmed,
    n: 1,
    seed: baseSeed + i,
  }));
}

/** 提交中途失败后，输入框里该剩下的文本（失败那行及其后续，改完可接着发）。 */
export function remainingText(submissions: Submission[], submitted: number): string {
  return submissions.slice(submitted).map((s) => s.prompt).join("\n");
}

/** `next === undefined` 表示这次回填没涉及提示词，保持现状；`""` 是显式清空。 */
export function pickPrefillPrompt(current: string, next?: string): string {
  return next === undefined ? current : next;
}
