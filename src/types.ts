export type Protocol = "openai_images" | "openai_chat_image" | "gemini_native" | "midjourney_proxy";

export interface ProviderListItem {
  id: string;
  name: string;
  base_url: string;
  protocol: Protocol;
  model: string;
  custom_headers: Record<string, string>;
  has_key: boolean;
  created_at: string;
  updated_at: string;
}

export interface ProviderDraft {
  id?: string | null;
  base_url: string;
  protocol: Protocol;
  model?: string;
  api_key?: string;
  custom_headers?: Record<string, string>;
}

export interface SaveInput {
  id: string;
  name: string;
  base_url: string;
  protocol: Protocol;
  model: string;
  custom_headers: Record<string, string>;
  api_key?: string | null;
}

export interface TestResult {
  ok: boolean;
  status: number | null;
  message: string;
  latency_ms: number | null;
}

export interface DiscoveredModels {
  models: string[];
  message: string;
}

export const PROTOCOL_LABELS: Record<Protocol, string> = {
  openai_images: "OpenAI Images（gpt-image / DALL-E / Flux）",
  openai_chat_image: "Chat 生图（gpt-4o-image / Grok / 万相）",
  gemini_native: "Gemini 原生（nano-banana）",
  midjourney_proxy: "Midjourney Proxy（MJ-Proxy）",
};

export const PROTOCOL_PRESET_URL: Record<Protocol, string> = {
  openai_images: "https://api.openai.com",
  openai_chat_image: "https://api.openai.com",
  gemini_native: "https://generativelanguage.googleapis.com",
  midjourney_proxy: "https://mj.example.com",
};

export const PROTOCOL_PRESET_MODELS: Record<Protocol, string[]> = {
  openai_images: ["gpt-image-2.5", "gpt-image-2", "gpt-image-1", "dall-e-3", "flux-1.1-pro", "seedream-4.0"],
  openai_chat_image: ["gpt-4o-image", "grok-2-image", "wanxiang-v2"],
  gemini_native: ["gemini-2.5-flash-image", "imagen-4.0-generate-001"],
  midjourney_proxy: ["midjourney", "midjourney-fast", "midjourney-turbo"],
};

// ---------- 生成 ----------

export interface RefImage {
  name: string;
  mime: string;
  data: string; // base64 无前缀
}

export interface GenerateRequest {
  provider_id: string;
  prompt: string;
  n: number;
  size?: string;
  quality?: string;
  seed?: number;
  refs: RefImage[];
  mask?: RefImage;
}

export interface ImageMeta {
  id: string;
  path: string;
  thumb_path: string;
}

export interface TaskProgress {
  taskId: string;
  status: string;
  done: number;
  total: number;
  error?: string;
  image?: ImageMeta;
}

export interface TaskItem {
  id: string;
  provider_id: string;
  protocol: string;
  status: string;
  total: number;
  done: number;
  error?: string;
  created_at: string;
  prompt?: string;
  /** 原始生成参数，供"一键重试"复用 */
  size?: string | null;
  quality?: string | null;
  n?: number;
  /** 参考图数量：>0 表示图生图，重试会缺参考图（前端据此隐藏重试入口） */
  ref_count?: number;
  /** 该任务已生成的图片（刷新后靠它回填缩略图） */
  images?: ImageMeta[];
}

// ---------- M3 ----------

export interface GalleryItem {
  id: string;
  path: string;
  thumb_path: string;
  prompt: string;
  model: string;
  favorite: boolean;
  width: number;
  height: number;
  size: number;
  params: { size?: string; quality?: string; seed?: number; mj_task_id?: string };
  created_at: string;
}

export interface PromptItem {
  id: string;
  title: string;
  content: string;
  tags: string[];
  created_at: string;
}

export interface SessionItem {
  id: string;
  name: string;
  draft: {
    providerId?: string;
    prompt?: string;
    size?: string;
    quality?: string;
    count?: number;
    seed?: string;
  };
  updated_at: string;
}
