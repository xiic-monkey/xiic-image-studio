use crate::error::{AppError, AppResult};
use crate::provider::types::*;
use reqwest::Client;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Instant;

/// 探活 / 模型列表 / 轮询等"秒回"请求用：20 秒足够，久等即是异常。
pub fn http_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("xiic-image-studio/0.1")
        .build()
        .expect("failed to build http client")
}

/// 同步出图请求（images/generations、chat 生图、Gemini 原生）用：
/// 出图本身要几十秒到几分钟，20 秒必然误杀，这里给足 10 分钟。
///
/// 注意配套约定：超时（AppError::Timeout）**不重试**——请求已送达上游，
/// 上游多半还在出图，重试只会重复扣费并让上游并发叠加（一个任务打出 2~3 个并发）。
pub fn image_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .user_agent("xiic-image-studio/0.1")
        .build()
        .expect("failed to build http client")
}

/// 拼接 URL：base + path，容忍 base 末尾带 / 或已带与 path 相同的首个路径段
pub fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    let first_path_segment = path.trim_start_matches('/').split('/').next().unwrap_or("");
    let last_base_segment = base.rsplit('/').next().unwrap_or("");
    // 用户的 baseUrl 经常已经带了 /v1 或 /v1beta；path 也带同一段时只保留一份。
    let base = if !first_path_segment.is_empty() && first_path_segment == last_base_segment {
        base.strip_suffix(last_base_segment)
            .unwrap_or(base)
            .trim_end_matches('/')
    } else {
        base
    };
    format!("{base}{path}")
}

/// 拼「base 可能已带版本前缀」的 URL：base 结尾已有 prefix 就不重复加。
/// 方舟的 base 既可能填 "https://ark.cn-beijing.volces.com"，
/// 也可能照抄文档填 ".../api/v3"，两种都得能拼对。
pub fn join_with_prefix(base: &str, prefix: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.ends_with(prefix) {
        format!("{base}{path}")
    } else {
        format!("{base}{prefix}{path}")
    }
}

pub fn apply_headers(
    mut rb: reqwest::RequestBuilder,
    custom: &BTreeMap<String, String>,
) -> reqwest::RequestBuilder {
    for (k, v) in custom {
        rb = rb.header(k, v);
    }
    rb
}

fn openai_auth(rb: reqwest::RequestBuilder, key: &str) -> reqwest::RequestBuilder {
    rb.bearer_auth(key)
}

/// 按协议发一次轻量探活请求，返回 (status, body)
async fn probe(draft: &ProviderDraft, key: &str) -> AppResult<(u16, String)> {
    let client = http_client();
    let base = draft.base_url.trim().to_string();
    if base.is_empty() {
        return Err(AppError::General("baseUrl 不能为空".into()));
    }

    let (url, authed) = match draft.protocol {
        Protocol::OpenAiImages | Protocol::OpenAiChatImage => (join_url(&base, "/v1/models"), true),
        Protocol::GeminiNative => (join_url(&base, "/v1beta/models"), false),
        Protocol::MidjourneyProxy => (join_url(&base, "/mj/task/list"), true),
        Protocol::VolcengineArk => (join_with_prefix(&base, "/api/v3", "/models"), true),
        Protocol::DashScope => (join_with_prefix(&base, "/api/v1", "/models"), true),
    };

    let mut rb = client.get(&url);
    rb = apply_headers(rb, &draft.custom_headers);
    match draft.protocol {
        Protocol::GeminiNative => {
            rb = rb.header("x-goog-api-key", key);
        }
        _ if authed => {
            rb = openai_auth(rb, key);
        }
        _ => {}
    }

    let resp = rb.send().await?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    Ok((status, body))
}

