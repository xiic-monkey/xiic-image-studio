import { useCallback } from "react";
import { api } from "../ipc";
import { errorMessage } from "../lib/error";

/** 把已有图片复制到系统剪贴板，画廊与任务队列共用。解码在后端，前端只传路径。 */
export function useCopyImage(onNotice: (msg: string) => void) {
  return useCallback(
    async (path: string) => {
      try {
        await api.imageCopy(path);
        onNotice("已复制图片到剪贴板");
      } catch (err) {
        onNotice(`复制图片失败：${errorMessage(err)}`);
      }
    },
    [onNotice],
  );
}
