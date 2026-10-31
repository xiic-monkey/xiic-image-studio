/**
 * 工作台提交语义。抽成纯函数是因为这三条规则过去全靠组件里的
 * `split("\n").length > 1` 隐式判断，改一行文案就会悄悄改变"提交几个任务"。
 */

export interface Submission {
  prompt: string;
  n: number;
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

/** 提交中途失败后，输入框里该剩下的文本（失败那行及其后续，改完可接着发）。 */
export function remainingText(submissions: Submission[], submitted: number): string {
  return submissions.slice(submitted).map((s) => s.prompt).join("\n");
}

/** `next === undefined` 表示这次回填没涉及提示词，保持现状；`""` 是显式清空。 */
export function pickPrefillPrompt(current: string, next?: string): string {
  return next === undefined ? current : next;
}
