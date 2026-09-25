use crate::error::{AppError, AppResult};
use crate::generate::types::{GenCtx, ImageOut};
use base64::Engine;
use serde_json::{json, Value};

use super::client::{apply_headers, http_client, image_client, join_url};

fn b64_decode(s: &str) -> AppResult<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .map_err(|e| AppError::General(format!("base64 解码失败: {e}")))
}

fn data_url(mime: &str, data: &str) -> String {
    format!("data:{mime};base64,{}", data.trim())
}

/// 发请求并按 HTTP 状态解析。
///
/// 非 2xx 一律转成带状态码的 `AppError::Status`：上游 5xx 常直接返回 HTML 错误页，
/// 原来的 `.json()` 会把它变成 "error decoding response body" 这种看不懂的报错，
/// 而且因为看不出状态码，所有错误都会被无差别重试。
async fn send_json(rb: reqwest::RequestBuilder) -> AppResult<Value> {
    let resp = rb.send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !status.is_success() {
        let msg = body
            .get("error")
            .and_then(|e| {
                e.get("message")
                    .and_then(|m| m.as_str())
                    .or_else(|| e.as_str())
            })
            .map(|s| s.to_string())
            .unwrap_or_else(|| text.chars().take(300).collect());
        return Err(AppError::Status(status.as_u16(), msg));
    }
    Ok(body)
}

/// 从任意响应数据里拿一张图：b64_json / url / data url
async fn image_from_data(v: &Value) -> AppResult<ImageOut> {
    if let Some(b64) = v.get("b64_json").and_then(|x| x.as_str()) {
        return Ok(ImageOut::new(b64_decode(b64)?, "image/png"));
    }
    if let Some(url) = v.get("url").and_then(|x| x.as_str()) {
        return download_url(url).await;
    }
    Err(AppError::General("响应中没有图片数据（b64_json/url 均缺失）".into()))
}

async fn download_url(url: &str) -> AppResult<ImageOut> {
    if let Some(rest) = url.strip_prefix("data:") {
        let (head, data) = rest.split_once(',').ok_or_else(|| AppError::General("data url 格式错误".into()))?;
        let mime = head.split(';').next().unwrap_or("image/png").to_string();
        return Ok(ImageOut { bytes: b64_decode(data)?, mime, meta: None });
    }
    let client = image_client();
    let resp = client.get(url).send().await?;
    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .split(';')
        .next()
        .unwrap_or("image/png")
        .to_string();
    if !resp.status().is_success() {
        return Err(AppError::Http(format!("下载图片失败 HTTP {}", resp.status())));
    }
    let bytes = resp.bytes().await?.to_vec();
    Ok(ImageOut { bytes, mime, meta: None })
}

/// 从文本中提取图片 URL（markdown 图片 / 裸链接 / data url）
fn extract_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'!' && text[i..].starts_with("![") {
            if let Some(open) = text[i..].find("](") {
                let start = i + open + 2;
                if let Some(close) = text[start..].find(')') {
                    let u = text[start..start + close].trim();
                    if !u.is_empty() {
                        urls.push(u.to_string());
                    }
                    i = start + close;
                    continue;
                }
            }
        }
        i += 1;
    }
    if urls.is_empty() {
        for token in text.split_whitespace() {
            let t = token.trim_matches(|c| c == ')' || c == '"' || c == '\'');
            if t.starts_with("http://") || t.starts_with("https://") || t.starts_with("data:image") {
                urls.push(t.to_string());
            }
        }
    }
    urls
}

// ---------- OpenAI Images ----------

