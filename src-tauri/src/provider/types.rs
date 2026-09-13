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
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::OpenAiImages => "openai_images",
            Protocol::OpenAiChatImage => "openai_chat_image",
            Protocol::GeminiNative => "gemini_native",
            Protocol::MidjourneyProxy => "midjourney_proxy",
        }
    }

    pub fn parse(s: &str) -> Option<Protocol> {
        match s {
            "openai_images" => Some(Protocol::OpenAiImages),
            "openai_chat_image" => Some(Protocol::OpenAiChatImage),
            "gemini_native" => Some(Protocol::GeminiNative),
            "midjourney_proxy" => Some(Protocol::MidjourneyProxy),
            _ => None,
        }
    }

    /// 常见默认 baseUrl，供前端预设一键填
    pub fn preset_base_url(&self) -> &'static str {
        match self {
            Protocol::OpenAiImages | Protocol::OpenAiChatImage => "https://api.openai.com",
            Protocol::GeminiNative => "https://generativelanguage.googleapis.com",
            Protocol::MidjourneyProxy => "https://mj.example.com",
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
