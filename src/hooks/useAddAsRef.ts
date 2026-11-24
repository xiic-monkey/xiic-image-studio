import { useCallback } from "react";
import { api } from "../ipc";
import { errorMessage } from "../lib/error";
import { dataUrlToRef, MAX_REFS } from "../lib/ref-image";
import { useAssets } from "../stores/assets";
import type { RefImage } from "../types";

/**
 * 读原图 → 转 RefImage → 去重后加入全局参考图。画廊与任务队列共用，
 * 返回实际新增张数；重复与满额各自提示，调用方只决定后续动作。
 */
export function useAddAsRef(onNotice: (msg: string) => void) {
  const addRefs = useAssets((s) => s.addRefs);
  return useCallback(
    async (items: { path: string; id?: string }[]): Promise<{ added: number; duplicates: number }> => {
      const refs: RefImage[] = [];
      for (const { path, id } of items) {
        try {
          const dataUrl = await api.imageReadB64(path);
          const name = path.split("/").pop() || "image";
          refs.push(dataUrlToRef(dataUrl, name, id));
        } catch (err) {
          onNotice(errorMessage(err));
          return { added: 0, duplicates: 0 };
        }
      }
      if (refs.length === 0) return { added: 0, duplicates: 0 };
      const result = addRefs(refs);
      const { added, duplicates } = result;
      if (duplicates > 0) {
        onNotice(added > 0 ? `已跳过 ${duplicates} 张重复图片` : "这张图已在参考图中");
      }
      if (added === 0) {
        if (duplicates === 0) onNotice(`参考图已满，最多 ${MAX_REFS} 张`);
      } else if (added < refs.length - duplicates) {
        onNotice(`只加入了 ${added} 张，参考图最多 ${MAX_REFS} 张`);
      }
      return result;
    },
    [addRefs, onNotice],
  );
}
