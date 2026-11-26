import { useCallback } from "react";
import { api } from "../ipc";
import { errorMessage } from "../lib/error";
import type { Notify } from "../stores/toast";

/** 把已有图片复制到系统剪贴板，画廊与任务队列共用。解码在后端，前端只传路径。 */
export function useCopyImage(onNotice: Notify) {
  return useCallback(
    async (path: string) => {
      try {
        await api.imageCopy(path);
        onNotice("已复制图片到剪贴板", "success");
      } catch (err) {
        onNotice(`复制图片失败：${errorMessage(err)}`, "error");
      }
    },
    [onNotice],
  );
}
