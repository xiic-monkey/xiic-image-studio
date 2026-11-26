import { create } from "zustand";

/** 提示语义：三档颜色 + 中性兜底（纯陈述，不表达成败） */
export type ToastKind = "info" | "success" | "warning" | "error";

/** 接受可选级别的提示回调，避免每个调用点都自己去读 store */
export type Notify = (msg: string, kind?: ToastKind) => void;

interface ToastState {
  message: string | null;
  kind: ToastKind;
  /** 每次 show 自增：同一条消息重复触发也能重置计时器 */
  seq: number;
  show: (msg: string, kind?: ToastKind) => void;
  hide: () => void;
}

/** 全局唯一的轻量提示状态，任何模块用 useToast().show 即可。 */
export const useToast = create<ToastState>((set) => ({
  message: null,
  kind: "info",
  seq: 0,
  show: (message, kind = "info") => set((s) => ({ message, kind, seq: s.seq + 1 })),
  hide: () => set({ message: null }),
}));
