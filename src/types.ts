import type { Protocol } from "./lib/protocols.ts";

export type { Protocol };
export {
  PROTOCOL_LABELS,
  PROTOCOL_PRESET_MODELS,
  PROTOCOL_PRESET_URL,
  PROVIDER_TEMPLATES,
} from "./lib/protocols.ts";
export type { ProviderTemplate } from "./lib/protocols.ts";

/** 某个「协议 + 模型」实际接受的参数集合，由后端能力表下发。 */
export interface Capabilities {
  /** 允许的尺寸取值；空数组 = 该协议/模型不接受尺寸参数 */
  sizes: string[];
  /** 允许的质量取值；空数组 = 不接受 quality */
  quality: string[];
  seed: boolean;
  refs: boolean;
  mask: boolean;
  /** 上游端点形态：images | chat | generate_content | predict | mj | ark | dashscope */
  endpoint: string;
  /** 尺寸控件标签：尺寸 | 画面比例 | 分辨率 */
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
  /** 挂在这个供应商下的模型 id（多对多关联） */
  model_ids: string[];
}

/** 一等模型实体：协议归模型，可被多家供应商复用。 */
export interface ModelItem {
  id: string;
  protocol: Protocol;
  model_name: string;
  label: string;
  capabilities: Capabilities;
  /** 挂了这个模型的供应商数量 */
  provider_count: number;
  /** 是否已挂在当前查看的供应商下 */
  linked: boolean;
  /** 用户对该模型能力的覆写 JSON（空 = 用内置推导值） */
  capabilities_override: string;
}

export interface ModelDraft {
  id?: string | null;
  protocol: Protocol;
  model_name: string;
  label: string;
  /** 能力覆写 JSON（可选）。非法 JSON 会在保存时报错，不会静默丢弃。 */
  capabilities_override?: string;
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

// ---------- 生成 ----------

export interface RefImage {
  name: string;
  mime: string;
  data: string; // base64 无前缀
  /** 来源图片 id（images 表），仅用于去重；后端忽略该字段 */
  id?: string;
}

export interface GenerateRequest {
  provider_id: string;
  /** 显式指定供应商下的哪个模型（id）；为空回退到默认模型 */
  model_id?: string | null;
  prompt: string;
  n: number;
  size?: string;
  quality?: string;
  seed?: number;
  refs: RefImage[];
  mask?: RefImage;
  /** 当前会话归属；CLI / 旧前端不传，后端按可空处理 */
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
  /** 参考图快照路径（相对 images 根）；旧任务的图生图没有这个键 */
  ref_paths?: string[];
  /** 会话归属；CLI 任务与旧数据为 null */
  session_id?: string | null;
  /** 该任务使用的模型 id（重试时据此复原模型） */
  model_id?: string | null;
  /** 该任务使用的上游模型名（展示用） */
  model_name?: string | null;
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
  /** 参考图快照路径；非空表示重试能复现原图生图 */
  refPaths: string[];
  /** 随机种子；旧数据为 null */
  seed: number | null;
  /** 会话归属；CLI 任务与旧数据为 null，重试时回落到当前会话 */
  sessionId: string | null;
  /** 该任务使用的模型 id（重试时据此复原模型） */
  modelId: string | null;
  /** 该任务使用的上游模型名（展示用） */
  modelName: string | null;
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
  /** 来源任务记的供应商/模型 id，供「存为配方」复原模型选择 */
  provider_id?: string | null;
  model_id?: string | null;
}

export interface PromptItem {
  id: string;
  title: string;
  content: string;
  tags: string[];
  created_at: string;
}

/** 一份可复用的生成配方：提示词 + 能让它复现的参数。 */
export interface PresetItem {
  id: string;
  name: string;
  prompt: string;
  size?: string | null;
  quality?: string | null;
  seed?: number | null;
  provider_id?: string | null;
  /** 配方保存时用的模型 id（应用时恢复模型选择） */
  model_id?: string | null;
  created_at: string;
}

export interface SessionItem {
  id: string;
  name: string;
  draft: {
    providerId?: string;
    modelId?: string;
    prompt?: string;
    size?: string;
    quality?: string;
    count?: number;
    seed?: string;
    batchMode?: boolean;
    scanMode?: boolean;
    scanCount?: number;
  };
  updated_at: string;
}
