use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Deserialize)]
pub struct RefImage {
    #[serde(default)]
    pub name: String,
    pub mime: String,
    /// base64（不带 data: 前缀）
    pub data: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateRequest {
    pub provider_id: String,
    pub prompt: String,
    #[serde(default = "default_n")]
    pub n: u32,
    /// 如 "1024x1024"
    #[serde(default)]
    pub size: Option<String>,
    /// openai images: auto/low/medium/high 或 standard/hd
    #[serde(default)]
    pub quality: Option<String>,
    /// 随机种子：支持的协议（openai_images / chat / gemini）会写进请求体，
    /// 上游不支持时静默忽略，不影响其他参数。
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub refs: Vec<RefImage>,
    /// 局部重绘蒙版（透明区=重绘区，仅 openai_images 使用）
    #[serde(default)]
    pub mask: Option<RefImage>,
}

fn default_n() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageMeta {
    pub id: String,
    pub path: String,
    pub thumb_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub task_id: String,
    pub status: String, // running | completed | failed | canceled
    pub done: u32,
    pub total: u32,
    pub error: Option<String>,
    pub image: Option<ImageMeta>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskItem {
    pub id: String,
    pub provider_id: String,
    pub protocol: String,
    pub status: String,
    pub total: u32,
    pub done: u32,
    pub error: Option<String>,
    pub created_at: String,
    /// 从 request_json 解析出的提示词，用于任务卡片展示缩写
    pub prompt: String,
    /// 以下三项从 request_json 解析，供"一键重试"复用原始参数
    pub size: Option<String>,
    pub quality: Option<String>,
    pub n: u32,
    /// 参考图数量。>0 说明是图生图：我们没存参考图本体，重试会退化成文生图，
    /// 前端据此不提供重试入口，避免"看着一样、结果不一样"的误导。
    pub ref_count: u64,
    /// 该任务已生成的图片：刷新窗口后靠它回填缩略图
    /// （实时进度只推送"新生成"的图，历史任务没有这条就永远没缩略图）
    pub images: Vec<ImageMeta>,
}

/// 适配器输出：原始字节 + mime + 协议特有元数据（如 MJ task id）
pub struct ImageOut {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub meta: Option<serde_json::Value>,
}

impl ImageOut {
    pub fn new(bytes: Vec<u8>, mime: &str) -> Self {
        ImageOut { bytes, mime: mime.to_string(), meta: None }
    }
}

/// 适配器运行上下文（已解析 Key）
pub struct GenCtx {
    pub base_url: String,
    pub key: String,
    pub model: String,
    pub custom_headers: std::collections::BTreeMap<String, String>,
    pub prompt: String,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub seed: Option<u64>,
    pub refs: Vec<RefImage>,
    pub mask: Option<RefImage>,
    /// 取消信号（MJ 长轮询期间检查）
    pub cancel: Option<Arc<AtomicBool>>,
}
