use crate::generate::types::GenerateRequest;
use crate::provider::types::Protocol;
use serde::{Deserialize, Serialize};

/// 某个「协议 + 模型」实际接受的参数集合。唯一真源：
/// 适配器据此发参、前端据此收敛控件、提交入口据此裁剪请求。
///
/// 能力来源有三层，后者覆盖前者：
/// 1. 协议默认值（`protocol_defaults`）
/// 2. 内置模型规则表（`MODEL_RULES`，按模型名前缀匹配）
/// 3. 用户对单个模型的 DB 级覆写（`models.capabilities_json`）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// 允许的尺寸取值；空 = 该协议/模型不接受尺寸参数
    pub sizes: Vec<String>,
    /// 允许的质量取值；空 = 不接受 quality
    pub quality: Vec<String>,
    pub seed: bool,
    pub refs: bool,
    pub mask: bool,
    /// 上游端点形态：images | chat | generate_content | predict | mj | ark | dashscope
    pub endpoint: String,
    /// 前端尺寸控件标签：尺寸 | 画面比例 | 分辨率
    pub size_label: String,
}

impl Capabilities {
    pub fn allows_size(&self, s: &str) -> bool {
        self.sizes.iter().any(|x| x == s)
    }

    pub fn allows_quality(&self, q: &str) -> bool {
        self.quality.iter().any(|x| x == q)
    }

    /// 叠加用户对单个模型的能力覆写（models.capabilities_json）。
    /// 只覆盖用户显式给出的字段，其余保持推导值。
    pub fn apply_override(&mut self, o: &CapabilityOverride) {
        if let Some(s) = &o.sizes {
            self.sizes = s.clone();
        }
        if let Some(q) = &o.quality {
            self.quality = q.clone();
        }
        if let Some(v) = o.seed {
            self.seed = v;
        }
        if let Some(v) = o.refs {
            self.refs = v;
        }
        if let Some(v) = o.mask {
            self.mask = v;
        }
        if let Some(v) = &o.endpoint {
            self.endpoint = v.clone();
        }
        if let Some(v) = &o.size_label {
            self.size_label = v.clone();
        }
    }
}

/// 模型级能力覆写（部分字段）。存 models.capabilities_json，
/// 给中转站这类非标准模型补尺寸/质量取值用。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CapabilityOverride {
    #[serde(default)]
    pub sizes: Option<Vec<String>>,
    #[serde(default)]
    pub quality: Option<Vec<String>>,
    #[serde(default)]
    pub seed: Option<bool>,
    #[serde(default)]
    pub refs: Option<bool>,
    #[serde(default)]
    pub mask: Option<bool>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub size_label: Option<String>,
}

/// 解析用户填的能力覆写 JSON。空/非法一律当作「没覆写」——
/// 能力表只是为了让参数发得出去，解析失败不该挡住用户出图。
pub fn parse_override(json: Option<&str>) -> Option<CapabilityOverride> {
    let raw = json?.trim();
    if raw.is_empty() {
        return None;
    }
    serde_json::from_str::<CapabilityOverride>(raw).ok()
}

pub const ENDPOINT_IMAGES: &str = "images";
pub const ENDPOINT_CHAT: &str = "chat";
pub const ENDPOINT_GENERATE_CONTENT: &str = "generate_content";
pub const ENDPOINT_PREDICT: &str = "predict";
pub const ENDPOINT_MJ: &str = "mj";
pub const ENDPOINT_ARK: &str = "ark";
pub const ENDPOINT_DASHSCOPE: &str = "dashscope";

const GPT_IMAGE_SIZES: &[&str] = &["1024x1024", "1536x1024", "1024x1536"];
const DALL_E3_SIZES: &[&str] = &["1024x1024", "1792x1024", "1024x1792"];
const RELAY_SIZES: &[&str] = &["1024x1024", "1536x1024", "1024x1536", "2048x2048"];
const GPT_IMAGE_QUALITY: &[&str] = &["auto", "low", "medium", "high"];
const DALL_E3_QUALITY: &[&str] = &["standard", "hd"];
const GEMINI_RATIOS: &[&str] = &["1:1", "3:4", "4:3", "9:16", "16:9"];
/// 火山方舟 Seedream 用档位而非精确像素（也支持精确宽高，值域宽，这里给常用档）
const ARK_SIZES: &[&str] = &["1K", "1.5K", "2K", "auto"];
/// 百炼万相的尺寸用「宽*高」星号格式
const DASHSCOPE_SIZES: &[&str] = &[
    "1024*1024",
    "1024*768",
    "768*1024",
    "1280*720",
    "720*1280",
    "1440*1440",
];

