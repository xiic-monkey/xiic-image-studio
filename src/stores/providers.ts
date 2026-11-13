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
      // Tauri 命令错误的拒绝值是字符串（AppError 直接序列化成文本），
      // 而 JS 异常是 Error 对象——分开处理，别让界面上出现 "Error: " 前缀
      set({ error: e instanceof Error ? e.message : String(e), loading: false });
    }
  },
  remove: async (id) => {
    await api.providerDelete(id);
    await useProviders.getState().load();
  },
}));
