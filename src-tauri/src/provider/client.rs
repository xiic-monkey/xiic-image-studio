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

/// 拼接 URL：base + path，容忍 base 末尾带 / 或带 /v1
pub fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    // 用户习惯把 /v1 直接填在 baseUrl 里，这里做归一化
    let base = if base.ends_with("/v1") && !path.starts_with("/v1") {
        base.trim_end_matches("/v1").to_string()
    } else {
        base.to_string()
    };
    format!("{base}{path}")
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
        Protocol::OpenAiImages | Protocol::OpenAiChatImage => {
            (join_url(&base, "/v1/models"), true)
        }
        Protocol::GeminiNative => (join_url(&base, "/v1beta/models"), false),
        Protocol::MidjourneyProxy => (join_url(&base, "/mj/task/list"), true),
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
            TestResult { ok, status: Some(status), message, latency_ms: latency }
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
            models: vec!["midjourney".into(), "midjourney-relax".into(), "midjourney-fast".into(), "midjourney-turbo".into()],
            message: "MJ-Proxy 固定模型名".into(),
        }),
    }
}