fn normalize_model(model: &str) -> String {
    // 先小写再剥前缀：`MODELS/Imagen-4.0` 也要归一成 `imagen-4.0`
    model
        .trim()
        .to_ascii_lowercase()
        .trim_start_matches("models/")
        .to_string()
}

/// 编译期能力描述。用 &'static 切片而非 Vec，避免每次推导都分配。
#[derive(Clone, Copy)]
struct CapsSpec {
    sizes: &'static [&'static str],
    quality: &'static [&'static str],
    seed: bool,
    refs: bool,
    mask: bool,
    endpoint: &'static str,
    size_label: &'static str,
}

/// 模型级覆写规则。命中方式：同协议 + 模型名（小写去前缀后）以 prefix 开头。
/// 表内顺序即优先级：第一条命中的规则生效，后续不再叠加。
/// 新增模型只需要在这里加一行数据，不必改控制流。
struct ModelRule {
    protocol: Protocol,
    prefix: &'static str,
    sizes: Option<&'static [&'static str]>,
    quality: Option<&'static [&'static str]>,
    seed: Option<bool>,
    refs: Option<bool>,
    mask: Option<bool>,
    endpoint: Option<&'static str>,
    size_label: Option<&'static str>,
}

impl ModelRule {
    /// 全空规则：静态表里用 `..ModelRule::none()` 补齐其余字段。
    /// 必须是 const fn——静态初始化器里不能调用非 const 的 Default::default。
    const fn none() -> Self {
        ModelRule {
            protocol: Protocol::OpenAiImages,
            prefix: "",
            sizes: None,
            quality: None,
            seed: None,
            refs: None,
            mask: None,
            endpoint: None,
            size_label: None,
        }
    }
}

static MODEL_RULES: &[ModelRule] = &[
    // DALL·E 3 没有 edits 端点（只有 dall-e-2 和 gpt-image-1 支持图生图/蒙版），
    // 带参考图打到 /v1/images/edits 会被上游 400。
    ModelRule {
        protocol: Protocol::OpenAiImages,
        prefix: "dall-e-3",
        sizes: Some(DALL_E3_SIZES),
        quality: Some(DALL_E3_QUALITY),
        refs: Some(false),
        mask: Some(false),
        ..ModelRule::none()
    },
    ModelRule {
        protocol: Protocol::OpenAiImages,
        prefix: "gpt-image",
        sizes: Some(GPT_IMAGE_SIZES),
        quality: Some(GPT_IMAGE_QUALITY),
        refs: Some(true),
        mask: Some(true),
        ..ModelRule::none()
    },
    // Imagen 走 :predict 端点，不是 :generateContent
    ModelRule {
        protocol: Protocol::GeminiNative,
        prefix: "imagen",
        seed: Some(false),
        refs: Some(false),
        mask: Some(false),
        endpoint: Some(ENDPOINT_PREDICT),
        ..ModelRule::none()
    },
];

