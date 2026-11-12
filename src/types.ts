export type Protocol = "openai_images" | "openai_chat_image" | "gemini_native" | "midjourney_proxy";

/** 某个「协议 + 模型」实际接受的参数集合，由后端能力表下发。 */
export interface Capabilities {
  /** 允许的尺寸取值；空数组 = 该协议/模型不接受尺寸参数 */
  sizes: string[];
  /** 允许的质量取值；空数组 = 不接受 quality */
  quality: string[];
  seed: boolean;
  refs: boolean;
  mask: boolean;
  /** 上游端点形态：images | chat | generate_content | predict | mj */
  endpoint: string;
  /** 尺寸控件标签：尺寸 | 画面比例 */
  sizeLabel: string;
}

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
  capabilities: Capabilities;
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
  /** 当前会话归属；MCP / 旧前端不传，后端按可空处理 */
  session_id?: string;
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
  /** 随机种子，重试时一并带回 */
  seed?: number | null;
  /** 参考图数量：>0 表示图生图，重试会缺参考图（前端据此隐藏重试入口） */
  ref_count?: number;
  /** 会话归属；MCP 任务与旧数据为 null */
  session_id?: string | null;
  /** 该任务已生成的图片（刷新后靠它回填缩略图） */
  images?: ImageMeta[];
}

/** 任务卡视图模型：轮询结果与实时进度事件合并后的形状。 */
export interface TaskView {
  id: string;
  status: string;
  done: number;
  total: number;
  errors: string[];
  images: ImageMeta[];
  createdAt: string;
  prompt: string;
  /** 原始参数，重试时复用（重试不带参考图，refCount > 0 的任务不提供重试） */
  providerId: string;
  size: string | null;
  quality: string | null;
  n: number;
  refCount: number;
  /** 随机种子；旧数据为 null */
  seed: number | null;
  /** 会话归属；MCP 任务与旧数据为 null，重试时回落到当前会话 */
  sessionId: string | null;
}

export type TaskUpsert = Partial<TaskView> & { id: string; error?: string };

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
    batchMode?: boolean;
  };
  updated_at: string;
}
