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
  | "dashscope";

export const PROTOCOL_LABELS: Record<Protocol, string> = {
  openai_images: "OpenAI Images（gpt-image / DALL-E / Flux）",
  openai_chat_image: "Chat 生图（gpt-4o-image / Grok / 万相）",
  gemini_native: "Gemini 原生（nano-banana）",
  midjourney_proxy: "Midjourney Proxy（MJ-Proxy）",
  volcengine_ark: "火山方舟（豆包 Seedream / 即梦）",
  dashscope: "阿里云百炼（通义万相）",
};

export const PROTOCOL_PRESET_URL: Record<Protocol, string> = {
  openai_images: "https://api.openai.com",
  openai_chat_image: "https://api.openai.com",
  gemini_native: "https://generativelanguage.googleapis.com",
  midjourney_proxy: "https://mj.example.com",
  volcengine_ark: "https://ark.cn-beijing.volces.com",
  dashscope: "https://dashscope.aliyuncs.com",
};

export const PROTOCOL_PRESET_MODELS: Record<Protocol, string[]> = {
  openai_images: ["gpt-image-2.5", "gpt-image-2", "gpt-image-1", "dall-e-3", "flux-1.1-pro", "seedream-4.0"],
  openai_chat_image: ["gpt-4o-image", "grok-2-image", "wanxiang-v2"],
  gemini_native: ["gemini-2.5-flash-image", "imagen-4.0-generate-001"],
  midjourney_proxy: ["midjourney", "midjourney-fast", "midjourney-turbo"],
  volcengine_ark: ["doubao-seedream-5-5-261028", "doubao-seedream-5-0-pro-260128", "doubao-seedream-4-0-250828"],
  dashscope: ["wan2.6-t2i", "wan2.5-t2i-preview", "wan2.2-t2i-flash", "wanx2.1-t2i-turbo"],
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
];
