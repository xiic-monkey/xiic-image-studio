import { create } from "zustand";
import { appendRefs } from "../lib/ref-image";
import type { RefImage } from "../types";

interface AssetsState {
  refs: RefImage[];
  /** 追加参考图，返回实际加入张数；满 4 张时剩余部分不静默丢失，由调用方提示。 */
  addRefs: (imgs: RefImage[]) => number;
  removeRef: (index: number) => void;
  clearRefs: () => void;
}

/**
 * 参考图放全局而非 WorkbenchView 的 useState：切 tab 会卸载工作台，
 * 本地 state 会丢；画廊加图 / 工具箱转图都要往同一份 refs 里累加。
 */
export const useAssets = create<AssetsState>((set, get) => ({
  refs: [],
  addRefs: (imgs) => {
    const { refs, added } = appendRefs(get().refs, imgs);
    if (added > 0) set({ refs });
    return added;
  },
  removeRef: (index) => set((s) => ({ refs: s.refs.filter((_, i) => i !== index) })),
  clearRefs: () => set({ refs: [] }),
}));
