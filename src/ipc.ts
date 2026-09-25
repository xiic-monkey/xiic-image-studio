import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { DiscoveredModels, GenerateRequest, GalleryItem, PromptItem, ProviderDraft, ProviderListItem, SaveInput, SessionItem, TaskItem, TaskProgress, TestResult } from "./types";

export const api = {
  providerList: () => invoke<ProviderListItem[]>("provider_list"),
  providerSave: (provider: SaveInput) => invoke<void>("provider_save", { provider }),
  providerDelete: (id: string) => invoke<void>("provider_delete", { id }),
  providerTest: (draft: ProviderDraft) => invoke<TestResult>("provider_test", { draft }),
  providerDiscover: (draft: ProviderDraft) => invoke<DiscoveredModels>("provider_discover_models", { draft }),
  generateSubmit: (request: GenerateRequest) => invoke<string>("generate_submit", { request }),
  generateCancel: (taskId: string) => invoke<void>("generate_cancel", { taskId }),
  taskList: (limit?: number) => invoke<TaskItem[]>("task_list", { limit }),
  taskDelete: (id: string) => invoke<void>("task_delete", { id }),
  imageReadB64: (path: string) => invoke<string>("image_read_b64", { path }),
  imageList: (opts?: { q?: string; favoriteOnly?: boolean; model?: string; limit?: number; offset?: number }) =>
    invoke<GalleryItem[]>("image_list", {
      q: opts?.q ?? null,
      favoriteOnly: opts?.favoriteOnly ?? null,
      model: opts?.model ?? null,
      limit: opts?.limit ?? null,
      offset: opts?.offset ?? null,
    }),
  imageSetFavorite: (id: string, favorite: boolean) => invoke<void>("image_set_favorite", { id, favorite }),
  imageDelete: (id: string) => invoke<void>("image_delete", { id }),
  imageReveal: (id: string) => invoke<void>("image_reveal", { id }),
  promptList: () => invoke<PromptItem[]>("prompt_list"),
  promptSave: (prompt: { id?: string; title: string; content: string; tags?: string[] }) =>
    invoke<string>("prompt_save", { prompt }),
  promptDelete: (id: string) => invoke<void>("prompt_delete", { id }),
  sessionList: () => invoke<SessionItem[]>("session_list"),
  sessionSave: (session: { id?: string; name?: string; draft?: unknown }) =>
    invoke<string>("session_save", { session }),
  sessionDelete: (id: string) => invoke<void>("session_delete", { id }),
  mjAction: (imageId: string, command: string) => invoke<string>("mj_action", { imageId, command }),
  toolsStatus: () =>
    invoke<{ available: boolean; path: string; source: string; formats: Record<string, boolean> | null }>("tools_status"),
  toolsSetFfmpegPath: (path: string) =>
    invoke<{ available: boolean; path: string; source: string; formats: Record<string, boolean> | null }>("tools_set_ffmpeg_path", { path }),
  toolsConvert: (inputs: string[], format: string, quality?: number, maxSide?: number) =>
    invoke<string[]>("tools_convert", { inputs, format, quality: quality ?? null, maxSide: maxSide ?? null }),
  toolsGif: (inputs: string[], fps?: number, maxSide?: number) =>
    invoke<string>("tools_gif", { inputs, fps: fps ?? null, maxSide: maxSide ?? null }),
  toolsVideoFrames: (video: string, intervalSec?: number, maxFrames?: number, maxSide?: number) =>
    invoke<string[]>("tools_video_frames", { video, intervalSec: intervalSec ?? null, maxFrames: maxFrames ?? null, maxSide: maxSide ?? null }),
  toolsFileB64: (path: string) => invoke<string>("tools_file_b64", { path }),
  readTextFile: (path: string) => invoke<string>("read_text_file", { path }),
};

export function onTaskProgress(cb: (p: TaskProgress) => void): Promise<() => void> {
  return listen<TaskProgress>("task-progress", (e) => cb(e.payload));
}
