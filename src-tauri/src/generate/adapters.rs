use crate::capabilities;
use crate::error::{AppError, AppResult};
use crate::generate::types::{GenCtx, ImageOut};
use crate::provider::types::Protocol;
use base64::Engine;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::client::{apply_headers, http_client, image_client, join_url};

/// 测试里把轮询间隔压成 0，避免每个用例空等 3 秒。
#[cfg(test)]
const MJ_POLL_INTERVAL: Duration = Duration::ZERO;
#[cfg(not(test))]
const MJ_POLL_INTERVAL: Duration = Duration::from_secs(3);
const MJ_POLL_TIMES: usize = 100;

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
        let bytes = b64_decode(b64)
            .map_err(|e| AppError::Fetch(format!("上游图片 base64 解码失败: {e}")))?;
        return Ok(ImageOut::new(bytes, "image/png"));
    }
    if let Some(url) = v.get("url").and_then(|x| x.as_str()) {
        return download_url(url).await;
    }
    Err(AppError::Fetch("响应中没有图片数据（b64_json/url 均缺失）".into()))
}

/// 下载生成结果。这里只重试下载本身，绝不重新提交生成请求——
/// 生成成功的图已经产生费用，重跑 adapter 会重复扣费。
async fn download_url(url: &str) -> AppResult<ImageOut> {
    if let Some(rest) = url.strip_prefix("data:") {
        let (head, data) = rest
            .split_once(',')
            .ok_or_else(|| AppError::Fetch("data url 格式错误".into()))?;
        let mime = head.split(';').next().unwrap_or("image/png").to_string();
        let bytes =
            b64_decode(data).map_err(|e| AppError::Fetch(format!("data url 解码失败: {e}")))?;
        return Ok(ImageOut { bytes, mime, meta: None });
    }

    let client = image_client();
    let mut last_err = String::new();
    for attempt in 1..=3 {
        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let mime = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("image/png")
                    .split(';')
                    .next()
                    .unwrap_or("image/png")
                    .to_string();
                if status.is_success() {
                    match resp.bytes().await {
                        Ok(bytes) if !bytes.is_empty() => {
                            return Ok(ImageOut {
                                bytes: bytes.to_vec(),
                                mime,
                                meta: None,
                            });
                        }
                        Ok(_) => last_err = "下载到的图片为空".into(),
                        Err(e) => last_err = format!("读取图片响应失败: {e}"),
                    }
                } else {
                    last_err = format!("下载图片失败 HTTP {status}");
                    if status.as_u16() != 429 && !status.is_server_error() {
                        break;
                    }
                }
            }
            Err(e) => {
                last_err = format!("下载图片请求失败: {e}");
                // 超时通常意味着线路/上游异常，连续三次各等 10 分钟没有意义
                if e.is_timeout() {
                    break;
                }
            }
        }
        if attempt < 3 {
            tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64)).await;
        }
    }
    Err(AppError::Fetch(format!(
        "{last_err}（下载已重试，未重新提交生成）"
    )))
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

/// 值在能力表里才允许下发；不在就当作没有这个参数。
fn allowed(table: &[&str], value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .filter(|v| table.contains(&v.as_str()))
        .cloned()
}

pub async fn openai_images(ctx: &GenCtx) -> AppResult<ImageOut> {
    let caps = capabilities::for_model(Protocol::OpenAiImages, &ctx.model);
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
        if let Some(size) = allowed(caps.sizes, &ctx.size) {
            form = form.text("size", size);
        }
        if let Some(q) = allowed(caps.quality, &ctx.quality) {
            form = form.text("quality", q);
        }
        // seed 不下发：Images API 规范里没有这个字段
        for r in &ctx.refs {
            let file = reqwest::multipart::Part::bytes(b64_decode(&r.data)?)
                .file_name(if r.name.is_empty() { "ref.png".into() } else { r.name.clone() })
                .mime_str(&r.mime)?;
            form = form.part("image[]", file);
        }
        if let Some(mask) = &ctx.mask {
            if caps.mask {
                let file = reqwest::multipart::Part::bytes(b64_decode(&mask.data)?)
                    .file_name("mask.png")
                    .mime_str("image/png")?;
                form = form.part("mask", file);
            }
        }
        send_json(rb.multipart(form)).await?
    } else {
        let mut body = json!({
            "model": ctx.model,
            "prompt": ctx.prompt,
            "n": 1,
        });
        if let Some(size) = allowed(caps.sizes, &ctx.size) {
            body["size"] = json!(size);
        }
        if let Some(q) = allowed(caps.quality, &ctx.quality) {
            body["quality"] = json!(q);
        }
        // seed 不下发：Images API 规范里没有这个字段
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
        .ok_or_else(|| AppError::Fetch("上游返回为空".into()))?;
    image_from_data(first).await
}

