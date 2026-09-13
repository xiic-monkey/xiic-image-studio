//! 无头 MCP 模式：`xiic-image-studio --mcp`
//!
//! # 通信方式：纯 stdio，不占任何端口
//! 没有 HTTP、没有 SSE、没有 unix socket、没有常驻 daemon。
//! agent 的 MCP 客户端把这个可执行文件当子进程拉起来，JSON-RPC 从 stdin 进、
//! 从 stdout 出（这就是 MCP 的标准 stdio 传输）。进程随 agent 会话结束而退出。
//!
//! # 能力归属：仍然是 studio 自己
//! 这里不复制任何"底层能力"——不重新实现适配器、不自己解析 provider、
//! 不走另一套落库逻辑。它只是把 app 自己的生成链路（同一个 SQLite、
//! 同一套 adapters、同一套任务/图片落库规则）在**没有 GUI** 的情况下跑一遍。
//! 所以外部调用产出的图，和工作室里手动点「生成」的产物完全一致：
//! 同一张 tasks/images 表、同一个 images 目录，工作台打开就看得见。

use std::io::{self, BufReader};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use rusqlite::Connection;
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use crate::error::{AppError, AppResult};
use crate::generate::types::GenCtx;
use crate::generate::{self, finish_task_row, read_file_b64, run_one_core};
use crate::mcp_protocol::{MessageFraming, read_message, write_message};
use crate::provider::db as provider_db;
use crate::storage::{self, Db};

const SERVER_NAME: &str = "xiic-image-studio";
const PROTOCOL_VERSION: &str = "2024-11-05";

/// 无 Tauri 运行时时的应用数据目录（与 `app.path().app_data_dir()` 一致）。
fn app_data_dir() -> AppResult<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| AppError::General("无法定位应用数据目录".into()))?;
    Ok(base.join("com.xiic.image-studio"))
}

/// 无头入口：stdin/stdout 跑 MCP 协议，直到 stdin 关闭。
pub fn run_stdio() -> Result<()> {
    let dir = app_data_dir()?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("无法创建应用数据目录：{}", dir.display()))?;

    let conn = Connection::open(storage::db_path(&dir))
        .with_context(|| format!("无法打开数据库：{}", dir.display()))?;
    // GUI 也在写同一张表（WAL 只解决读写冲突，不解决写写冲突）
    conn.busy_timeout(Duration::from_secs(10))?;
    storage::init(&conn)?;
    let db = Db(Mutex::new(conn));

    let images_dir = dir.join("images");
    std::fs::create_dir_all(&images_dir)?;

    let runtime = Runtime::new().context("无法创建 tokio runtime")?;

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut out = stdout.lock();

    loop {
        let incoming = match read_message(&mut reader)? {
            Some(message) => message,
            // stdin 关闭 = agent 断开，正常退出
            None => break,
        };
        if let Some(response) = handle(&db, &images_dir, &runtime, incoming.payload) {
            write_message(&mut out, &response, incoming.framing)?;
        }
    }
    Ok(())
}

fn handle(db: &Db, images_dir: &PathBuf, runtime: &Runtime, message: Value) -> Option<Value> {
    let method = message.get("method").and_then(|m| m.as_str())?;
    let id = message.get("id").cloned();
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));

    let response = match (id, method) {
        // 通知类（notifications/initialized 等）不回复
        (None, _) => return None,
        (Some(id), "initialize") => success(id, initialize()),
        (Some(id), "ping") => success(id, json!({})),
        (Some(id), "resources/list") => success(id, json!({ "resources": [] })),
        (Some(id), "tools/list") => success(id, tools_list()),
        (Some(id), "tools/call") => match call_tool(db, images_dir, runtime, params, id.clone()) {
            Ok(result) => result,
            Err(err) => success(id, tool_error(format!("{err:#}"))),
        },
        (Some(id), other) => error(id, -32601, format!("method '{other}' not found")),
    };
    Some(response)
}

fn initialize() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
        "instructions": "用 generate_image 让 xiic-image-studio 出图（结果会写进工作室的任务与图库，GUI 打开即可看到）。不确定用哪个 provider 就先调 list_providers。"
    })
}

fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "generate_image",
                "description": "用本地 xiic-image-studio 生成图片。出图会写进工作室的任务列表与图库（GUI 打开即可看到），并返回本地路径与图片内容。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "prompt": { "type": "string", "description": "画面描述（想画什么）。" },
                        "provider": {
                            "type": "string",
                            "description": "可选：provider 名称或 id；不填用默认（第一个已配好 API Key 的）。用 list_providers 查看。"
                        },
                        "n": { "type": "integer", "minimum": 1, "maximum": 4, "description": "生成张数（默认 1，最多 4）。" },
                        "size": { "type": "string", "description": "可选分辨率，如 1024x1024。" },
                        "quality": { "type": "string", "description": "可选质量，如 low/medium/high 或 standard/hd。" },
                        "seed": { "type": "integer", "description": "可选随机种子。" }
                    },
                    "required": ["prompt"],
                    "additionalProperties": false
                }
            },
            {
                "name": "list_providers",
                "description": "列出工作室里已配置的 provider（名称 / 协议 / 模型 / 是否已配 Key）。",
                "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
            }
        ]
    })
}

fn call_tool(
    db: &Db,
    images_dir: &PathBuf,
    runtime: &Runtime,
    params: Value,
    id: Value,
) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

    match name.as_str() {
        "list_providers" => {
            let items = {
                let conn = db.0.lock().unwrap();
                provider_db::list(&conn)?
            };
            let text = if items.is_empty() {
                "还没有配置任何 provider（请先在 image-studio 应用里添加一个）".to_string()
            } else {
                items
                    .iter()
                    .map(|p| {
                        format!(
                            "- {} [{}] model={} key={}",
                            p.name,
                            p.protocol.as_str(),
                            if p.model.is_empty() { "默认" } else { &p.model },
                            if p.has_key { "已配置" } else { "未配置" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok(success(
                id,
                json!({
                    "content": [{ "type": "text", "text": format!("可用 provider：\n{text}") }],
                    "structuredContent": { "providers": items }
                }),
            ))
        }
        "generate_image" => match generate(db, images_dir, runtime, arguments) {
            Ok(result) => Ok(success(id, tool_ok(&result))),
            Err(err) => Ok(success(id, tool_error(format!("{err:#}")))),
        },
        other => Ok(error(id, -32601, format!("未知工具 '{other}'"))),
    }
}

/// 生成：建任务 → 跑适配器 → 落库 → 收尾，全用 app 自己的链路。
fn generate(
    db: &Db,
    images_dir: &PathBuf,
    runtime: &Runtime,
    arguments: Value,
) -> Result<Value> {
    let prompt = arguments
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if prompt.is_empty() {
        bail!("prompt 不能为空");
    }
    let n = arguments
        .get("n")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .clamp(1, 4) as u32;

    let provider_hint = arguments.get("provider").and_then(|v| v.as_str());

    // 选 provider：名称或 id 都接受，不给就用第一个配好 key 的
    let (provider, key) = {
        let conn = db.0.lock().unwrap();
        let items = provider_db::list(&conn)?;
        if items.is_empty() {
            bail!("还没有配置任何 provider，请先在 image-studio 应用里添加一个");
        }
        let chosen = match provider_hint.map(str::trim).filter(|h| !h.is_empty()) {
            Some(hint) => items
                .iter()
                .find(|p| p.id == hint || p.name.eq_ignore_ascii_case(hint))
                .ok_or_else(|| {
                    anyhow!(
                        "找不到 provider '{hint}'（可用：{}）",
                        items
                            .iter()
                            .map(|p| p.name.as_str())
                            .collect::<Vec<_>>()
                            .join("、")
                    )
                })?,
            None => items.iter().find(|p| p.has_key).ok_or_else(|| {
                anyhow!("所有 provider 都还没填 API Key，请先在 image-studio 应用里配置")
            })?,
        };
        if !chosen.has_key {
            bail!("provider '{}' 还没填 API Key", chosen.name);
        }
        let (provider, key) = provider_db::get_with_key(&conn, &chosen.id)?
            .ok_or_else(|| anyhow!("provider '{}' 不存在或没有可用的 API Key", chosen.name))?;
        (provider, key)
    };

    let request_json = json!({
        "prompt": prompt,
        "n": n,
        "size": arguments.get("size"),
        "quality": arguments.get("quality"),
        "seed": arguments.get("seed"),
        "ref_count": 0,
        "source": "mcp",
    })
    .to_string();

    let task_id = {
        let conn = db.0.lock().unwrap();
        generate::create_task(&conn, &provider.id, provider.protocol.as_str(), &request_json, n)?
    };

    let ctx = GenCtx {
        base_url: provider.base_url.clone(),
        key,
        model: provider.model.clone(),
        custom_headers: provider.custom_headers.clone(),
        prompt: prompt.clone(),
        size: arguments
            .get("size")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        quality: arguments
            .get("quality")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        seed: arguments.get("seed").and_then(|v| v.as_u64()),
        refs: Vec::new(),
        mask: None,
        cancel: None,
    };

    let mut images: Vec<Value> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for index in 0..n {
        let cancel = Arc::new(AtomicBool::new(false));
        let ctx = GenCtx {
            base_url: ctx.base_url.clone(),
            key: ctx.key.clone(),
            model: ctx.model.clone(),
            custom_headers: ctx.custom_headers.clone(),
            prompt: ctx.prompt.clone(),
            size: ctx.size.clone(),
            quality: ctx.quality.clone(),
            seed: ctx.seed,
            refs: Vec::new(),
            mask: None,
            cancel: Some(cancel.clone()),
        };

        match runtime.block_on(run_one_core(
            db,
            images_dir,
            &task_id,
            provider.protocol,
            &ctx,
            index,
            &prompt,
            &provider.model,
            cancel,
            // 无头模式不参与 GUI 的全局并发闸门（那道闸门是给 UI 批量提交兜底的）
            None,
        )) {
            Ok(Some(meta)) => {
                let data_url = read_file_b64(images_dir, &meta.path).unwrap_or_default();
                images.push(json!({
                    "id": meta.id,
                    "path": meta.path,
                    "thumb_path": meta.thumb_path,
                    "data_url": data_url,
                }));
            }
            Ok(None) => {}
            Err(err) => errors.push(err.to_string()),
        }
    }

    let status = if images.is_empty() {
        "failed"
    } else if errors.is_empty() {
        "completed"
    } else {
        "completed_with_errors"
    };
    {
        let conn = db.0.lock().unwrap();
        finish_task_row(&conn, &task_id, status, errors.first().cloned());
    }

    if images.is_empty() {
        bail!("{}", errors.first().cloned().unwrap_or_else(|| "生成失败".into()));
    }

    let mut result = json!({
        "task_id": task_id,
        "status": status,
        "count": images.len(),
        "images": images,
    });
    if let Some(err) = errors.first() {
        result["error"] = json!(err);
    }
    Ok(result)
}

/// 把生成结果翻译成 MCP 内容：文本摘要 + 图片本体（agent 可直接看图）。
fn tool_ok(result: &Value) -> Value {
    let images = result
        .get("images")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let task_id = result.get("task_id").and_then(|v| v.as_str()).unwrap_or("");

    let mut text = format!(
        "已生成 {} 张图片，已写入 image-studio 的任务与图库（打开工作室即可看到，task_id: {}）",
        images.len(),
        task_id
    );
    for img in &images {
        text.push_str(&format!("\n- file://{}", img["path"].as_str().unwrap_or("")));
    }
    if let Some(err) = result.get("error").and_then(|v| v.as_str()) {
        text.push_str(&format!("\n注意：{err}"));
    }

    let mut content: Vec<Value> = vec![json!({ "type": "text", "text": text })];
    for img in &images {
        if let Some(data_url) = img["data_url"].as_str() {
            if let Some(rest) = data_url.strip_prefix("data:") {
                if let Some((head, data)) = rest.split_once(',') {
                    let mime = head.split(';').next().unwrap_or("image/png").to_string();
                    content.push(json!({ "type": "image", "data": data, "mimeType": mime }));
                }
            }
        }
    }

    json!({
        "content": content,
        "structuredContent": {
            "task_id": task_id,
            "status": result.get("status").cloned().unwrap_or_else(|| json!("completed")),
            "count": images.len(),
            "images": images.iter().map(|img| json!({
                "path": img["path"], "thumb_path": img["thumb_path"]
            })).collect::<Vec<_>>(),
        }
    })
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "structuredContent": { "error": message },
        "isError": true
    })
}

#[allow(dead_code)]
fn _assert_framing(_f: MessageFraming) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_reports_server_info() {
        let info = initialize();
        assert_eq!(info["serverInfo"]["name"], "xiic-image-studio");
        assert_eq!(info["protocolVersion"], "2024-11-05");
    }

    #[test]
    fn tools_list_exposes_studio_capabilities() {
        let tools = tools_list();
        let arr = tools["tools"].as_array().unwrap();
        let names: Vec<&str> = arr.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["generate_image", "list_providers"]);
        assert_eq!(arr[0]["inputSchema"]["required"], json!(["prompt"]));
    }

    #[test]
    fn unknown_tool_is_method_not_found() {
        let db = {
            let dir = std::env::temp_dir().join(format!("xiic-mcp-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let conn = Connection::open(dir.join("image-studio.db")).unwrap();
            storage::init(&conn).unwrap();
            (dir, Db(Mutex::new(conn)))
        };
        let runtime = Runtime::new().unwrap();
        let response = call_tool(
            &db.1,
            &db.0.join("images"),
            &runtime,
            json!({ "name": "nope", "arguments": {} }),
            json!(1),
        )
        .unwrap();
        assert_eq!(response["result"]["error"]["code"], -32601);
    }
}
