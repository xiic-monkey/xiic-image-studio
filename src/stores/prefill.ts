import { create } from "zustand";
import type { RefImage } from "../types";

export interface Prefill {
  prompt?: string;
  size?: string;
  quality?: string;
  count?: number;
  seed?: string;
  refs?: RefImage[];
  nonce: number;
}

interface PrefillState {
  prefill: Prefill | null;
  send: (p: Omit<Prefill, "nonce">) => void;
}

export const usePrefill = create<PrefillState>((set) => ({
  prefill: null,
  send: (p) => set({ prefill: { ...p, nonce: Date.now() } }),
}));
