import { ask } from "@tauri-apps/plugin-dialog";

/**
 * 统一的危险操作确认。浏览器原生 confirm 在暗色主题下是系统白框，
 * 按钮文案也不可控（"OK / Cancel"），与右键菜单、toast 的风格对不上。
 * 这里统一走 Tauri 原生对话框，标题、按钮文案都由应用决定。
 * 注意：需要 capabilities 里的 `dialog:allow-ask`（`dialog:default` 不含它）。
 */
export function confirmDelete(message: string, title = "确认删除"): Promise<boolean> {
  return ask(message, { title, kind: "warning", okLabel: "删除", cancelLabel: "取消" });
}
