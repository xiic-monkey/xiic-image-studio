use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    /// OpenAI Images: POST {base}/v1/images/generations | edits
    #[serde(rename = "openai_images")]
    OpenAiImages,
    /// OpenAI Chat 生图: POST {base}/v1/chat/completions，从回复解析图片
    #[serde(rename = "openai_chat_image")]
    OpenAiChatImage,
    /// Gemini 原生: POST {base}/v1beta/models/{m}:generateContent
    #[serde(rename = "gemini_native")]
    GeminiNative,
    /// Midjourney Proxy: /mj/submit/* + 轮询
    #[serde(rename = "midjourney_proxy")]
    MidjourneyProxy,
    /// 火山方舟 Seedream: POST {base}/api/v3/images/generations（同步返回）
    #[serde(rename = "volcengine_ark")]
    VolcengineArk,
    /// 阿里云百炼万相: POST {base}/api/v1/services/aigc/text2image/image-synthesis + 轮询任务
    #[serde(rename = "dashscope")]
    DashScope,
    /// Stability AI: POST {base}/v2beta/stable-image/generate/{model}（multipart，尺寸是画面比例）
    #[serde(rename = "stability")]
    Stability,
    /// Ideogram: POST {base}/v1/{model}/generate（Api-Key 头，尺寸是画面比例）
    #[serde(rename = "ideogram")]
    Ideogram,
    /// Black Forest Labs FLUX: POST {base}/v1/{model} + 轮询 polling_url（x-key 头）
    #[serde(rename = "flux_bfl")]
    FluxBfl,
    /// MiniMax 图像: POST {base}/v1/wand/minimax-image/generation（+ /edits 走参考图）
    #[serde(rename = "minimax")]
    Minimax,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::OpenAiImages => "openai_images",
            Protocol::OpenAiChatImage => "openai_chat_image",
            Protocol::GeminiNative => "gemini_native",
            Protocol::MidjourneyProxy => "midjourney_proxy",
            Protocol::VolcengineArk => "volcengine_ark",
            Protocol::DashScope => "dashscope",
            Protocol::Stability => "stability",
            Protocol::Ideogram => "ideogram",
            Protocol::FluxBfl => "flux_bfl",
            Protocol::Minimax => "minimax",
        }
    }

    pub fn parse(s: &str) -> Option<Protocol> {
        match s {
            "openai_images" => Some(Protocol::OpenAiImages),
            "openai_chat_image" => Some(Protocol::OpenAiChatImage),
            "gemini_native" => Some(Protocol::GeminiNative),
            "midjourney_proxy" => Some(Protocol::MidjourneyProxy),
            "volcengine_ark" => Some(Protocol::VolcengineArk),
            "dashscope" => Some(Protocol::DashScope),
            "stability" => Some(Protocol::Stability),
            "ideogram" => Some(Protocol::Ideogram),
            "flux_bfl" => Some(Protocol::FluxBfl),
            "minimax" => Some(Protocol::Minimax),
            _ => None,
        }
    }

    /// 常见默认 baseUrl，供前端预设一键填
    pub fn preset_base_url(&self) -> &'static str {
        match self {
            Protocol::OpenAiImages | Protocol::OpenAiChatImage => "https://api.openai.com",
            Protocol::GeminiNative => "https://generativelanguage.googleapis.com",
            Protocol::MidjourneyProxy => "https://mj.example.com",
            Protocol::VolcengineArk => "https://ark.cn-beijing.volces.com",
            Protocol::DashScope => "https://dashscope.aliyuncs.com",
            Protocol::Stability => "https://api.stability.ai",
            Protocol::Ideogram => "https://api.ideogram.ai",
            Protocol::FluxBfl => "https://api.bfl.ai",
            Protocol::Minimax => "https://tokenhub.tencentmaas.com",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub custom_headers: BTreeMap<String, String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 前端回传给测试/发现的草稿配置（key 明文只在测试时出现，不落库）
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderDraft {
    #[serde(default)]
    pub id: Option<String>,
    pub base_url: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub model: String,
    /// 留空时若提供了 id 则使用已存的钥匙串 Key
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub custom_headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderListItem {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub protocol: Protocol,
    pub model: String,
    pub custom_headers: BTreeMap<String, String>,
    pub has_key: bool,
    pub created_at: String,
    pub updated_at: String,
    /// 该协议+模型实际接受的参数（前端据此收敛控件，适配器据此发参）
    pub capabilities: crate::capabilities::Capabilities,
    /// 挂在这个供应商下的模型 id（多对多关联；前端据此渲染模型下拉）
    #[serde(default)]
    pub model_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    pub ok: bool,
    pub status: Option<u16>,
    pub message: String,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredModels {
    pub models: Vec<String>,
    pub message: String,
}