pub async fn test_connection(draft: &ProviderDraft, key: &str) -> TestResult {
    let start = Instant::now();
    match probe(draft, key).await {
        Ok((status, body)) => {
            let latency = Some(start.elapsed().as_millis() as u64);
            let ok = (200..300).contains(&status);
            let message = if ok {
                "连接成功".to_string()
            } else if status == 401 || status == 403 {
                format!("鉴权失败（HTTP {status}），请检查 API Key")
            } else if status == 404 {
                format!(
                    "端点不存在（HTTP 404），请确认 baseUrl 与协议匹配（当前协议 {}）",
                    draft.protocol.as_str()
                )
            } else {
                let brief = body.chars().take(200).collect::<String>();
                format!("HTTP {status}: {brief}")
            };
            TestResult {
                ok,
                status: Some(status),
                message,
                latency_ms: latency,
            }
        }
        Err(e) => TestResult {
            ok: false,
            status: None,
            message: format!("请求失败: {e}"),
            latency_ms: None,
        },
    }
}

/// 拉取上游模型列表
pub async fn discover_models(draft: &ProviderDraft, key: &str) -> AppResult<DiscoveredModels> {
    match draft.protocol {
        Protocol::OpenAiImages | Protocol::OpenAiChatImage => {
            let client = http_client();
            let url = join_url(draft.base_url.trim(), "/v1/models");
            let rb = apply_headers(openai_auth(client.get(&url), key), &draft.custom_headers);
            let resp = rb.send().await?;
            let status = resp.status();
            let body: Value = resp.json().await?;
            if !status.is_success() {
                return Err(AppError::Http(format!(
                    "HTTP {}: {}",
                    status,
                    body.to_string().chars().take(300).collect::<String>()
                )));
            }
            let mut models: Vec<String> = body["data"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            models.sort();
            let n = models.len();
            Ok(DiscoveredModels {
                models,
                message: format!("发现 {n} 个模型（含非生图模型，请自行挑选）"),
            })
        }
        Protocol::GeminiNative => {
            let client = http_client();
            let url = join_url(draft.base_url.trim(), "/v1beta/models");
            let rb = apply_headers(
                client.get(&url).header("x-goog-api-key", key),
                &draft.custom_headers,
            );
            let resp = rb.send().await?;
            let status = resp.status();
            let body: Value = resp.json().await?;
            if !status.is_success() {
                return Err(AppError::Http(format!(
                    "HTTP {}: {}",
                    status,
                    body.to_string().chars().take(300).collect::<String>()
                )));
            }
            let mut models: Vec<String> = body["models"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["name"].as_str())
                        .map(|n| n.trim_start_matches("models/").to_string())
                        .filter(|n| n.contains("image") || n.contains("imagen"))
                        .collect()
                })
                .unwrap_or_default();
            models.sort();
            let n = models.len();
            Ok(DiscoveredModels {
                models,
                message: format!("发现 {n} 个图像模型"),
            })
        }
        Protocol::MidjourneyProxy => Ok(DiscoveredModels {
            models: vec![
                "midjourney".into(),
                "midjourney-relax".into(),
                "midjourney-fast".into(),
                "midjourney-turbo".into(),
            ],
            message: "MJ-Proxy 固定模型名".into(),
        }),
        // 方舟与百炼的模型列表都是 OpenAI 风格的 data 数组，只是字段名不完全一致，
        // 这里对常见字段名都做兼容，避免因为字段名差异就一个模型都发现不了。
        Protocol::VolcengineArk | Protocol::DashScope => {
            let client = http_client();
            let base = draft.base_url.trim();
            let url = if draft.protocol == Protocol::VolcengineArk {
                join_with_prefix(base, "/api/v3", "/models")
            } else {
                join_with_prefix(base, "/api/v1", "/models")
            };
            let rb = apply_headers(openai_auth(client.get(&url), key), &draft.custom_headers);
            let resp = rb.send().await?;
            let status = resp.status();
            let body: Value = resp.json().await?;
            if !status.is_success() {
                return Err(AppError::Http(format!(
                    "HTTP {}: {}",
                    status,
                    body.to_string().chars().take(300).collect::<String>()
                )));
            }
            let mut models: Vec<String> = body["data"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| {
                            m.get("id")
                                .and_then(|v| v.as_str())
                                .or_else(|| m.get("model_id").and_then(|v| v.as_str()))
                                .or_else(|| m.get("model").and_then(|v| v.as_str()))
                                .or_else(|| m.get("name").and_then(|v| v.as_str()))
                                .map(|s| s.to_string())
                        })
                        .collect()
                })
                .unwrap_or_default();
            // 平台常把文本模型一并返回，图像模型名里基本都带 wan / image / seedream
            let images_only: Vec<String> = models
                .iter()
                .filter(|m| {
                    let lower = m.to_ascii_lowercase();
                    lower.contains("image")
                        || lower.contains("wan")
                        || lower.contains("seedream")
                        || lower.contains("imagen")
                })
                .cloned()
                .collect();
            if !images_only.is_empty() {
                models = images_only;
            }
            models.sort();
            let n = models.len();
            Ok(DiscoveredModels {
                models,
                message: format!("发现 {n} 个模型（含非生图模型，请自行挑选）"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{join_url, join_with_prefix};

    #[test]
    fn join_with_prefix_adds_version_path_when_missing() {
        assert_eq!(
            join_with_prefix(
                "https://ark.cn-beijing.volces.com",
                "/api/v3",
                "/images/generations"
            ),
            "https://ark.cn-beijing.volces.com/api/v3/images/generations"
        );
    }

    #[test]
    fn join_with_prefix_does_not_duplicate_existing_prefix() {
        // 用户照抄文档把 /api/v3 写进 base 时不该拼成 /api/v3/api/v3
        assert_eq!(
            join_with_prefix(
                "https://ark.cn-beijing.volces.com/api/v3",
                "/api/v3",
                "/images/generations"
            ),
            "https://ark.cn-beijing.volces.com/api/v3/images/generations"
        );
        assert_eq!(
            join_with_prefix(
                "https://ark.cn-beijing.volces.com/",
                "/api/v3",
                "/images/generations"
            ),
            "https://ark.cn-beijing.volces.com/api/v3/images/generations"
        );
    }

    #[test]
    fn base_with_v1_does_not_duplicate_version_segment() {
        assert_eq!(
            join_url("https://api.openai.com/v1", "/v1/images/generations"),
            "https://api.openai.com/v1/images/generations"
        );
    }

    #[test]
    fn base_with_v1beta_does_not_duplicate_version_segment() {
        assert_eq!(
            join_url(
                "https://generativelanguage.googleapis.com/v1beta",
                "/v1beta/models/gemini-2.5-flash-image:generateContent"
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent"
        );
    }

    #[test]
    fn trailing_slash_on_base_is_ignored() {
        assert_eq!(
            join_url("https://api.openai.com/v1/", "/v1/images/generations"),
            "https://api.openai.com/v1/images/generations"
        );
    }

    #[test]
    fn unversioned_base_still_gets_version_path() {
        assert_eq!(
            join_url("https://api.openai.com", "/v1/images/generations"),
            "https://api.openai.com/v1/images/generations"
        );
    }

    #[test]
    fn unrelated_base_path_is_preserved() {
        assert_eq!(
            join_url("https://proxy.example.com/openai", "/v1/images/generations"),
            "https://proxy.example.com/openai/v1/images/generations"
        );
    }

    #[test]
    fn mj_base_and_path_join_without_duplication() {
        assert_eq!(
            join_url("https://mj.example.com", "/mj/submit/imagine"),
            "https://mj.example.com/mj/submit/imagine"
        );
        assert_eq!(
            join_url("https://mj.example.com/mj", "/mj/submit/imagine"),
            "https://mj.example.com/mj/submit/imagine"
        );
    }
}
