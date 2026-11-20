import { create } from "zustand";

export interface Prefill {
  prompt?: string;
  size?: string;
  quality?: string;
  count?: number;
  seed?: string;
  nonce: number;
}

interface PrefillState {
  prefill: Prefill | null;
  send: (p: Omit<Prefill, "nonce">) => void;
  /** 消费后清掉：切 tab 会卸载工作台，留着会在下次挂载时又灌一遍旧值。 */
  clear: () => void;
}

export const usePrefill = create<PrefillState>((set) => ({
  prefill: null,
  send: (p) => set({ prefill: { ...p, nonce: Date.now() } }),
  clear: () => set({ prefill: null }),
}));
