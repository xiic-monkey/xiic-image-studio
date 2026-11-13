/**
 * Tauri 的 IPC 桥（`window.__TAURI_INTERNALS__`）由应用外壳注入。
 * 只跑 vite 前端、在浏览器里打开（预览模式）时没有它，所有 `invoke` 都会抛
 * "Cannot read properties of undefined (reading 'invoke')"——那是给开发者看的
 * 原始报错，糊在界面上会让人以为程序崩了。这里统一换成一句能解释现状的话。
 */

/** 预览模式下的统一提示。 */
export const NOT_IN_APP_MESSAGE =
  "当前是浏览器预览，没有应用外壳，无法访问本地数据库与后端。请用 npx tauri dev 或打包后的应用打开。";

/** 是否在 Tauri 外壳里（有没有 IPC 桥）。 */
export function hasTauriBridge(): boolean {
  return (
    typeof window !== "undefined" &&
    Boolean((window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
  );
}
