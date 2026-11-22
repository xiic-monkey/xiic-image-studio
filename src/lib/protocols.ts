/** 协议常量与供应商模板。
 *
 * 单独成文件是为了让 node --test 能直接 import（测试要带 .ts 扩展名，
 * 而 src/types.ts 被大量组件 import，不适合当测试入口）。
 * 后端 Protocol 枚举新增时，这里要同步，否则前端下拉会缺协议。
 */
export type Protocol =
  | "openai_images"
  | "openai_chat_image"
  | "gemini_native"
  | "midjourney_proxy"
  | "volcengine_ark"
  | "dashscope"
  | "stability"
  | "ideogram"
  | "flux_bfl"
  | "minimax";

export const PROTOCOL_LABELS: Record<Protocol, string> = {
  openai_images: "OpenAI Images（gpt-image / DALL-E / Flux）",
  openai_chat_image: "Chat 生图（gpt-4o-image / Grok / 万相）",
  gemini_native: "Gemini 原生（nano-banana）",
  midjourney_proxy: "Midjourney Proxy（MJ-Proxy）",
  volcengine_ark: "火山方舟（豆包 Seedream / 即梦）",
  dashscope: "阿里云百炼（通义万相 / Qwen-Image）",
  stability: "Stability AI（Stable Image / SD3.5）",
  ideogram: "Ideogram（文字排版）",
  flux_bfl: "Black Forest Labs FLUX（FLUX.2）",
  minimax: "MiniMax 图像（H3-Image）",
};

export const PROTOCOL_PRESET_URL: Record<Protocol, string> = {
  openai_images: "https://api.openai.com",
  openai_chat_image: "https://api.openai.com",
  gemini_native: "https://generativelanguage.googleapis.com",
  midjourney_proxy: "https://mj.example.com",
  volcengine_ark: "https://ark.cn-beijing.volces.com",
  dashscope: "https://dashscope.aliyuncs.com",
  stability: "https://api.stability.ai",
  ideogram: "https://api.ideogram.ai",
  flux_bfl: "https://api.bfl.ai",
  minimax: "https://tokenhub.tencentmaas.com",
};

/**
 * 每个协议下常用模型名。同一个模型家族只放**最新两版**——
 * 更老的版本除了复现历史任务没有意义，能力表按前缀匹配天然覆盖后续版本。
 */
export const PROTOCOL_PRESET_MODELS: Record<Protocol, string[]> = {
  openai_images: [
    "gpt-image-2.6",
    "gpt-image-2.5",
    "gpt-image-2",
    "dall-e-3",
    "recraftv4_1",
    "qwen-image-3.0-pro",
    "flux-1.1-pro",
  ],
  openai_chat_image: ["grok-2-image", "gpt-4o-image", "wanxiang-v2"],
  gemini_native: [
    "gemini-3.1-flash-image",
    "gemini-3-pro-image-preview",
    "gemini-2.5-flash-image",
    "imagen-4.0-generate-001",
  ],
  midjourney_proxy: ["midjourney", "midjourney-fast", "midjourney-turbo"],
  volcengine_ark: [
    "doubao-seedream-5-5-261028",
    "doubao-seedream-5-0-pro-260128",
    "doubao-seedream-4-0-250828",
  ],
  dashscope: [
    "qwen-image-3.0-pro",
    "qwen-image-3.0",
    "wan2.6-t2i",
    "wan2.5-t2i-preview",
    "wan2.2-t2i-flash",
  ],
  stability: [
    "stable-image-ultra",
    "stable-image-core",
    "sd3.5-large",
    "sd3.5-medium",
    "sd3.5-flash",
  ],
  ideogram: ["ideogram-v4", "ideogram-v3"],
  flux_bfl: [
    "flux-2-pro",
    "flux-2-max",
    "flux-2-flex",
    "flux-2-klein-9b",
    "flux-2-klein-4b",
  ],
  minimax: ["minimax-image-h3", "minimax-image-h3-highspeed"],
};

/** 供应商模板：选一个就把名称/baseUrl/协议/常用模型填好，剩下的改 Key 就行。 */
export interface ProviderTemplate {
  id: string;
  name: string;
  base_url: string;
  protocol: Protocol;
  /** 常用模型名，一键批量建好并挂上 */
  models: string[];
  note?: string;
}

export const PROVIDER_TEMPLATES: ProviderTemplate[] = [
  {
    id: "openai",
    name: "OpenAI 官方",
    base_url: "https://api.openai.com",
    protocol: "openai_images",
    models: ["gpt-image-1", "dall-e-3"],
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    base_url: "https://openrouter.ai/api",
    protocol: "openai_chat_image",
    models: ["google/gemini-2.5-flash-image", "black-forest-labs/flux-1.1-pro"],
    note: "OpenRouter 走 Chat Completions 出图",
  },
  {
    id: "siliconflow",
    name: "SiliconFlow 硅基流动",
    base_url: "https://api.siliconflow.cn",
    protocol: "openai_images",
    models: ["black-forest-labs/FLUX.1-dev"],
  },
  {
    id: "gemini",
    name: "Google Gemini",
    base_url: "https://generativelanguage.googleapis.com",
    protocol: "gemini_native",
    models: ["gemini-2.5-flash-image", "imagen-4.0-generate-001"],
  },
  {
    id: "volcengine",
    name: "火山方舟（豆包 Seedream）",
    base_url: "https://ark.cn-beijing.volces.com",
    protocol: "volcengine_ark",
    models: ["doubao-seedream-5-5-261028", "doubao-seedream-4-0-250828"],
    note: "国内直连，免梯子",
  },
  {
    id: "dashscope",
    name: "阿里云百炼（万相）",
    base_url: "https://dashscope.aliyuncs.com",
    protocol: "dashscope",
    models: ["wan2.6-t2i", "wan2.5-t2i-preview"],
    note: "异步任务型，国内直连",
  },
  {
    id: "dashscope-compatible",
    name: "阿里云百炼 OpenAI 兼容模式",
    base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    protocol: "openai_images",
    models: ["qwen-image-3.0-pro", "qwen-image-3.0"],
    note: "Qwen-Image 走 OpenAI 兼容端点",
  },
  {
    id: "stability",
    name: "Stability AI",
    base_url: "https://api.stability.ai",
    protocol: "stability",
    models: ["stable-image-ultra", "stable-image-core", "sd3.5-large"],
    note: "multipart 请求，尺寸是画面比例",
  },
  {
    id: "ideogram",
    name: "Ideogram",
    base_url: "https://api.ideogram.ai",
    protocol: "ideogram",
    models: ["ideogram-v4", "ideogram-v3"],
    note: "文字排版最强；模型名在请求路径里",
  },
  {
    id: "flux",
    name: "Black Forest Labs FLUX",
    base_url: "https://api.bfl.ai",
    protocol: "flux_bfl",
    models: ["flux-2-pro", "flux-2-max", "flux-2-flex"],
    note: "提交后需轮询；宽高必须是 16 的倍数",
  },
  {
    id: "minimax",
    name: "MiniMax 图像（H3-Image）",
    base_url: "https://tokenhub.tencentmaas.com",
    protocol: "minimax",
    models: ["minimax-image-h3", "minimax-image-h3-highspeed"],
    note: "国内可用，支持多图参考编辑",
  },
  {
    id: "recraft",
    name: "Recraft",
    base_url: "https://external.api.recraft.ai/v1",
    protocol: "openai_images",
    models: ["recraftv4_1", "recraftv4_1_vector"],
    note: "Recraft 兼容 OpenAI Images 请求形状",
  },
];