/// 协议默认能力。新增协议时在这里加一个分支。
fn protocol_defaults(protocol: Protocol) -> CapsSpec {
    match protocol {
        Protocol::OpenAiImages => CapsSpec {
            sizes: RELAY_SIZES,
            quality: GPT_IMAGE_QUALITY,
            // Images API 规范里没有 seed 字段，官方与转 OpenAI 的中转站都会 400
            seed: false,
            refs: true,
            mask: true,
            endpoint: ENDPOINT_IMAGES,
            size_label: "尺寸",
        },
        Protocol::OpenAiChatImage => CapsSpec {
            sizes: &[],
            quality: &[],
            // Chat Completions 的标准参数：支持的模型生效，不支持的忽略而非报错
            seed: true,
            refs: true,
            mask: false,
            endpoint: ENDPOINT_CHAT,
            size_label: "尺寸",
        },
        Protocol::GeminiNative => CapsSpec {
            sizes: GEMINI_RATIOS,
            quality: &[],
            seed: true,
            refs: true,
            mask: false,
            endpoint: ENDPOINT_GENERATE_CONTENT,
            size_label: "画面比例",
        },
        Protocol::MidjourneyProxy => CapsSpec {
            sizes: &[],
            quality: &[],
            seed: false,
            refs: true,
            mask: false,
            endpoint: ENDPOINT_MJ,
            size_label: "尺寸",
        },
        Protocol::VolcengineArk => CapsSpec {
            sizes: ARK_SIZES,
            quality: &[],
            seed: false,
            refs: true,
            mask: false,
            endpoint: ENDPOINT_ARK,
            size_label: "分辨率",
        },
        Protocol::DashScope => CapsSpec {
            sizes: DASHSCOPE_SIZES,
            quality: &[],
            seed: false,
            // 百炼的 image-synthesis 是文生图端点，不收参考图（图生图/编辑是另一套端点）。
            // 如实标成不支持，前端才不会给用户一个必然被上游拒掉的参考图入口。
            refs: false,
            mask: false,
            endpoint: ENDPOINT_DASHSCOPE,
            size_label: "尺寸",
        },
    }
}

/// 推导某个协议+模型的能力。模型名匹配不到就走宽松兜底行：
/// 宁可让上游报错，也不静默吞掉用户设的参数。
pub fn for_model(protocol: Protocol, model: &str) -> Capabilities {
    let m = normalize_model(model);
    let mut spec = protocol_defaults(protocol);
    if let Some(rule) = MODEL_RULES
        .iter()
        .find(|r| r.protocol == protocol && m.starts_with(r.prefix))
    {
        if let Some(s) = rule.sizes {
            spec.sizes = s;
        }
        if let Some(q) = rule.quality {
            spec.quality = q;
        }
        if let Some(v) = rule.seed {
            spec.seed = v;
        }
        if let Some(v) = rule.refs {
            spec.refs = v;
        }
        if let Some(v) = rule.mask {
            spec.mask = v;
        }
        if let Some(v) = rule.endpoint {
            spec.endpoint = v;
        }
        if let Some(v) = rule.size_label {
            spec.size_label = v;
        }
    }
    Capabilities {
        sizes: spec.sizes.iter().map(|s| s.to_string()).collect(),
        quality: spec.quality.iter().map(|s| s.to_string()).collect(),
        seed: spec.seed,
        refs: spec.refs,
        mask: spec.mask,
        endpoint: spec.endpoint.to_string(),
        size_label: spec.size_label.to_string(),
    }
}