// ---------- OpenAI Chat 生图 ----------

pub async fn openai_chat_image(ctx: &GenCtx) -> AppResult<ImageOut> {
    let caps = capabilities::for_model(Protocol::OpenAiChatImage, &ctx.model);
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
    if caps.seed {
        if let Some(s) = ctx.seed {
            body["seed"] = json!(s);
        }
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
    Err(AppError::Fetch(format!(
        "回复中未解析到图片，原文: {}",
        text.chars().take(300).collect::<String>()
    )))
}

// ---------- Gemini 原生 ----------

pub async fn gemini_native(ctx: &GenCtx) -> AppResult<ImageOut> {
    let caps = capabilities::for_model(Protocol::GeminiNative, &ctx.model);
    if caps.endpoint == capabilities::ENDPOINT_PREDICT {
        return gemini_predict(ctx).await;
    }
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
    // seed 与画面比例都在 generationConfig 里，放顶层会被上游忽略
    let mut generation_config = serde_json::Map::new();
    if let Some(size) = allowed(caps.sizes, &ctx.size) {
        generation_config.insert("aspectRatio".into(), json!(size));
    }
    if caps.seed {
        if let Some(s) = ctx.seed {
            generation_config.insert("seed".into(), json!(s));
        }
    }
    if !generation_config.is_empty() {
        body["generationConfig"] = json!(generation_config);
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
        .ok_or_else(|| AppError::Fetch(format!("无 candidates: {}", resp_body.to_string().chars().take(200).collect::<String>())))?;
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
    Err(AppError::Fetch("Gemini 响应中无图片 part".into()))
}

/// Imagen 走 :predict 端点（不是 :generateContent）：请求是 instances + parameters，
/// 响应是 predictions[].bytesBase64Encoded。
async fn gemini_predict(ctx: &GenCtx) -> AppResult<ImageOut> {
    let caps = capabilities::for_model(Protocol::GeminiNative, &ctx.model);
    let client = image_client();
    let model = ctx.model.trim_start_matches("models/");
    let url = join_url(&ctx.base_url, &format!("/v1beta/models/{model}:predict"));

    let mut parameters = serde_json::Map::new();
    if let Some(size) = allowed(caps.sizes, &ctx.size) {
        parameters.insert("aspectRatio".into(), json!(size));
    }
    parameters.insert("sampleImageSize".into(), json!("1K"));
    let body = json!({
        "instances": [{"prompt": ctx.prompt}],
        "parameters": serde_json::Value::Object(parameters),
    });

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
    let predictions = resp_body["predictions"].as_array().ok_or_else(|| {
        AppError::Fetch(format!(
            "Imagen 响应无 predictions: {}",
            resp_body.to_string().chars().take(200).collect::<String>()
        ))
    })?;
    for p in predictions {
        if let Some(b64) = p["bytesBase64Encoded"].as_str() {
            let mime = p["mimeType"].as_str().unwrap_or("image/png");
            return Ok(ImageOut::new(b64_decode(b64)?, mime));
        }
    }
    Err(AppError::Fetch("Imagen 响应中无 bytesBase64Encoded".into()))
}

// ---------- Midjourney Proxy ----------

/// imagine 与 U/V 共用的轮询段。统一走 send_json，非 2xx 才不会被
/// 解析成 "error decoding response body" 这种看不懂的报错。
async fn poll_mj_task(
    client: &reqwest::Client,
    base_url: &str,
    task_id: &str,
    ctx: &GenCtx,
) -> AppResult<ImageOut> {
    let fetch_url = join_url(base_url, &format!("/mj/task/{task_id}/fetch"));
    for _ in 0..MJ_POLL_TIMES {
        tokio::time::sleep(MJ_POLL_INTERVAL).await;
        if let Some(flag) = &ctx.cancel {
            if flag.load(Ordering::Relaxed) {
                return Err(AppError::General("已取消".into()));
            }
        }
        let rb = apply_headers(client.get(&fetch_url).bearer_auth(&ctx.key), &ctx.custom_headers);
        let status: Value = send_json(rb).await?;
        match status["status"].as_str().unwrap_or("") {
            "SUCCESS" => {
                let img_url = status["imageUrl"]
                    .as_str()
                    .or_else(|| status["image_url"].as_str())
                    .ok_or_else(|| AppError::Fetch("MJ 成功但无 imageUrl".into()))?;
                return download_url(img_url).await;
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

    let mut out = poll_mj_task(&client, &ctx.base_url, task_id, ctx).await?;
    out.meta = Some(json!({
        "mj_task_id": task_id,
        "mj_status_url": format!("/mj/task/{task_id}/fetch"),
    }));
    Ok(out)
}

/// MJ U/V 放大变换。走与 imagine 相同的提交/轮询/下载链路，
/// meta 里必须带回新的 mj_task_id，画廊才能对结果图继续做 U/V。
pub async fn midjourney_action(ctx: &GenCtx, mj_task_id: &str, command: &str) -> AppResult<ImageOut> {
    let client = http_client();
    let url = join_url(&ctx.base_url, "/mj/submit/action");
    let rb = apply_headers(client.post(&url).bearer_auth(&ctx.key), &ctx.custom_headers);
    let resp: Value = send_json(rb.json(&json!({"taskId": mj_task_id, "command": command}))).await?;
    if resp["code"].as_i64() != Some(1) && resp["code"].as_str() != Some("1") {
        return Err(AppError::General(format!(
            "MJ action 提交失败: {}",
            resp.to_string().chars().take(300).collect::<String>()
        )));
    }
    let new_mj_id = resp["result"]
        .as_str()
        .ok_or_else(|| AppError::General("MJ action 未返回任务 id".into()))?;

    let mut out = poll_mj_task(&client, &ctx.base_url, new_mj_id, ctx).await?;
    out.meta = Some(json!({
        "mj_task_id": new_mj_id,
        "action": command,
    }));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{download_url, image_from_data};
    use crate::generate::types::{GenCtx, RefImage};
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    fn serve_images(responses: Vec<(u16, &'static str)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let reason = match status {
                    200 => "OK",
                    404 => "Not Found",
                    500 => "Internal Server Error",
                    _ => "Error",
                };
                let head = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body.as_bytes());
                let _ = stream.flush();
            }
        });
        format!("http://{addr}/image.png")
    }

    /// 起一个只回一条固定响应的本地服务器，同时把请求头和请求体抓下来。
    fn capture_server(response: &'static str) -> (String, Arc<Mutex<(String, String)>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = Arc::new(Mutex::new((String::new(), String::new())));
        let out = sink.clone();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf: Vec<u8> = Vec::new();
            let mut tmp = [0u8; 4096];
            let mut header_end = 0usize;
            while header_end == 0 {
                let n = stream.read(&mut tmp).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = pos + 4;
                }
            }
            let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
            let content_length = head
                .lines()
                .find_map(|l| {
                    let lower = l.to_ascii_lowercase();
                    lower
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            while buf.len() < header_end + content_length {
                let n = stream.read(&mut tmp).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            let end = (header_end + content_length).min(buf.len());
            let body = String::from_utf8_lossy(&buf[header_end..end]).to_string();
            *out.lock().unwrap() = (head, body);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        });
        (format!("http://{addr}"), sink)
    }

    /// 按顺序回 N 条响应，并把每次请求的头部与请求体都记下来。
    fn capture_server_n(
        responses: Vec<(u16, &'static str)>,
    ) -> (String, Arc<Mutex<Vec<(String, String)>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = Arc::new(Mutex::new(Vec::new()));
        let out = sink.clone();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut buf: Vec<u8> = Vec::new();
                let mut tmp = [0u8; 4096];
                let mut header_end = 0usize;
                while header_end == 0 {
                    let n = stream.read(&mut tmp).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        header_end = pos + 4;
                    }
                }
                let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
                let content_length = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while buf.len() < header_end + content_length {
                    let n = stream.read(&mut tmp).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                }
                let end = (header_end + content_length).min(buf.len());
                out.lock()
                    .unwrap()
                    .push((head, String::from_utf8_lossy(&buf[header_end..end]).to_string()));
                let reason = match status {
                    200 => "OK",
                    500 => "Internal Server Error",
                    _ => "Error",
                };
                let resp = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        });
        (format!("http://{addr}"), sink)
    }

    fn ctx(base: String, model: &str) -> GenCtx {
        GenCtx {
            base_url: base,
            key: "sk-test".into(),
            model: model.into(),
            custom_headers: Default::default(),
            prompt: "cat".into(),
            size: Some("1024x1024".into()),
            quality: Some("high".into()),
            seed: Some(42),
            refs: vec![],
            mask: None,
            cancel: None,
        }
    }

    #[tokio::test]
    async fn openai_images_never_sends_seed() {
        let (base, cap) = capture_server(r#"{"data":[{"b64_json":"aGk="}]}"#);
        let _ = super::openai_images(&ctx(base, "gpt-image-1")).await;
        let (head, body) = cap.lock().unwrap().clone();
        assert!(
            head.starts_with("POST /v1/images/generations"),
            "实际请求行：{head}"
        );
        let v: Value = serde_json::from_str(&body).expect("请求体应是 JSON");
        assert!(v.get("seed").is_none(), "不该发 seed：{body}");
        assert_eq!(v["size"], "1024x1024");
        assert_eq!(v["quality"], "high");
    }

    /// Chat Completions 的 seed 是标准参数，要发；
    /// 这条用例锁住「去 seed」不能一刀切到 chat 协议上。
    #[tokio::test]
    async fn openai_chat_image_sends_seed_but_no_size_or_quality() {
        let (base, cap) = capture_server(r#"{"choices":[{"message":{"content":"x"}}]}"#);
        let _ = super::openai_chat_image(&ctx(base, "gpt-4o-image")).await;
        let (head, body) = cap.lock().unwrap().clone();
        assert!(
            head.starts_with("POST /v1/chat/completions"),
            "实际请求行：{head}"
        );
        let v: Value = serde_json::from_str(&body).expect("请求体应是 JSON");
        assert_eq!(v["seed"], 42, "chat 协议应保留 seed：{body}");
        assert!(v.get("size").is_none(), "chat 不接受 size：{body}");
        assert!(v.get("quality").is_none(), "chat 不接受 quality：{body}");
    }

    #[tokio::test]
    async fn openai_images_edits_multipart_has_no_seed_field() {
        let (base, cap) = capture_server(r#"{"data":[{"b64_json":"aGk="}]}"#);
        let mut c = ctx(base, "gpt-image-1");
        c.refs = vec![RefImage {
            name: "r.png".into(),
            mime: "image/png".into(),
            data: "aGk=".into(),
        }];
        c.mask = Some(RefImage {
            name: "mask.png".into(),
            mime: "image/png".into(),
            data: "aGk=".into(),
        });
        let _ = super::openai_images(&c).await;
        let (head, body) = cap.lock().unwrap().clone();
        assert!(head.starts_with("POST /v1/images/edits"), "实际请求行：{head}");
        assert!(!body.contains("name=\"seed\""), "multipart 不该带 seed：{body}");
        assert!(body.contains("name=\"size\""), "multipart 应带 size");
        assert!(body.contains("name=\"mask\""), "multipart 应带 mask");
    }

    #[tokio::test]
    async fn openai_images_drops_size_not_in_capability_table() {
        let (base, cap) = capture_server(r#"{"data":[{"b64_json":"aGk="}]}"#);
        let mut c = ctx(base, "gpt-image-1");
        c.size = Some("2048x2048".into());
        let _ = super::openai_images(&c).await;
        let (_, body) = cap.lock().unwrap().clone();
        let v: Value = serde_json::from_str(&body).expect("请求体应是 JSON");
        assert!(
            v.get("size").is_none(),
            "2048x2048 对 gpt-image-1 非法：{body}"
        );
    }

    #[tokio::test]
    async fn gemini_puts_seed_inside_generation_config() {
        let (base, cap) = capture_server(
            r#"{"candidates":[{"content":{"parts":[{"inlineData":{"mimeType":"image/png","data":"aGk="}}]}}]}"#,
        );
        let mut c = ctx(base, "gemini-2.5-flash-image");
        c.size = Some("16:9".into());
        c.quality = None;
        let _ = super::gemini_native(&c).await;
        let (head, body) = cap.lock().unwrap().clone();
        assert!(
            head.starts_with("POST /v1beta/models/gemini-2.5-flash-image:generateContent"),
            "实际请求行：{head}"
        );
        let v: Value = serde_json::from_str(&body).expect("请求体应是 JSON");
        assert!(v.get("seed").is_none(), "seed 不能放顶层：{body}");
        assert_eq!(v["generationConfig"]["seed"], 42);
        assert_eq!(v["generationConfig"]["aspectRatio"], "16:9");
    }

    #[tokio::test]
    async fn imagen_uses_predict_endpoint_and_reads_predictions() {
        let (base, cap) = capture_server(
            r#"{"predictions":[{"mimeType":"image/png","bytesBase64Encoded":"aGk="}]}"#,
        );
        let mut c = ctx(base, "imagen-4.0-generate-001");
        c.size = Some("1:1".into());
        c.quality = None;
        c.seed = None;
        let out = super::gemini_native(&c).await.expect("imagen 应成功");
        let (head, body) = cap.lock().unwrap().clone();
        assert!(
            head.starts_with("POST /v1beta/models/imagen-4.0-generate-001:predict"),
            "实际请求行：{head}"
        );
        assert_eq!(out.bytes, b"hi");
        let v: Value = serde_json::from_str(&body).expect("请求体应是 JSON");
        assert_eq!(v["instances"][0]["prompt"], "cat");
        assert_eq!(v["parameters"]["aspectRatio"], "1:1");
        assert_eq!(v["parameters"]["sampleImageSize"], "1K");
    }

    #[test]
    fn missing_image_payload_is_not_retryable() {
        let result = futures::executor::block_on(image_from_data(&json!({ "data": [] })));
        match result {
            Ok(_) => panic!("expected missing payload to fail"),
            Err(e) => assert!(
                !e.is_retryable(),
                "post-generation parse failures must not resubmit: {e}"
            ),
        }
    }
    #[tokio::test]
    async fn download_404_is_not_retryable() {
        let url = serve_images(vec![(404, "missing")]);
        let err = match download_url(&url).await {
            Ok(_) => panic!("expected 404 to fail"),
            Err(e) => e,
        };
        assert!(
            !err.is_retryable(),
            "download failure after generation must not resubmit: {err}"
        );
    }

    #[tokio::test]
    async fn download_retries_5xx_without_resubmitting_generation() {
        let url = serve_images(vec![(500, "temporary"), (200, "image-bytes")]);
        let out = download_url(&url).await.expect("download should retry the 500");
        assert_eq!(out.bytes, b"image-bytes");
    }

    #[tokio::test]
    async fn mj_action_posts_and_polls_with_meta() {
        let (base, cap) = capture_server_n(vec![
            (200, r#"{"code":1,"result":"new-mj"}"#),
            (200, r#"{"status":"SUCCESS","imageUrl":"data:image/png;base64,aGk="}"#),
        ]);
        let out = super::midjourney_action(&ctx(base, "midjourney"), "old-mj", "U1")
            .await
            .unwrap();
        let reqs = cap.lock().unwrap().clone();
        assert_eq!(reqs.len(), 2, "提交 + 轮询各一次");
        assert!(reqs[0].0.starts_with("POST /mj/submit/action"), "实际：{}", reqs[0].0);
        assert!(reqs[0].1.contains(r#""taskId":"old-mj""#), "实际：{}", reqs[0].1);
        assert!(reqs[0].1.contains(r#""command":"U1""#), "实际：{}", reqs[0].1);
        assert!(reqs[1].0.starts_with("GET /mj/task/new-mj/fetch"), "实际：{}", reqs[1].0);
        assert_eq!(out.bytes, b"hi");
        let meta = out.meta.expect("放大图必须带上新的 mj_task_id 才能继续 U/V");
        assert_eq!(meta["mj_task_id"], "new-mj");
        assert_eq!(meta["action"], "U1");
    }

    #[tokio::test]
    async fn mj_action_propagates_non_2xx_status() {
        let (base, _) = capture_server_n(vec![(500, r#"{"error":{"message":"boom"}}"#)]);
        let err = super::midjourney_action(&ctx(base, "midjourney"), "old", "U1")
            .await
            .unwrap_err();
        assert!(matches!(err, crate::error::AppError::Status(500, _)), "实际：{err}");
    }

    #[tokio::test]
    async fn mj_action_reports_task_failure() {
        let (base, _) = capture_server_n(vec![
            (200, r#"{"code":1,"result":"new-mj"}"#),
            (200, r#"{"status":"FAILURE","failReason":"blocked"}"#),
        ]);
        let err = super::midjourney_action(&ctx(base, "midjourney"), "old", "U1")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("blocked"), "实际：{err}");
    }

    #[tokio::test]
    async fn imagine_still_polls_and_returns_mj_task_id() {
        let (base, cap) = capture_server_n(vec![
            (200, r#"{"code":1,"result":"t1"}"#),
            (200, r#"{"status":"SUCCESS","imageUrl":"data:image/png;base64,aGk="}"#),
        ]);
        let out = super::midjourney_proxy(&ctx(base, "midjourney")).await.unwrap();
        let reqs = cap.lock().unwrap().clone();
        assert!(reqs[0].0.starts_with("POST /mj/submit/imagine"), "实际：{}", reqs[0].0);
        assert!(reqs[1].0.starts_with("GET /mj/task/t1/fetch"), "实际：{}", reqs[1].0);
        assert_eq!(out.meta.unwrap()["mj_task_id"], "t1");
    }
}
