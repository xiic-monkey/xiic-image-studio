# P0-6 协议 × 模型能力表 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让「这个协议+模型接受什么参数」有唯一真源——适配器按能力表发参、前端按能力表收敛控件，不再向严格上游发送非法参数。

**Architecture:** 新增 crate 根模块 `src-tauri/src/capabilities.rs`，纯函数 `for_model(protocol, model)` 推导 `Capabilities { sizes, quality, seed, refs, mask, endpoint, size_label }`。三个消费点：① 适配器只写能力允许的字段（gemini 新增 Imagen `:predict` 分支，修正 seed 的位置）；② `submit_task` 与 MCP 入口用 `sanitize` 裁剪请求；③ `ProviderListItem` 直接携带 `capabilities`，前端据此收敛控件并把不支持项置灰。

**Tech Stack:** Rust edition 2021 / rusqlite 0.37 / serde / reqwest / tokio；React 18 + Zustand + Tailwind；测试用 `cargo test` 与 Node 内置 `node --test`（TypeScript 原生类型擦除，零新依赖）。

**Spec:** docs/superpowers/specs/2026-10-31-p0-6-capability-table-design.md

## Global Constraints

- **不新增 Rust crate，不新增 npm 依赖**（沿用现有 `cargo test` 与 `node --test` 底座）。
- **能力表是唯一真源**：适配器不得自行判断「支不支持某参数」，一律问 `capabilities::for_model`。
- **前端不做能力推断**，只消费后端下发的 `capabilities` 字段。
- **API Key 仍留在后端**，不经 MCP 暴露；本次不改动 `custom_headers` 暴露面。
- **数据库结构不变**：能力表不落库，由 `(protocol, model)` 纯函数推导。
- **用户已确认的两项决策**：① openai_images 协议下 seed 一律不发；② imagen-* 本次实现 `:predict` 端点分支。
- 提交时只 stage 本任务涉及的文件，禁止 `git add -A` / `git add .`。
- 工作目录一律用仓库根 `/Users/xiic/RustroverProjects/xiic-image-studio`。

---

## File Structure

| 文件 | 职责 |
|---|---|
| `src-tauri/src/capabilities.rs`（新建） | 能力表唯一真源：`Capabilities` 结构、`for_model`、`sanitize` + 单测 |
| `src-tauri/src/lib.rs` | 加 `pub mod capabilities;` |
| `src-tauri/src/generate/adapters.rs` | 适配器按能力表构造请求体；新增 Imagen `:predict` 分支；加请求捕获测试服务器 |
| `src-tauri/src/provider/types.rs` | `ProviderListItem` 增 `capabilities` 字段 |
| `src-tauri/src/provider/db.rs` | `list()` 计算并填充 `capabilities` |
| `src-tauri/src/generate/commands.rs` | `submit_task` 调 `sanitize` 裁剪请求 |
| `src-tauri/src/mcp.rs` | MCP 入口同样按能力裁剪 `size`/`quality`/`seed` |
| `src/types.ts` | 前端 `Capabilities` 类型 + `ProviderListItem.capabilities` |
| `src/lib/capabilities.ts`（新建） | 前端纯函数：尺寸回落、质量回落、种子可用性 |
| `src/lib/capabilities.test.ts`（新建） | 上述纯函数的 node:test 用例 |
| `src/views/WorkbenchView.tsx` | 参数面板按能力收敛控件 |
| `package.json` | `test` 脚本加入新测试文件 |
| `docs/DESIGN.md` | 同步能力表与 seed 下发范围 |

---

### Task 1: 能力表纯函数

**Files:**
- Create: `src-tauri/src/capabilities.rs`
- Modify: `src-tauri/src/lib.rs:3-8`（在 `pub mod crypto;` 之前加 `pub mod capabilities;`）
- Test: `src-tauri/src/capabilities.rs` 内联 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::provider::types::Protocol`（枚举）、`crate::generate::types::GenerateRequest`（字段全部 pub）
- Produces:
  - `pub struct Capabilities { pub sizes: &'static [&'static str], pub quality: &'static [&'static str], pub seed: bool, pub refs: bool, pub mask: bool, pub endpoint: &'static str, pub size_label: &'static str }`（`Serialize`，`rename_all = "camelCase"`）
  - `pub fn for_model(protocol: Protocol, model: &str) -> Capabilities`
  - `pub fn sanitize(caps: &Capabilities, req: &mut GenerateRequest)`

- [x] **Step 1: 写失败测试**

创建 `src-tauri/src/capabilities.rs`，只放测试模块（实现留空给 Step 3）：

```rust
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
```

- [x] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test capabilities`

