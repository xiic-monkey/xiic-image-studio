use crate::provider::types::Protocol;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

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
    /// 是否下发由能力表决定（openai_images 规范里没有这个字段）
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub refs: Vec<RefImage>,
    /// 局部重绘蒙版（透明区=重绘区，仅 openai_images 使用）
    #[serde(default)]
    pub mask: Option<RefImage>,
    /// 当前会话归属；MCP / 旧前端不传时为空（列可空，旧数据兼容）
    #[serde(default)]
    pub session_id: Option<String>,
}

fn default_n() -> u32 {
    1
}

/// GUI 与 MCP 的提交上限不同：MCP 的 4 是工具对外契约（inputSchema）。
pub const MAX_N: u32 = 8;
pub const MCP_MAX_N: u32 = 4;
/// 单任务内同时在飞的上游请求数
pub const FANOUT: usize = 4;

pub fn clamp_n(n: u32, max: u32) -> u32 {
    n.clamp(1, max)
}

/// 三处入口唯一的 request_json 构造。只记「实际发出的参数」，
/// 一键重试靠这些字段复现；额外键只会随版本漂移且无人读取。
pub fn build_request_json(req: &GenerateRequest, n: u32) -> String {
    json!({
        "prompt": req.prompt.trim(),
        "n": n,
        "size": req.size,
        "quality": req.quality,
        "seed": req.seed,
        "ref_count": req.refs.len(),
    })
    .to_string()
}

/// MJ U/V 没有 prompt / 尺寸等可复现参数，task_list 解析出空 prompt
/// 后前端不会给出重试入口。
pub fn build_mj_action_request_json(command: &str, mj_task_id: &str) -> String {
    json!({ "action": command, "mj_task_id": mj_task_id }).to_string()
}

/// 单张图由谁产出：常规按协议分发；MJ U/V 是同一链路的另一种 action。
#[derive(Debug, Clone)]
pub enum ImageProducer {
    Protocol(Protocol),
    MjAction { mj_task_id: String, command: String },
}

impl ImageProducer {
    pub fn protocol(&self) -> Protocol {
        match self {
            ImageProducer::Protocol(p) => *p,
            ImageProducer::MjAction { .. } => Protocol::MidjourneyProxy,
        }
    }

    /// MJ 是异步提交 + 轮询，重试整个 adapter 等于重新提交一个付费任务。
    pub fn is_retryable(&self) -> bool {
        !matches!(
            self,
            ImageProducer::MjAction { .. } | ImageProducer::Protocol(Protocol::MidjourneyProxy)
        )
    }
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
    /// 以下字段从 request_json 解析，供"一键重试"复用原始参数
    pub size: Option<String>,
    pub quality: Option<String>,
    pub n: u32,
    pub seed: Option<u64>,
    /// 参考图数量。>0 说明是图生图：我们没存参考图本体，重试会退化成文生图，
    /// 前端据此不提供重试入口，避免"看着一样、结果不一样"的误导。
    pub ref_count: u64,
    /// 该任务已生成的图片：刷新窗口后靠它回填缩略图
    /// （实时进度只推送"新生成"的图，历史任务没有这条就永远没缩略图）
    pub images: Vec<ImageMeta>,
    /// 会话归属；MCP 任务与旧数据为 NULL
    pub session_id: Option<String>,
}

/// 适配器输出：原始字节 + mime + 协议特有元数据（如 MJ task id）
#[derive(Debug)]
pub struct ImageOut {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub meta: Option<serde_json::Value>,
}

impl ImageOut {
    pub fn new(bytes: Vec<u8>, mime: &str) -> Self {
        ImageOut {
            bytes,
            mime: mime.to_string(),
            meta: None,
        }
    }
}

/// 适配器运行上下文（已解析 Key）
#[derive(Clone)]
pub struct GenCtx {
    pub base_url: String,
    pub key: String,
    pub model: String,
    pub custom_headers: BTreeMap<String, String>,
    pub prompt: String,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub seed: Option<u64>,
    pub refs: Vec<RefImage>,
    pub mask: Option<RefImage>,
    /// 取消信号（MJ 长轮询期间检查）
    pub cancel: Option<Arc<AtomicBool>>,
}

/// 已落库、可直接交给编排层的任务
pub struct TaskRequest {
    pub task_id: String,
    pub n: u32,
    pub prompt: String,
    pub model: String,
    pub ctx: GenCtx,
    pub producer: ImageProducer,
}

