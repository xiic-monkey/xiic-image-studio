/**
 * 统一的错误文案提取。
 *
 * 为什么不用 `String(e)`：Error 实例的 String() 会带上 "Error: " 前缀，
 * 直接渲染到界面上像日志而不像提示。而 Tauri 命令失败时 reject 的值又是
 * 字符串，Tauri 官方文档也建议用 `String(err)` 兜底——两种形态都得接住，
 * 所以这里先判 Error 再兜底。
 */
export function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