pub async fn openai_images(ctx: &GenCtx) -> AppResult<ImageOut> {
    let client = image_client();
    let (path, is_multipart) = if ctx.refs.is_empty() {
        ("/v1/images/generations", false)
    } else {
        ("/v1/images/edits", true)
    };
    let url = join_url(&ctx.base_url, path);

    let rb = apply_headers(client.post(&url).bearer_auth(&ctx.key), &ctx.custom_headers);

    let resp_body: Value = if is_multipart {
        let mut form = reqwest::multipart::Form::new()
            .text("model", ctx.model.clone())
            .text("prompt", ctx.prompt.clone())
            .text("n", "1");
        if let Some(size) = &ctx.size {
            form = form.text("size", size.clone());
        }
        if let Some(q) = &ctx.quality {
            form = form.text("quality", q.clone());
        }
        if let Some(s) = ctx.seed {
            form = form.text("seed", s.to_string());
        }
        for r in &ctx.refs {
            let file = reqwest::multipart::Part::bytes(b64_decode(&r.data)?)
                .file_name(if r.name.is_empty() { "ref.png".into() } else { r.name.clone() })
                .mime_str(&r.mime)?;
            form = form.part("image[]", file);
        }
        if let Some(mask) = &ctx.mask {
            let file = reqwest::multipart::Part::bytes(b64_decode(&mask.data)?)
                .file_name("mask.png")
                .mime_str("image/png")?;
            form = form.part("mask", file);
        }
        send_json(rb.multipart(form)).await?
    } else {
        let mut body = json!({
            "model": ctx.model,
            "prompt": ctx.prompt,
            "n": 1,
        });
        if let Some(size) = &ctx.size {
            body["size"] = json!(size);
        }
        if let Some(q) = &ctx.quality {
            body["quality"] = json!(q);
        }
        if let Some(s) = ctx.seed {
            body["seed"] = json!(s);
        }
        send_json(rb.json(&body)).await?
    };

    if let Some(err) = resp_body.get("error") {
        return Err(AppError::Upstream(format!(
            "上游报错: {}",
            err["message"].as_str().unwrap_or(&err.to_string())
        )));
    }
    let first = resp_body["data"]
        .as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| AppError::General("上游返回为空".into()))?;
    image_from_data(first).await
}

// ---------- OpenAI Chat 生图 ----------

pub async fn openai_chat_image(ctx: &GenCtx) -> AppResult<ImageOut> {
    let client = image_client();
    let url = join_url(&ctx.base_url, "/v1/chat/completions");

    let mut content: Vec<Value> = vec![json!({"type": "text", "text": ctx.prompt})];
    for r in &ctx.refs {
        content.push(json!({
            "type": "image_url",
            "image_url": {"url": data_url(&r.mime, &r.data)}
        }));
    }
    let mut body = json!({
        "model": ctx.model,
        "messages": [{"role": "user", "content": content}],
    });
    if let Some(s) = ctx.seed {
        body["seed"] = json!(s);
    }

    let rb = apply_headers(client.post(&url).bearer_auth(&ctx.key), &ctx.custom_headers);
    let resp_body: Value = send_json(rb.json(&body)).await?;

    if let Some(err) = resp_body.get("error") {
        return Err(AppError::Upstream(format!(
            "上游报错: {}",
            err["message"].as_str().unwrap_or(&err.to_string())
        )));
    }
    let msg = &resp_body["choices"][0]["message"];

    // 1) message.images[] (new-api / one-api 扩展格式)
    if let Some(imgs) = msg.get("images").and_then(|x| x.as_array()) {
        for img in imgs {
            let url = img
                .get("image_url")
                .and_then(|u| u.get("url"))
                .and_then(|u| u.as_str())
                .or_else(|| img.as_str());
            if let Some(u) = url {
                return download_url(u).await;
            }
        }
    }
    // 2) content 文本中提取
    let text = match msg.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    for u in extract_urls(&text) {
        if u.starts_with("data:image") || u.contains("http") {
            return download_url(&u).await;
        }
    }
    Err(AppError::General(format!(
        "回复中未解析到图片，原文: {}",
        text.chars().take(300).collect::<String>()
    )))
}

// ---------- Gemini 原生 ----------

