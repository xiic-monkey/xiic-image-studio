use crate::generate::types::GenerateRequest;
use crate::provider::types::Protocol;
use serde::Serialize;

/// 某个「协议 + 模型」实际接受的参数集合。唯一真源：
/// 适配器据此发参、前端据此收敛控件、提交入口据此裁剪请求。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// 允许的尺寸取值；空 = 该协议/模型不接受尺寸参数
    pub sizes: &'static [&'static str],
    /// 允许的质量取值；空 = 不接受 quality
    pub quality: &'static [&'static str],
    pub seed: bool,
    pub refs: bool,
    pub mask: bool,
    /// 上游端点形态：images | chat | generate_content | predict | mj
    pub endpoint: &'static str,
    /// 前端尺寸控件标签：尺寸 | 画面比例
    pub size_label: &'static str,
}

pub const ENDPOINT_IMAGES: &str = "images";
pub const ENDPOINT_CHAT: &str = "chat";
pub const ENDPOINT_GENERATE_CONTENT: &str = "generate_content";
pub const ENDPOINT_PREDICT: &str = "predict";
pub const ENDPOINT_MJ: &str = "mj";

const GPT_IMAGE_SIZES: &[&str] = &["1024x1024", "1536x1024", "1024x1536"];
const DALL_E3_SIZES: &[&str] = &["1024x1024", "1792x1024", "1024x1792"];
const RELAY_SIZES: &[&str] = &["1024x1024", "1536x1024", "1024x1536", "2048x2048"];
const GPT_IMAGE_QUALITY: &[&str] = &["auto", "low", "medium", "high"];
const DALL_E3_QUALITY: &[&str] = &["standard", "hd"];
const GEMINI_RATIOS: &[&str] = &["1:1", "3:4", "4:3", "9:16", "16:9"];

fn normalize_model(model: &str) -> String {
    // 先小写再剥前缀：`MODELS/Imagen-4.0` 也要归一成 `imagen-4.0`
    model
        .trim()
        .to_ascii_lowercase()
        .trim_start_matches("models/")
        .to_string()
}

/// 推导某个协议+模型的能力。模型名匹配不到就走宽松兜底行：
/// 宁可让上游报错，也不静默吞掉用户设的参数。
pub fn for_model(protocol: Protocol, model: &str) -> Capabilities {
    let m = normalize_model(model);
    match protocol {
        Protocol::OpenAiImages => {
            // DALL·E 3 没有 edits 端点（只有 dall-e-2 和 gpt-image-1 支持图生图/蒙版），
            // 带参考图打到 /v1/images/edits 会被上游 400。
            let (sizes, quality, refs, mask) = if m.starts_with("dall-e-3") {
                (DALL_E3_SIZES, DALL_E3_QUALITY, false, false)
            } else if m.starts_with("gpt-image") {
                (GPT_IMAGE_SIZES, GPT_IMAGE_QUALITY, true, true)
            } else {
                (RELAY_SIZES, GPT_IMAGE_QUALITY, true, true)
            };
            Capabilities {
                sizes,
                quality,
                // Images API 规范里没有 seed 字段，官方与转 OpenAI 的中转站都会 400
                seed: false,
                refs,
                mask,
                endpoint: ENDPOINT_IMAGES,
                size_label: "尺寸",
            }
        }
        Protocol::OpenAiChatImage => Capabilities {
            sizes: &[],
            quality: &[],
            // Chat Completions 的标准参数：支持的模型生效，不支持的忽略而非报错
            seed: true,
            refs: true,
            mask: false,
            endpoint: ENDPOINT_CHAT,
            size_label: "尺寸",
        },
        Protocol::GeminiNative => {
            if m.starts_with("imagen") {
                Capabilities {
                    sizes: GEMINI_RATIOS,
                    quality: &[],
                    seed: false,
                    refs: false,
                    mask: false,
                    endpoint: ENDPOINT_PREDICT,
                    size_label: "画面比例",
                }
            } else {
                Capabilities {
                    sizes: GEMINI_RATIOS,
                    quality: &[],
                    seed: true,
                    refs: true,
                    mask: false,
                    endpoint: ENDPOINT_GENERATE_CONTENT,
                    size_label: "画面比例",
                }
            }
        }
        Protocol::MidjourneyProxy => Capabilities {
            sizes: &[],
            quality: &[],
            seed: false,
            refs: true,
            mask: false,
            endpoint: ENDPOINT_MJ,
            size_label: "尺寸",
        },
    }
}

/// 按能力表裁剪请求：丢弃该协议/模型不支持的参数。
/// GUI 提交与 MCP 入口都走这里，保证任何一条链路都发不出非法参数。
pub fn sanitize(caps: &Capabilities, req: &mut GenerateRequest) {
    req.size = req.size.take().filter(|s| caps.sizes.contains(&s.as_str()));
    req.quality = req
        .quality
        .take()
        .filter(|q| caps.quality.contains(&q.as_str()));
    if !caps.seed {
        req.seed = None;
    }
    if !caps.mask {
        req.mask = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::types::{GenerateRequest, RefImage};

    fn req() -> GenerateRequest {
        GenerateRequest {
            provider_id: "p1".into(),
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
        assert!(!c.sizes.contains(&"2048x2048"));
        assert!(c.mask && c.refs);
        assert_eq!(c.endpoint, "images");
        assert_eq!(c.size_label, "尺寸");
    }

    #[test]
    fn dall_e3_uses_its_own_sizes_and_quality() {
        let c = for_model(Protocol::OpenAiImages, "dall-e-3");
        assert!(c.sizes.contains(&"1792x1024"));
        assert!(c.sizes.contains(&"1024x1792"));
        assert_eq!(c.quality, &["standard", "hd"][..]);
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
        assert!(c.sizes.contains(&"2048x2048"));
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
        assert!(c.sizes.contains(&"16:9"));
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
