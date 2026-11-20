import { create } from "zustand";

export type Tab = "workbench" | "gallery" | "prompts" | "tools" | "providers";

interface NavState {
  tab: Tab;
  setTab: (tab: Tab) => void;
}

/** 当前视图提到全局，画廊/工具箱才能把用户送到工作台。 */
export const useNav = create<NavState>((set) => ({
  tab: "workbench",
  setTab: (tab) => set({ tab }),
}));
