import { create } from "zustand";
import { appendRefs } from "../lib/ref-image";
import type { RefImage } from "../types";

interface AssetsState {
  refs: RefImage[];
  /** 追加参考图并去重，返回实际加入 / 重复张数；满 4 张时剩余部分不静默丢失。 */
  addRefs: (imgs: RefImage[]) => { added: number; duplicates: number };
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
    const { refs, added, duplicates } = appendRefs(get().refs, imgs);
    if (added > 0) set({ refs });
    return { added, duplicates };
  },
  removeRef: (index) => set((s) => ({ refs: s.refs.filter((_, i) => i !== index) })),
  clearRefs: () => set({ refs: [] }),
}));
