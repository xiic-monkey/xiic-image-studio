import { create } from "zustand";

interface ToastState {
  message: string | null;
  /** 每次 show 自增：同一条消息重复触发也能重置计时器 */
  seq: number;
  show: (msg: string) => void;
  hide: () => void;
}

/** 全局唯一的轻量提示状态，任何模块用 useToast().show 即可。 */
export const useToast = create<ToastState>((set) => ({
  message: null,
  seq: 0,
  show: (message) => set((s) => ({ message, seq: s.seq + 1 })),
  hide: () => set({ message: null }),
}));
