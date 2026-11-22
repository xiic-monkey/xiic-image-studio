import { create } from "zustand";
import { api } from "../ipc";
import { errorMessage } from "../lib/error";
import type { ModelItem } from "../types";

interface ModelState {
  models: ModelItem[];
  loading: boolean;
  error: string | null;
  load: () => Promise<void>;
  remove: (id: string) => Promise<void>;
}

export const useModels = create<ModelState>((set) => ({
  models: [],
  loading: false,
  error: null,
  load: async () => {
    set({ loading: true, error: null });
    try {
      const models = await api.modelList();
      set({ models, loading: false });
    } catch (e) {
      set({ error: errorMessage(e), loading: false });
    }
  },
  remove: async (id) => {
    await api.modelDelete(id);
    await useModels.getState().load();
  },
}));
