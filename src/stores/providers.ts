import { create } from "zustand";
import { api } from "../ipc";
import type { ProviderListItem } from "../types";

interface ProviderState {
  providers: ProviderListItem[];
  loading: boolean;
  error: string | null;
  load: () => Promise<void>;
  remove: (id: string) => Promise<void>;
}

export const useProviders = create<ProviderState>((set) => ({
  providers: [],
  loading: false,
  error: null,
  load: async () => {
    set({ loading: true, error: null });
    try {
      const providers = await api.providerList();
      set({ providers, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  remove: async (id) => {
    await api.providerDelete(id);
    await useProviders.getState().load();
  },
}));