/// 按能力表裁剪请求：丢弃该协议/模型不支持的参数（含参考图）。
/// GUI 提交与 CLI 入口都走这里，保证任何一条链路都发不出非法参数。
pub fn sanitize(caps: &Capabilities, req: &mut GenerateRequest) {
    req.size = req.size.take().filter(|s| caps.allows_size(s));
    req.quality = req.quality.take().filter(|q| caps.allows_quality(q));
    if !caps.seed {
        req.seed = None;
    }
    if !caps.mask {
        req.mask = None;
    }
    if !caps.refs {
        req.refs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::types::{GenerateRequest, RefImage};

    fn req() -> GenerateRequest {
        GenerateRequest {
            provider_id: "p1".into(),
            model_id: None,
            prompt: "cat".into(),
            n: 1,
            size: Some("2048x2048".into()),
            quality: Some("hd".into()),
            seed: Some(7),
            refs: vec![],
            mask: Some(RefImage {
                name: "mask.png".into(),
                mime: "image/png".into(),
                data: "aGk=".into(),
            }),
            session_id: None,
        }
    }

    #[test]
    fn gpt_image_has_no_seed_and_no_2048() {
        let c = for_model(Protocol::OpenAiImages, "gpt-image-1");
        assert!(!c.seed, "Images API 没有 seed 字段");
        assert!(!c.allows_size("2048x2048"));
        assert!(c.mask && c.refs);
        assert_eq!(c.endpoint, "images");
        assert_eq!(c.size_label, "尺寸");
    }

    #[test]
    fn dall_e3_uses_its_own_sizes_and_quality() {
        let c = for_model(Protocol::OpenAiImages, "dall-e-3");
        assert!(c.allows_size("1792x1024"));
        assert!(c.allows_size("1024x1792"));
        assert_eq!(c.quality, vec!["standard", "hd"]);
    }

    #[test]
    fn dall_e3_has_no_edits_endpoint() {
        let c = for_model(Protocol::OpenAiImages, "dall-e-3");
        assert!(!c.refs && !c.mask, "DALL·E 3 不支持 /v1/images/edits");
    }

    #[test]
    fn dall_e2_and_gpt_image_support_edits() {
        assert!(for_model(Protocol::OpenAiImages, "dall-e-2").mask);
        assert!(for_model(Protocol::OpenAiImages, "gpt-image-1").refs);
    }

    #[test]
    fn unknown_openai_model_falls_back_to_relay_row() {
        let c = for_model(Protocol::OpenAiImages, "flux-1.1-pro");
        assert!(c.allows_size("2048x2048"));
        assert!(!c.seed);
        assert!(c.mask);
    }

    #[test]
    fn chat_supports_seed_but_not_size_quality_or_mask() {
        let c = for_model(Protocol::OpenAiChatImage, "gpt-4o-image");
        assert!(c.seed && c.refs);
        assert!(c.sizes.is_empty() && c.quality.is_empty());
        assert!(!c.mask);
        assert_eq!(c.endpoint, "chat");
    }

    #[test]
    fn imagen_uses_predict_endpoint_without_refs() {
        let c = for_model(Protocol::GeminiNative, "imagen-4.0-generate-001");
        assert_eq!(c.endpoint, "predict");
        assert!(!c.seed && !c.refs && !c.mask);
        assert_eq!(c.size_label, "画面比例");
        assert!(c.allows_size("16:9"));
    }

    #[test]
    fn gemini_image_model_uses_generate_content() {
        let c = for_model(Protocol::GeminiNative, "gemini-2.5-flash-image");
        assert_eq!(c.endpoint, "generate_content");
        assert!(c.seed && c.refs);
        assert!(!c.mask);
    }

    #[test]
    fn model_name_is_normalized() {
        // 大小写不敏感 + 去掉 models/ 前缀
        assert_eq!(
            for_model(Protocol::GeminiNative, "MODELS/Imagen-4.0").endpoint,
            "predict"
        );
        assert_eq!(
            for_model(Protocol::OpenAiImages, "GPT-Image-1").endpoint,
            "images"
        );
    }

    #[test]
    fn mj_supports_refs_only() {
        let c = for_model(Protocol::MidjourneyProxy, "midjourney");
        assert!(c.refs);
        assert!(!c.seed && !c.mask);
        assert!(c.sizes.is_empty() && c.quality.is_empty());
        assert_eq!(c.endpoint, "mj");
    }

    #[test]
    fn ark_uses_resolution_tiers_and_no_seed() {
        let c = for_model(Protocol::VolcengineArk, "doubao-seedream-5-5-261028");
        assert_eq!(c.endpoint, "ark");
        assert!(c.allows_size("2K") && c.allows_size("1K"));
        assert!(!c.allows_size("1024x1024"), "方舟用档位不是像素尺寸");
        assert!(!c.seed, "方舟图像接口没有 seed 字段");
        assert!(c.refs && !c.mask);
        assert_eq!(c.size_label, "分辨率");
    }

    #[test]
    fn dashscope_uses_star_separated_sizes() {
        let c = for_model(Protocol::DashScope, "wan2.6-t2i");
        assert_eq!(c.endpoint, "dashscope");
        assert!(c.allows_size("1024*1024"));
        assert!(!c.allows_size("1024x1024"), "百炼尺寸用星号分隔");
        assert!(!c.seed);
        // image-synthesis 是文生图端点，不收参考图
        assert!(!c.refs && !c.mask);
    }

    #[test]
    fn override_replaces_only_given_fields() {
        let mut c = for_model(Protocol::OpenAiImages, "gpt-image-1");
        let o = CapabilityOverride {
            sizes: Some(vec!["512x512".into()]),
            seed: Some(true),
            ..Default::default()
        };
        c.apply_override(&o);
        assert!(c.allows_size("512x512"));
        assert!(!c.allows_size("1024x1024"), "覆写后原尺寸表应被替换");
        assert!(c.seed, "覆写可开启 seed");
        assert_eq!(
            c.quality,
            vec!["auto", "low", "medium", "high"],
            "未覆写字段保留"
        );
    }

    #[test]
    fn parse_override_handles_bad_json_as_no_override() {
        assert!(parse_override(None).is_none());
        assert!(parse_override(Some("   ")).is_none());
        assert!(parse_override(Some("{不是json")).is_none());
        let o = parse_override(Some(r#"{"seed":true}"#)).unwrap();
        assert_eq!(o.seed, Some(true));
        assert!(o.sizes.is_none());
    }

    #[test]
    fn sanitize_drops_unsupported_fields() {
        let mut r = req();
        let caps = for_model(Protocol::OpenAiImages, "gpt-image-1");
        sanitize(&caps, &mut r);
        assert!(r.size.is_none(), "2048x2048 不在 gpt-image-1 的尺寸表内");
        assert!(r.quality.is_none(), "hd 不在 gpt-image-1 的质量表内");
        assert!(r.seed.is_none(), "Images API 不支持 seed");
        // 蒙版是 gpt-image-1 支持的能力，应当保留
        assert!(r.mask.is_some());
    }

    #[test]
    fn sanitize_drops_mask_when_unsupported() {
        let mut r = req();
        let caps = for_model(Protocol::GeminiNative, "gemini-2.5-flash-image");
        sanitize(&caps, &mut r);
        assert!(r.mask.is_none(), "gemini 不支持蒙版");
        assert_eq!(r.seed, Some(7), "gemini 支持 seed");
    }

    #[test]
    fn sanitize_keeps_supported_fields() {
        let mut r = req();
        r.size = Some("1024x1024".into());
        r.quality = Some("high".into());
        let caps = for_model(Protocol::OpenAiImages, "gpt-image-1");
        sanitize(&caps, &mut r);
        assert_eq!(r.size.as_deref(), Some("1024x1024"));
        assert_eq!(r.quality.as_deref(), Some("high"));
        assert!(r.mask.is_some());
        // seed 对 gpt-image-1 仍不支持
        assert!(r.seed.is_none());
    }

    #[test]
    fn sanitize_drops_refs_when_unsupported() {
        let mut r = req();
        r.refs.push(RefImage {
            name: "ref.png".into(),
            mime: "image/png".into(),
            data: "aGk=".into(),
        });
        let caps = for_model(Protocol::OpenAiImages, "dall-e-3");
        sanitize(&caps, &mut r);
        assert!(r.refs.is_empty(), "dall-e-3 不支持参考图，应被裁掉");
    }

    #[test]
    fn sanitize_keeps_refs_when_supported() {
        let mut r = req();
        r.refs.push(RefImage {
            name: "ref.png".into(),
            mime: "image/png".into(),
            data: "aGk=".into(),
        });
        let caps = for_model(Protocol::OpenAiImages, "gpt-image-1");
        sanitize(&caps, &mut r);
        assert_eq!(r.refs.len(), 1, "gpt-image-1 支持参考图，应保留");
    }

    #[test]
    fn sanitize_keeps_gemini_seed_and_ratio() {
        let mut r = req();
        r.size = Some("16:9".into());
        r.quality = None;
        r.mask = None;
        let caps = for_model(Protocol::GeminiNative, "gemini-2.5-flash-image");
        sanitize(&caps, &mut r);
        assert_eq!(r.size.as_deref(), Some("16:9"));
        assert_eq!(r.seed, Some(7));
    }
}
