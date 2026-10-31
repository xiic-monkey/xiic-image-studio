use crate::capabilities;
use crate::error::{AppError, AppResult};
use crate::generate::types::{GenCtx, ImageOut};
use crate::provider::types::Protocol;
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
                    .ok_or_else(|| AppError::Fetch("MJ 成功但无 imageUrl".into()))?;
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
}