Expected: 编译失败 —— `cannot find function 'for_model' in this scope`、`cannot find type 'Protocol' in this scope` 之类——因为实现还不存在。这是预期的 RED。

- [x] **Step 3: 写实现**

在 `src-tauri/src/capabilities.rs` 中**把实现加在文件开头**（`#[cfg(test)] mod tests { … }` 保持在文件末尾，不要被实现包住）：

```rust
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
            let (sizes, quality) = if m.starts_with("dall-e-3") {
                (DALL_E3_SIZES, DALL_E3_QUALITY)
            } else if m.starts_with("gpt-image") {
                (GPT_IMAGE_SIZES, GPT_IMAGE_QUALITY)
            } else {
                (RELAY_SIZES, GPT_IMAGE_QUALITY)
            };
            Capabilities {
                sizes,
                quality,
                // Images API 规范里没有 seed 字段，官方与转 OpenAI 的中转站都会 400
                seed: false,
                refs: true,
                mask: true,
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
```

并在 `src-tauri/src/lib.rs` 的模块声明区加一行：

```rust
pub mod capabilities;
```

- [x] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test capabilities`

Expected: `test result: ok. 11 passed; 0 failed`

- [x] **Step 5: 提交**

```bash
git add src-tauri/src/capabilities.rs src-tauri/src/lib.rs
git commit -m "feat(capabilities): 协议×模型能力表（尺寸/质量/seed/参考图/蒙版/端点）"
```

---

### Task 2: openai 适配器按能力发参（不再发 seed）

**Files:**
- Modify: `src-tauri/src/generate/adapters.rs`（`openai_images` 与 `openai_chat_image` 函数体；测试模块新增捕获服务器 helper 与两个用例）

**Interfaces:**
- Consumes: `crate::capabilities::{for_model, Capabilities}`、`crate::provider::types::Protocol`、`crate::generate::types::{GenCtx, RefImage}`
- Produces: 测试模块内的 helper `fn capture_server(response: &'static str) -> (String, Arc<Mutex<(String, String)>>)`（返回 base_url 与抓到的请求头/请求体）

- [x] **Step 1: 写失败测试**

在 `src-tauri/src/generate/adapters.rs` 的 `mod tests` 里，把 `use` 改成：

```rust
#[cfg(test)]
mod tests {
    use super::{download_url, image_from_data};
    use crate::generate::types::{GenCtx, RefImage};
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
```

并追加 helper 与用例：

```rust
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
                    if lower.starts_with("content-length:") {
                        lower["content-length:".len()..].trim().parse::<usize>().ok()
                    } else {
                        None
                    }
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
        assert!(v.get("size").is_none(), "2048x2048 对 gpt-image-1 非法：{body}");
    }
```

- [x] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test openai_images`

Expected: FAIL —— `openai_images_never_sends_seed` 断言失败（实际请求体里带 `"seed":42`），`openai_images_edits_multipart_has_no_seed_field` 失败（multipart 里带 `name="seed"`），`openai_images_drops_size_not_in_capability_table` 失败（实际带上了 `2048x2048`）。

- [x] **Step 3: 写实现**

在 `src-tauri/src/generate/adapters.rs` 顶部 import 区加：

```rust
use crate::capabilities;
use crate::provider::types::Protocol;
```

把 `openai_images` 改成（其余部分不变）：

```rust
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
        if let Some(size) = allowed(&caps.sizes, &ctx.size) {
            form = form.text("size", size);
        }
        if let Some(q) = allowed(&caps.quality, &ctx.quality) {
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
        if let Some(size) = allowed(&caps.sizes, &ctx.size) {
            body["size"] = json!(size);
        }
        if let Some(q) = allowed(&caps.quality, &ctx.quality) {
            body["quality"] = json!(q);
        }
        // seed 不下发：Images API 规范里没有这个字段
        send_json(rb.json(&body)).await?
    };
```

在 `mod tests` 之前加一个小 helper（模块级，函数体内可用）：

```rust
/// 值在能力表里才允许下发；不在就当作没有这个参数。
fn allowed(table: &[&str], value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .filter(|v| table.contains(&v.as_str()))
        .cloned()
}
```

- [x] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test openai_images`

Expected: PASS（3 个新用例 + 原有 3 个 adapters 用例全绿）

- [x] **Step 5: 提交**

```bash
git add src-tauri/src/generate/adapters.rs
git commit -m "fix(adapter): openai images 按能力表发参，不再下发 seed 与非法尺寸"
```

---

### Task 3: gemini 适配器（generationConfig 修正 + Imagen `:predict` 分支）

**Files:**
- Modify: `src-tauri/src/generate/adapters.rs`（`gemini_native` 函数体 + 新增 `gemini_predict`）

**Interfaces:**
- Consumes: `crate::capabilities::{for_model, ENDPOINT_PREDICT}`、`crate::provider::types::Protocol`
- Produces: `pub async fn gemini_predict(ctx: &GenCtx) -> AppResult<ImageOut>`（模块内私有，`gemini_native` 内部按 endpoint 分发）

- [x] **Step 1: 写失败测试**

在 `src-tauri/src/generate/adapters.rs` 的 `mod tests` 里追加：

```rust
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
```

- [x] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test gemini`

Expected: FAIL —— `gemini_puts_seed_inside_generation_config` 失败（当前 seed 在顶层、没有 `generationConfig`）；`imagen_uses_predict_endpoint_and_reads_predictions` 失败（当前走 `:generateContent`，且响应里没有 `candidates` → 报错）。

- [x] **Step 3: 写实现**

把 `gemini_native` 改成：

```rust
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
    let mut generation_config = serde_json::Map::new();
    if let Some(size) = allowed(&caps.sizes, &ctx.size) {
        generation_config.insert("aspectRatio".into(), json!(size));
    }
    if let Some(s) = ctx.seed {
        if caps.seed {
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
    let candidates = resp_body["candidates"].as_array().ok_or_else(|| {
        AppError::Fetch(format!(
            "无 candidates: {}",
            resp_body.to_string().chars().take(200).collect::<String>()
        ))
    })?;
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
```

并在同文件新增 Imagen 分支：

```rust
/// Imagen 走 :predict 端点（不是 :generateContent）：请求是 instances + parameters，
/// 响应是 predictions[].bytesBase64Encoded。
async fn gemini_predict(ctx: &GenCtx) -> AppResult<ImageOut> {
    let caps = capabilities::for_model(Protocol::GeminiNative, &ctx.model);
    let client = image_client();
    let model = ctx.model.trim_start_matches("models/");
    let url = join_url(&ctx.base_url, &format!("/v1beta/models/{model}:predict"));

    let mut parameters = serde_json::Map::new();
    if let Some(size) = allowed(&caps.sizes, &ctx.size) {
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
    let predictions = resp_body["predictions"]
        .as_array()
        .ok_or_else(|| AppError::Fetch(format!(
            "Imagen 响应无 predictions: {}",
            resp_body.to_string().chars().take(200).collect::<String>()
        )))?;
    for p in predictions {
        if let Some(b64) = p["bytesBase64Encoded"].as_str() {
            let mime = p["mimeType"].as_str().unwrap_or("image/png");
            return Ok(ImageOut::new(b64_decode(b64)?, mime));
        }
    }
    Err(AppError::Fetch("Imagen 响应中无 bytesBase64Encoded".into()))
}
```

- [x] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test gemini`

Expected: PASS（2 个新用例通过）

- [x] **Step 5: 提交**

```bash
git add src-tauri/src/generate/adapters.rs
git commit -m "fix(adapter): gemini 的 seed/size 走 generationConfig，imagen 改走 :predict 端点"
```

---

### Task 4: ProviderListItem 携带能力 + 提交入口裁剪

**Files:**
- Modify: `src-tauri/src/provider/types.rs`（`ProviderListItem`）
- Modify: `src-tauri/src/provider/db.rs`（`list()`）
- Modify: `src-tauri/src/generate/commands.rs`（`submit_task`）
- Modify: `src-tauri/src/mcp.rs`（`generate()` 构造 ctx 前裁剪）
- Test: `src-tauri/src/provider/db.rs` 内联测试模块新增一个用例

**Interfaces:**
- Consumes: `crate::capabilities::{for_model, sanitize, Capabilities}`
- Produces: `ProviderListItem.capabilities: Capabilities`（序列化为 camelCase 字段 `capabilities`）

- [x] **Step 1: 写失败测试**

在 `src-tauri/src/provider/db.rs` 的 `mod tests` 里追加（复用已有的 `insert_provider`，它固定写 `protocol='openai_images'`、`model=''`）：

```rust
    #[test]
    fn list_items_carry_capabilities() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        insert_provider(&conn, "p1", "sk-abcdef");
        let items = super::list(&conn).unwrap();
        assert_eq!(items.len(), 1);
        // model 为空 → 走中转兜底行：OpenAI Images 不发 seed、支持蒙版
        assert!(!items[0].capabilities.seed);
        assert!(items[0].capabilities.mask);
        assert_eq!(items[0].capabilities.endpoint, "images");
    }
```

- [x] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test list_items_carry_capabilities`

Expected: 编译失败 —— `no field 'capabilities' on type 'ProviderListItem'`。

- [x] **Step 3: 写实现**

`src-tauri/src/provider/types.rs` 的 `ProviderListItem` 加字段：

```rust
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
    /// 该协议+模型实际接受的参数（前端据此收敛控件，适配器据此发参）
    pub capabilities: crate::capabilities::Capabilities,
}
```

`src-tauri/src/provider/db.rs` 的 `list()` 里填充（**先算能力再进结构体字面量**：字面量里 `model: p.model` 会把 `p.model` 移走，后面再 `&p.model` 就是 move 之后的借用，编译不过）：

```rust
    for item in rows {
        let (p, has_key) = item?;
        let capabilities = crate::capabilities::for_model(p.protocol, &p.model);
        out.push(ProviderListItem {
            id: p.id,
            name: p.name,
            base_url: p.base_url,
            protocol: p.protocol,
            model: p.model,
            custom_headers: p.custom_headers,
            has_key,
            created_at: p.created_at,
            updated_at: p.updated_at,
            capabilities,
        });
    }
```

`src-tauri/src/generate/commands.rs` 的 `submit_task`：把形参改成 `mut request`，并在解析出 provider 之后、建 ctx 之前插入裁剪：

```rust
pub fn submit_task(app: &AppHandle, mut request: GenerateRequest) -> AppResult<String> {
    ...
    let key = { ... };

    // 按能力表裁剪：MCP/重试/外部调用也走这里，保证不发非法参数
    let caps = crate::capabilities::for_model(provider.protocol, &provider.model);
    crate::capabilities::sanitize(&caps, &mut request);

    let ctx = generate::types::GenCtx {
        ...
        size: request.size.clone(),
        quality: request.quality.clone(),
        seed: request.seed,
        refs: request.refs.clone(),
        mask: request.mask.clone(),
        cancel: None,
    };
```

同时把落库 `request_json` 里的 size/quality/seed 换成裁剪后的 `request.size` / `request.quality` / `request.seed`（保持与实际发出的参数一致，重试才能复现）。

`src-tauri/src/mcp.rs` 的 `generate()`：在构造 `GenCtx` 之前加同样的裁剪（它不走 `submit_task`，是独立入口）：

```rust
    let caps = crate::capabilities::for_model(provider.protocol, &provider.model);
    let mut size = arguments
        .get("size")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let mut quality = arguments
        .get("quality")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let mut seed = arguments.get("seed").and_then(|v| v.as_u64());
    if !caps.sizes.contains(&size.as_deref().unwrap_or("")) {
        size = None;
    }
    if !caps.quality.contains(&quality.as_deref().unwrap_or("")) {
        quality = None;
    }
    if !caps.seed {
        seed = None;
    }
```

并把 `request_json` 与 `GenCtx` 里的 size/quality/seed 换成这三个裁剪后的变量。

- [x] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test`

Expected: 全部通过（新增 1 个用例 + 原有 30 个）

- [x] **Step 5: 提交**

```bash
git add src-tauri/src/provider/types.rs src-tauri/src/provider/db.rs src-tauri/src/generate/commands.rs src-tauri/src/mcp.rs
git commit -m "feat(db): ProviderListItem 携带能力表；GUI/MCP 提交入口按能力裁剪参数"
```

---

### Task 5: 前端类型契约 + 纯函数

**Files:**
- Modify: `src/types.ts`（新增 `Capabilities`，`ProviderListItem` 加字段）
- Create: `src/lib/capabilities.ts`
- Create: `src/lib/capabilities.test.ts`
- Modify: `package.json`（`test` 脚本）

**Interfaces:**
- Consumes: 后端下发的 `ProviderListItem.capabilities`（camelCase 字段）
- Produces:
  - `src/types.ts`: `export interface Capabilities { sizes: string[]; quality: string[]; seed: boolean; refs: boolean; mask: boolean; endpoint: string; sizeLabel: string; }`
  - `src/lib/capabilities.ts`: `normalizeSize(caps, current): string | undefined`、`normalizeQuality(caps, current): string | undefined`、`isSeedEnabled(caps): boolean`

- [x] **Step 1: 写失败测试**

创建 `src/lib/capabilities.test.ts`：

```ts
import test from "node:test";
import assert from "node:assert/strict";
import { isSeedEnabled, normalizeQuality, normalizeSize } from "./capabilities.ts";
import type { Capabilities } from "../types.ts";

const caps = (over: Partial<Capabilities> = {}): Capabilities => ({
  sizes: ["1024x1024", "1536x1024"],
  quality: ["auto", "high"],
  seed: true,
  refs: true,
  mask: true,
  endpoint: "images",
  sizeLabel: "尺寸",
  ...over,
});

test("当前尺寸合法就保持不变", () => {
  assert.equal(normalizeSize(caps(), "1536x1024"), "1536x1024");
});

test("当前尺寸不在能力表内就回落到第一个", () => {
  assert.equal(normalizeSize(caps(), "2048x2048"), "1024x1024");
});

test("不支持尺寸的协议没有可用尺寸", () => {
  assert.equal(normalizeSize(caps({ sizes: [] }), "1024x1024"), undefined);
});

test("质量合法则保持，不合法回落到第一个", () => {
  assert.equal(normalizeQuality(caps(), "high"), "high");
  assert.equal(normalizeQuality(caps(), "standard"), "auto");
});

test("不支持质量时返回 undefined", () => {
  assert.equal(normalizeQuality(caps({ quality: [] }), "auto"), undefined);
});

test("种子是否可用由能力表决定", () => {
  assert.equal(isSeedEnabled(caps()), true);
  assert.equal(isSeedEnabled(caps({ seed: false })), false);
});
```

- [x] **Step 2: 运行测试确认失败**

Run: `npm test`

Expected: FAIL —— `ERR_MODULE_NOT_FOUND` 找不到 `src/lib/capabilities.ts`（以及 `../types.ts` 的 `Capabilities` 类型）。

- [x] **Step 3: 写实现**

`src/types.ts` 加：

```ts
export interface Capabilities {
  /** 允许的尺寸取值；空数组 = 该协议/模型不接受尺寸参数 */
  sizes: string[];
  /** 允许的质量取值；空数组 = 不接受 quality */
  quality: string[];
  seed: boolean;
  refs: boolean;
  mask: boolean;
  /** 上游端点形态：images | chat | generate_content | predict | mj */
  endpoint: string;
  /** 尺寸控件标签：尺寸 | 画面比例 */
  sizeLabel: string;
}
```

`ProviderListItem` 加 `capabilities: Capabilities;`。

创建 `src/lib/capabilities.ts`：

```ts
import type { Capabilities } from "../types.ts";

/** 当前尺寸是否被能力表允许；不允许就回落到表内第一个（表空 → undefined）。 */
export function normalizeSize(caps: Capabilities, current: string): string | undefined {
  if (caps.sizes.includes(current)) return current;
  return caps.sizes[0];
}

/** 质量同理；该协议/模型不支持质量（表空）→ undefined。 */
export function normalizeQuality(caps: Capabilities, current: string): string | undefined {
  if (caps.quality.includes(current)) return current;
  return caps.quality[0];
}

/** 种子是否可用（不支持时前端置灰并说明原因，而不是静默忽略）。 */
export function isSeedEnabled(caps: Capabilities): boolean {
  return caps.seed;
}
```

`package.json` 的 `test` 脚本改为：

```json
"test": "node --test src/lib/submission.test.ts src/lib/mask-geometry.test.ts src/lib/capabilities.test.ts"
```

- [x] **Step 4: 运行测试确认通过**

Run: `npm test`

Expected: `# pass 16`、`# fail 0`（原 10 个 + 新 6 个）

- [x] **Step 5: 提交**

```bash
git add src/types.ts src/lib/capabilities.ts src/lib/capabilities.test.ts package.json
git commit -m "feat(workbench): 前端能力类型契约 + 尺寸/质量/种子纯函数"
```

---

### Task 6: 工作台参数面板按能力收敛

**Files:**
- Modify: `src/views/WorkbenchView.tsx`

**Interfaces:**
- Consumes: `provider.capabilities`（`ProviderListItem` 字段）、`src/lib/capabilities.ts` 的三个纯函数
- Produces: 无新导出（组件行为变化）

- [x] **Step 1: 写验收用例（浏览器实测断言表）**

纯逻辑（尺寸回落 / 质量回落 / 种子可用性）已在 Task 5 用 `src/lib/capabilities.test.ts` 覆盖；本任务只做接线，判定靠浏览器实测。验收断言表（每行都要实际点一遍）：

| provider（protocol / model） | 尺寸控件 | 质量控件 | 种子输入框 | 蒙版入口 |
|---|---|---|---|---|
| gpt-image-1（openai_images） | 显示，选项恰为 `1024x1024` / `1536x1024` / `1024x1536` | 显示，选项恰为 `auto` / `low` / `medium` / `high` | 置灰 + 提示「当前协议/模型不支持种子」 | 显示 |
| gemini-2.5-flash-image（gemini_native） | 显示，标签为「画面比例」，选项恰为 `1:1` / `3:4` / `4:3` / `9:16` / `16:9` | 不显示 | 可用 | 不显示 |
| gpt-4o-image（openai_chat_image） | 不显示 | 不显示 | 可用 | 不显示 |
| imagen-4.0-generate-001（gemini_native） | 显示，标签为「画面比例」 | 不显示 | 置灰 | 不显示 |

附加断言：从 gpt-image-1 切到 gemini 后，尺寸控件的值必须自动从 `1024x1024` 变成 `1:1`（旧值不在新能力表内 → 回落）。

- [x] **Step 2: 记录当前行为（RED 基线）**

在改代码之前，按 Step 1 的表在浏览器里点一遍并记录实际表现。当前实现（所有协议共用一套固定选项、种子永不置灰、蒙版按协议而非能力判断）会让上表除「gpt-image-1 的尺寸选项」外**全部不成立**——这就是本次要修的 RED。

- [x] **Step 3: 写实现**

在 `src/views/WorkbenchView.tsx` 顶部 import 区加：

```tsx
import { isSeedEnabled, normalizeQuality, normalizeSize } from "../lib/capabilities";
```

在 `WorkbenchView` 里，`const provider = providers.find(...)` 之后加：

```tsx
  const caps = provider?.capabilities;
  const sizeOptions = caps?.sizes ?? [];
  const qualityOptions = caps?.quality ?? [];
```

把参数行整段替换为：

```tsx
        {/* 参数行：按协议/模型能力收敛——不支持的能力置灰或不显示，不再静默忽略 */}
        <div className={`grid gap-2 ${qualityOptions.length > 0 ? "grid-cols-[minmax(0,1fr)_minmax(0,1fr)_3.5rem_3.5rem]" : "grid-cols-[minmax(0,1fr)_3.5rem_3.5rem]"}`}>
          {sizeOptions.length > 0 && (
            <div>
              <label className={labelCls}>{caps?.sizeLabel ?? "尺寸"}</label>
              <Select
                className={selCls}
                value={size}
                onChange={setSize}
                options={sizeOptions.map((s) => ({ value: s, label: s }))}
              />
            </div>
          )}
          {qualityOptions.length > 0 && (
            <div>
              <label className={labelCls}>质量</label>
              <Select
                className={selCls}
                value={quality}
                onChange={setQuality}
                options={qualityOptions.map((q) => ({ value: q, label: q }))}
              />
            </div>
          )}
          <div>
            <label className={labelCls} title="仅部分协议生效（openai_images / chat / gemini）">种子</label>
            <input
              type="number"
              className={selCls}
              value={seed}
              onChange={(e) => setSeed(e.target.value)}
              placeholder="随机"
              disabled={caps ? !isSeedEnabled(caps) : false}
              title={caps && !isSeedEnabled(caps) ? "当前协议/模型不支持种子" : "仅部分协议生效（openai_images / chat / gemini）"}
            />
          </div>
          <div>
            <label className={labelCls}>数量</label>
            <Select
              className={selCls}
              value={String(count)}
              onChange={(v) => setCount(Number(v))}
              options={[1, 2, 3, 4, 6, 8].map((n) => ({ value: String(n), label: String(n) }))}
            />
          </div>
        </div>
```

加一个 effect：切换供应商后，把不合法的 size/quality 拉回能力表内的值。

```tsx
  // 切换供应商/模型后，旧的尺寸/质量可能不在新能力表内 → 拉回合法值
  useEffect(() => {
    if (!caps) return;
    const nextSize = normalizeSize(caps, size);
    if (nextSize !== undefined && nextSize !== size) setSize(nextSize);
    const nextQuality = normalizeQuality(caps, quality);
    if (nextQuality !== undefined && nextQuality !== quality) setQuality(nextQuality);
  }, [caps, size, quality]);
```

把蒙版入口的判断从 `supportsMask` 改为能力判断：

```tsx
            {refs.length > 0 && caps?.mask && (
```

`showQuality` / `supportsMask` 两个旧常量都要收掉，但它们还有别处在用，**逐个替换不要直接删**：

- `supportsMask` 只有两处引用：蒙版入口（上面已改）与 `submit()` 里的 `mask: supportsMask && mask ? mask : undefined`（下面一并改成 `caps?.mask`）。改完即可删除常量。
- `showQuality` 有三处引用：`submit()`（下面一并改）、`importBatch()` 里的 `quality: showQuality ? (it.quality ?? (quality === "auto" ? undefined : quality)) : undefined` → 改成 `qualityOptions.length > 0 ? … : undefined`、以及参数行里判断是否渲染质量控件（下面已改成 `qualityOptions.length > 0`）。三处都改完再删常量。

在 `submit()` 里，构造请求前按能力裁剪（与服务端一致）：

```tsx
          const taskId = await api.generateSubmit({
            provider_id: provider.id,
            prompt: line,
            n,
            size: caps ? normalizeSize(caps, size) ?? undefined : size || undefined,
            quality: caps ? normalizeQuality(caps, quality) ?? undefined : undefined,
            seed: caps && isSeedEnabled(caps) && seed ? Number(seed) : undefined,
            refs,
            mask: caps?.mask && mask ? mask : undefined,
          });
```

- [x] **Step 4: 验证（浏览器实测 + 构建）**

Run: `npm run build` → 期望构建通过、无 TS 报错（`tsc` 会把 `noUnusedLocals` 违规也报出来，所以残留的未用常量会在这里暴露）。

浏览器实测（沿用此前验证 MaskEditor 的做法）：起 `npm run tauri dev`，在 in-app browser 里 stub `window.__TAURI_INTERNALS__.invoke`，让 `provider_list` 返回四条假 provider（openai_images/gpt-image-1、gemini_native/gemini-2.5-flash-image、gemini_native/imagen-4.0-generate-001、openai_chat_image/gpt-4o-image），然后**逐条核对 Step 1 的断言表**：

- 每个 provider 的尺寸/质量控件存在性与选项集合完全一致
- gpt-image-1 与 imagen 的种子输入框 `disabled === true`，chat 与 gemini 的 `disabled === false`
- 蒙版入口只在 gpt-image-1 下出现（加参考图后）
- 从 gpt-image-1 切到 gemini-2.5-flash-image 后，尺寸值从 `1024x1024` 自动变成 `1:1`

- [x] **Step 5: 提交**

```bash
git add src/views/WorkbenchView.tsx
git commit -m "feat(workbench): 参数面板按协议×模型能力收敛，不支持的项置灰或隐藏"
```

---

### Task 7: DESIGN 同步 + 全量验证

**Files:**
- Modify: `docs/DESIGN.md`

**Interfaces:**
- Consumes: 前面所有任务的最终形态
- Produces: 无代码接口

- [x] **Step 1: 更新文档**

`docs/DESIGN.md`：

1. §参数说明（现「seed」条目）改为：

```markdown
- **seed（随机种子）**：由「协议 × 模型能力表」（`src-tauri/src/capabilities.rs`）决定是否下发。
  openai_chat_image 走请求体顶层（Chat Completions 标准参数）；gemini_native 走 `generationConfig.seed`；
  **openai_images 不下发**（Images API 规范里没有该字段，官方与转 OpenAI 的中转站都会 400）。
  上游不支持时静默忽略，不影响其他参数。
```

2. §参数说明追加一条：

```markdown
- **能力表（协议 × 模型）**：`for_model(protocol, model)` 推导该组合实际接受的参数
  （尺寸取值 / 质量取值 / 是否支持 seed、参考图、蒙版 / 上游端点形态）。
  适配器据此发参，GUI 与 MCP 提交入口据此裁剪请求，前端据此收敛控件并把不支持项置灰。
  模型名匹配不到时走宽松兜底行（宁可上游报错，也不静默吞掉用户设的参数）。
```

3. §8 目录结构补两行：

```
│   ├── capabilities.rs                # 协议×模型能力表（唯一真源）+ 请求裁剪
```
（放在 `src-tauri/src/` 那一段，与 `crypto.rs` 并列）

```
│   ├── lib/{submission,mask-geometry,capabilities}.ts + 同名 .test.ts   # 纯函数（提交语义 / 蒙版几何 / 能力收敛），node --test
```

4. §11 关键决策追加一条：

```markdown
- **能力表而非逐处打补丁**：协议差异多且会变（中转站、模型版本），散落在适配器里迟早再次不一致。
  收敛成一张 `(protocol, model) → Capabilities` 的纯函数表，适配器、提交入口、前端读同一份，
  "协议不支持的参数"就只有一个地方能定义。
```

- [x] **Step 2: 全量验证**

Run:
```bash
cd src-tauri && cargo test
```
Expected: 全部通过，0 failed

Run:
```bash
cd src-tauri && cargo clippy --all-targets
```
Expected: 0 error（警告数不超过改动前的 10 条基线）

Run:
```bash
npm test
```
Expected: `# fail 0`

Run:
```bash
npm run build
```
Expected: 构建通过

Run:
```bash
git diff --check
```
Expected: 无输出

- [x] **Step 3: 提交**

```bash
git add docs/DESIGN.md
git commit -m "docs: DESIGN 同步能力表、seed 下发范围与请求裁剪策略"
```

---

## Self-Review

**Spec 覆盖检查**
- §3 能力表结构 → Task 1 ✓
- §3.1 表内容与匹配规则 → Task 1（11 个单测覆盖每行 + 兜底）✓
- §4.1 适配器映射 → Task 2（openai）+ Task 3（gemini/imagen）✓
- §4.2 服务端兜底 sanitize → Task 1（sanitize）+ Task 4（两处入口接线）✓
- §4.3 前端契约与 UI → Task 5（契约 + 纯函数）+ Task 6（UI）✓
- §5 错误处理 → 落在各任务实现里（裁剪静默、4xx 原样透出），无需单独任务 ✓
- §6 测试策略 → Task 1/2/3/4/5 各带测试；浏览器实测在 Task 6 Step 4 ✓
- §7 约束 → 写进 Global Constraints ✓
- §8 文档 → Task 7 ✓

**占位符扫描**：无 TBD/TODO；所有代码步骤都给出完整代码；Task 6 的浏览器实测给了明确的断言表。

**类型一致性**
- `Capabilities` 字段名在 Rust 侧是 `sizes/quality/seed/refs/mask/endpoint/size_label`，`rename_all = "camelCase"` 后前端是 `sizes/quality/seed/refs/mask/endpoint/sizeLabel` —— Task 5 的 TS 接口与之一致 ✓
- `for_model(Protocol, &str) -> Capabilities`、`sanitize(&Capabilities, &mut GenerateRequest)` 在 Task 1 定义，Task 2/3/4 调用，签名一致 ✓
- Task 2 引入的 `allowed(&[&str], &Option<String>) -> Option<String>` 在 Task 3 复用，定义在 Task 2 ✓
- Task 4 的 `ProviderListItem.capabilities` 被 Task 5 的 TS `ProviderListItem.capabilities` 与 Task 6 的 `provider.capabilities` 消费 ✓