pub async fn gemini_native(ctx: &GenCtx) -> AppResult<ImageOut> {
    let client = image_client();
    let model = ctx.model.trim_start_matches("models/");
    let url = join_url(&ctx.base_url, &format!("/v1beta/models/{model}:generateContent"));

    let mut parts: Vec<Value> = vec![json!({"text": ctx.prompt})];
    for r in &ctx.refs {
        parts.push(json!({
            "inlineData": {"mimeType": r.mime, "data": r.data.trim()}
        }));
    }
    let mut body = json!({
        "contents": [{"parts": parts}],
    });
    if let Some(s) = ctx.seed {
        body["seed"] = json!(s);
    }

    let rb = apply_headers(
        client.post(&url).header("x-goog-api-key", &ctx.key),
        &ctx.custom_headers,
    );
    let resp_body: Value = send_json(rb.json(&body)).await?;

    if let Some(err) = resp_body.get("error") {
        return Err(AppError::Upstream(format!(
            "上游报错: {}",
            err["message"].as_str().unwrap_or(&err.to_string())
        )));
    }
    let candidates = resp_body["candidates"]
        .as_array()
        .ok_or_else(|| AppError::General(format!("无 candidates: {}", resp_body.to_string().chars().take(200).collect::<String>())))?;
    for cand in candidates {
        if let Some(parts) = cand["content"]["parts"].as_array() {
            for p in parts {
                let b64 = p["inlineData"]["data"]
                    .as_str()
                    .or_else(|| p["inline_data"]["data"].as_str());
                let mime = p["inlineData"]["mimeType"]
                    .as_str()
                    .or_else(|| p["inline_data"]["mime_type"].as_str())
                    .unwrap_or("image/png");
                if let Some(b64) = b64 {
                    return Ok(ImageOut::new(b64_decode(b64)?, mime));
                }
            }
        }
    }
    Err(AppError::General("Gemini 响应中无图片 part".into()))
}

// ---------- Midjourney Proxy ----------

pub async fn midjourney_proxy(ctx: &GenCtx) -> AppResult<ImageOut> {
    let client = http_client();
    let url = join_url(&ctx.base_url, "/mj/submit/imagine");
    let mut body = json!({"prompt": ctx.prompt});
    if !ctx.refs.is_empty() {
        body["base64Array"] = json!(ctx.refs.iter().map(|r| data_url(&r.mime, &r.data)).collect::<Vec<_>>());
    }
    let rb = apply_headers(client.post(&url).bearer_auth(&ctx.key), &ctx.custom_headers);
    let resp: Value = send_json(rb.json(&body)).await?;
    if resp["code"].as_i64() != Some(1) && resp["code"].as_str() != Some("1") {
        return Err(AppError::General(format!(
            "MJ 提交失败: {}",
            resp.to_string().chars().take(300).collect::<String>()
        )));
    }
    let task_id = resp["result"]
        .as_str()
        .ok_or_else(|| AppError::General("MJ 未返回任务 id".into()))?;

    // 轮询最多 5 分钟
    let fetch_url = join_url(&ctx.base_url, &format!("/mj/task/{task_id}/fetch"));
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        if let Some(flag) = &ctx.cancel {
            if flag.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(AppError::General("已取消".into()));
            }
        }
        let rb = apply_headers(client.get(&fetch_url).bearer_auth(&ctx.key), &ctx.custom_headers);
        let status: Value = send_json(rb).await?;
        let st = status["status"].as_str().unwrap_or("");
        match st {
            "SUCCESS" => {
                let img_url = status["imageUrl"]
                    .as_str()
                    .or_else(|| status["image_url"].as_str())
                    .ok_or_else(|| AppError::General("MJ 成功但无 imageUrl".into()))?;
                let mut out = download_url(img_url).await?;
                out.meta = Some(json!({
                    "mj_task_id": task_id,
                    "mj_status_url": format!("/mj/task/{task_id}/fetch"),
                }));
                return Ok(out);
            }
            "FAILURE" => {
                return Err(AppError::General(format!(
                    "MJ 任务失败: {}",
                    status["failReason"].as_str().unwrap_or("未知原因")
                )));
            }
            _ => continue,
        }
    }
    Err(AppError::General("MJ 任务超时（5 分钟）".into()))
}