/// 编排层执行结果。MCP 侧靠它组装 tools/call 响应。
#[derive(Default)]
pub struct TaskOutcome {
    pub task_id: String,
    pub status: String,
    pub images: Vec<ImageMeta>,
    pub errors: Vec<String>,
    /// 任务状态没写回库时记这里：图已生成（已扣费），不能因为记账失败丢掉
    pub db_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> GenerateRequest {
        GenerateRequest {
            provider_id: "p1".into(),
            prompt: "  cat  ".into(),
            n: 3,
            size: Some("1024x1024".into()),
            quality: Some("high".into()),
            seed: Some(42),
            refs: vec![RefImage {
                name: "r.png".into(),
                mime: "image/png".into(),
                data: "x".into(),
            }],
            mask: None,
            session_id: Some("s1".into()),
        }
    }

    #[test]
    fn build_request_json_keeps_only_retryable_fields() {
        let v: serde_json::Value = serde_json::from_str(&build_request_json(&req(), 3)).unwrap();
        assert_eq!(v["prompt"], "cat");
        assert_eq!(v["n"], 3);
        assert_eq!(v["size"], "1024x1024");
        assert_eq!(v["quality"], "high");
        assert_eq!(v["seed"], 42);
        assert_eq!(v["ref_count"], 1);
        assert!(
            v.get("session_id").is_none(),
            "会话归属走表列，不进 request_json"
        );
        assert!(v.get("source").is_none());
    }

    #[test]
    fn build_mj_action_request_json_has_no_prompt_key() {
        let v: serde_json::Value =
            serde_json::from_str(&build_mj_action_request_json("U1", "mj-1")).unwrap();
        assert!(v.get("prompt").is_none(), "MJ action 不该显示重试入口");
        assert_eq!(v["action"], "U1");
        assert_eq!(v["mj_task_id"], "mj-1");
    }

    #[test]
    fn clamp_n_never_exceeds_max() {
        assert_eq!(clamp_n(0, MAX_N), 1);
        assert_eq!(clamp_n(99, MAX_N), MAX_N);
        assert_eq!(clamp_n(99, MCP_MAX_N), MCP_MAX_N);
        assert_eq!(clamp_n(2, MCP_MAX_N), 2);
    }

    #[test]
    fn gen_ctx_clone_preserves_all_fields() {
        let ctx = GenCtx {
            base_url: "https://example.com".into(),
            key: "k".into(),
            model: "m".into(),
            custom_headers: BTreeMap::from([("X-A".to_string(), "b".to_string())]),
            prompt: "p".into(),
            size: Some("1:1".into()),
            quality: Some("high".into()),
            seed: Some(7),
            refs: vec![RefImage {
                name: "r".into(),
                mime: "image/png".into(),
                data: "d".into(),
            }],
            mask: None,
            cancel: Some(Arc::new(AtomicBool::new(false))),
        };
        let cloned = ctx.clone();
        assert_eq!(cloned.base_url, ctx.base_url);
        assert_eq!(cloned.model, ctx.model);
        assert_eq!(cloned.custom_headers, ctx.custom_headers);
        assert_eq!(cloned.prompt, ctx.prompt);
        assert_eq!(cloned.size, ctx.size);
        assert_eq!(cloned.quality, ctx.quality);
        assert_eq!(cloned.seed, ctx.seed);
        assert_eq!(cloned.refs.len(), 1);
        assert!(Arc::ptr_eq(
            cloned.cancel.as_ref().unwrap(),
            ctx.cancel.as_ref().unwrap()
        ));
    }

    #[test]
    fn image_producer_maps_mj_action_to_midjourney_proxy() {
        let p = ImageProducer::MjAction {
            mj_task_id: "t".into(),
            command: "U1".into(),
        };
        assert_eq!(p.protocol().as_str(), "midjourney_proxy");
        assert_eq!(
            ImageProducer::Protocol(Protocol::OpenAiImages)
                .protocol()
                .as_str(),
            "openai_images"
        );
    }

    #[test]
    fn image_producer_mj_is_not_retryable() {
        assert!(!ImageProducer::MjAction {
            mj_task_id: "t".into(),
            command: "U1".into()
        }
        .is_retryable());
        assert!(!ImageProducer::Protocol(Protocol::MidjourneyProxy).is_retryable());
        assert!(ImageProducer::Protocol(Protocol::OpenAiImages).is_retryable());
    }
}
