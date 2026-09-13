use crate::error::{AppError, AppResult};
use crate::generate::{self, QueueState};
use crate::generate::types::{GenerateRequest, ImageMeta, TaskItem};
use crate::provider::db as provider_db;
use crate::provider::types::Protocol;
use crate::storage::Db;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn generate_submit(
    app: AppHandle,
    db: State<'_, Db>,
    queue: State<'_, QueueState>,
    request: GenerateRequest,
) -> AppResult<String> {
    let _ = (&db, &queue);
    submit_task(&app, request)
}

/// 提交生成任务的核心实现。
///
/// UI 命令与本地能力端口（bridge，供 MCP 调用）**共用这一条链路**：
/// 外部调用产出的任务，落库、图片目录、进度事件与手动点「生成」完全一致，
/// 所以工作台天然能看到、能实时刷新，不需要外部进程自己去写库。
pub fn submit_task(app: &AppHandle, request: GenerateRequest) -> AppResult<String> {
    let db = app.state::<Db>();
    let prompt = request.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AppError::General("prompt 不能为空".into()));
    }
    let n = request.n.clamp(1, 8);

    // 解析供应商
    let provider = {
        let conn = db.0.lock().unwrap();
        provider_db::get(&conn, &request.provider_id)?
            .ok_or_else(|| AppError::General("供应商不存在".into()))?
    };
    let key = {
        let conn = db.0.lock().unwrap();
        provider_db::get_key(&conn, &request.provider_id)?
            .ok_or_else(|| AppError::General("该供应商尚未保存 API Key".into()))?
    };

    let ctx = generate::types::GenCtx {
        base_url: provider.base_url.clone(),
        key,
        model: provider.model.clone(),
        custom_headers: provider.custom_headers.clone(),
        prompt: prompt.clone(),
        size: request.size.clone(),
        quality: request.quality.clone(),
        seed: request.seed,
        refs: request.refs.clone(),
        mask: request.mask.clone(),
        cancel: None,
    };

    let task_id = uuid::Uuid::new_v4().to_string();
    {
        let conn = db.0.lock().unwrap();
        conn.execute(
            "INSERT INTO tasks (id, session_id, provider_id, protocol, request_json, status, concurrency, total, created_at)
             VALUES (?1, NULL, ?2, ?3, ?4, 'running', ?5, ?5, ?6)",
            params![
                task_id,
                provider.id,
                provider.protocol.as_str(),
                serde_json::json!({
                    "prompt": prompt,
                    "n": n,
                    "size": request.size,
                    "quality": request.quality,
                    "seed": request.seed,
                    "ref_count": request.refs.len(),
                })
                .to_string(),
                n,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
    }

    generate::spawn_task(
        app.clone(),
        task_id.clone(),
        provider.protocol,
        ctx,
        n,
        prompt,
        provider.model.clone(),
    );
    Ok(task_id)
}

#[tauri::command]
pub fn generate_cancel(queue: State<QueueState>, task_id: String) {
    if let Some(flag) = queue.cancels.lock().unwrap().get(&task_id) {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn task_list(db: State<Db>, limit: Option<u32>) -> AppResult<Vec<TaskItem>> {
    let conn = db.0.lock().unwrap();
    let limit = limit.unwrap_or(50).clamp(1, 500);
    let mut stmt = conn.prepare(
        "SELECT t.id, t.provider_id, t.protocol, t.status,
                COALESCE(t.total, 1),
                (SELECT COUNT(*) FROM images i WHERE i.task_id = t.id),
                t.error, t.created_at, t.request_json
         FROM tasks t ORDER BY t.created_at DESC LIMIT ?1",
    )?;
    let mut items: Vec<TaskItem> = stmt
        .query_map(params![limit], |r| {
            let req_json: String = r.get(8).unwrap_or_default();
            let req: serde_json::Value = serde_json::from_str(&req_json).unwrap_or_default();
            let str_of = |k: &str| req.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
            let prompt = str_of("prompt").unwrap_or_default();
            Ok(TaskItem {
                id: r.get(0)?,
                provider_id: r.get(1)?,
                protocol: Protocol::parse(&r.get::<_, String>(2)?)
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_default(),
                status: r.get(3)?,
                total: r.get(4)?,
                done: r.get(5)?,
                error: r.get(6)?,
                created_at: r.get(7)?,
                prompt,
                size: str_of("size"),
                quality: str_of("quality"),
                n: req.get("n").and_then(|v| v.as_u64()).unwrap_or(1) as u32,
                ref_count: req.get("ref_count").and_then(|v| v.as_u64()).unwrap_or(0),
                images: Vec::new(),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // 一次性取出这批任务的所有图片并按 task_id 分组回填（避免 N+1 次查询）。
    // 没有这一步，刷新窗口后历史任务永远没有缩略图——实时进度只推"新生成"的图。
    if !items.is_empty() {
        let ids: Vec<String> = items.iter().map(|t| t.id.clone()).collect();
        let marks = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT task_id, id, path, thumb_path FROM images WHERE task_id IN ({}) ORDER BY created_at",
            marks
        );
        let mut img_stmt = conn.prepare(&sql)?;
        let mut grouped: HashMap<String, Vec<ImageMeta>> = HashMap::new();
        let rows = img_stmt.query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok((
                r.get::<_, String>(0)?,
                ImageMeta {
                    id: r.get(1)?,
                    path: r.get(2)?,
                    thumb_path: r.get(3)?,
                },
            ))
        })?;
        for (task_id, meta) in rows.filter_map(|r| r.ok()) {
            grouped.entry(task_id).or_default().push(meta);
        }
        for t in &mut items {
            if let Some(v) = grouped.remove(&t.id) {
                t.images = v;
            }
        }
    }
    Ok(items)
}

/// 从任务队列删除一条任务记录（仅删 tasks 表，不影响已生成的图片/gallery）。
/// 主要用于清理已取消/失败/完成的任务。
#[tauri::command]
pub fn task_delete(db: State<Db>, id: String) -> AppResult<()> {
    let conn = db.0.lock().unwrap();
    conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
    Ok(())
}

/// MJ U/V 放大变换：基于已有图片记录里的 mj_task_id 提交 action
#[tauri::command]
pub async fn mj_action(
    app: AppHandle,
    db: State<'_, Db>,
    image_id: String,
    command: String,
) -> AppResult<String> {
    const ALLOWED: [&str; 8] = ["U1", "U2", "U3", "U4", "V1", "V2", "V3", "V4"];
    if !ALLOWED.contains(&command.as_str()) {
        return Err(AppError::General("command 仅支持 U1-U4 / V1-V4".into()));
    }
    let (mj_task_id, provider_id) = {
        let conn = db.0.lock().unwrap();
        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT params_json, task_id FROM images WHERE id = ?1",
                params![image_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (params_json, task_id) = row.ok_or_else(|| AppError::General("图片不存在".into()))?;
        let params: serde_json::Value = serde_json::from_str(&params_json).unwrap_or_default();
        let mj_task_id = params["mj_task_id"]
            .as_str()
            .ok_or_else(|| AppError::General("该图不是 Midjourney 生成，无法执行 U/V 操作".into()))?
            .to_string();
        let provider_id: String = conn
            .query_row("SELECT provider_id FROM tasks WHERE id = ?1", params![task_id], |r| r.get(0))?;
        (mj_task_id, provider_id)
    };
    let provider = {
        let conn = db.0.lock().unwrap();
        provider_db::get(&conn, &provider_id)?
            .ok_or_else(|| AppError::General("原供应商已被删除".into()))?
    };
    let key = {
        let conn = db.0.lock().unwrap();
        provider_db::get_key(&conn, &provider_id)?
            .ok_or_else(|| AppError::General("供应商缺少 API Key".into()))?
    };

    let task_id = uuid::Uuid::new_v4().to_string();
    {
        let conn = db.0.lock().unwrap();
        conn.execute(
            "INSERT INTO tasks (id, session_id, provider_id, protocol, request_json, status, concurrency, total, created_at)
             VALUES (?1, NULL, ?2, 'midjourney_proxy', ?3, 'running', 1, 1, ?4)",
            params![
                task_id,
                provider.id,
                serde_json::json!({"action": command, "mj_task_id": mj_task_id}).to_string(),
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
    }

    let worker_task_id = task_id.clone();
    tauri::async_runtime::spawn(async move {
        mj_action_worker(app, worker_task_id, provider.base_url, key, provider.custom_headers, mj_task_id, command, provider.model).await;
    });
    Ok(task_id)
}

#[allow(clippy::too_many_arguments)]
async fn mj_action_worker(
    app: AppHandle,
    task_id: String,
    base_url: String,
    key: String,
    headers: std::collections::BTreeMap<String, String>,
    mj_task_id: String,
    command: String,
    model: String,
) {
    let db = app.state::<Db>();
    let fail = |app: &AppHandle, db: &Db, task_id: &str, e: String| {
        generate::finish_task(app, db, task_id, "failed", Some(e));
    };

    let run = async {
        let client = generate::client::http_client();
        let url = generate::client::join_url(&base_url, "/mj/submit/action");
        let rb = generate::client::apply_headers(client.post(&url).bearer_auth(&key), &headers);
        let resp: serde_json::Value = rb
            .json(&serde_json::json!({"taskId": mj_task_id, "command": command}))
            .send()
            .await?
            .json()
            .await?;
        if resp["code"].as_i64() != Some(1) && resp["code"].as_str() != Some("1") {
            return Err(AppError::General(format!(
                "MJ action 提交失败: {}",
                resp.to_string().chars().take(300).collect::<String>()
            )));
        }
        let new_mj_id = resp["result"]
            .as_str()
            .ok_or_else(|| AppError::General("MJ action 未返回任务 id".into()))?;

        let fetch_url =
            generate::client::join_url(&base_url, &format!("/mj/task/{new_mj_id}/fetch"));
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            let rb = generate::client::apply_headers(
                client.get(&fetch_url).bearer_auth(&key),
                &headers,
            );
            let status: serde_json::Value = rb.send().await?.json().await?;
            match status["status"].as_str().unwrap_or("") {
                "SUCCESS" => {
                    let img_url = status["imageUrl"]
                        .as_str()
                        .or_else(|| status["image_url"].as_str())
                        .ok_or_else(|| AppError::General("MJ 成功但无 imageUrl".into()))?;
                    return Ok((img_url.to_string(), new_mj_id.to_string()));
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
    };

    let (img_url, new_mj_id) = match run.await {
        Ok(v) => v,
        Err(e) => return fail(&app, &db, &task_id, e.to_string()),
    };

    let download = async {
        let resp = generate::client::http_client().get(&img_url).send().await?;
        let mime = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/png")
            .split(';')
            .next()
            .unwrap_or("image/png")
            .to_string();
        let bytes = resp.bytes().await?.to_vec();
        Ok::<_, AppError>((bytes, mime))
    };
    let (bytes, mime) = match download.await {
        Ok(v) => v,
        Err(e) => return fail(&app, &db, &task_id, e.to_string()),
    };
    let out = generate::types::ImageOut {
        bytes,
        mime,
        meta: Some(serde_json::json!({"mj_task_id": new_mj_id})),
    };

    let saved = generate::images_dir(&app)
        .and_then(|dir| generate::save_image(&dir, &task_id, 0, &out));
    match saved {
        Ok((path, thumb_path)) => {
            let img_id = uuid::Uuid::new_v4().to_string();
            {
                let conn = db.0.lock().unwrap();
                let _ = conn.execute(
                    "INSERT INTO images (id, task_id, path, thumb_path, prompt, model, params_json, favorite, width, height, size, created_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,0,?8,?9,?10,?11)",
                    params![
                        img_id,
                        task_id,
                        path,
                        thumb_path,
                        format!("[{command}]"),
                        model,
                        serde_json::json!({"mj_task_id": new_mj_id, "action": command}).to_string(),
                        0, 0, out.bytes.len() as i64,
                        chrono::Utc::now().to_rfc3339()
                    ],
                );
            }
            generate::emit_progress(&app, &db, &task_id, "completed", None,
                Some(generate::types::ImageMeta { id: img_id, path, thumb_path }));
            generate::finish_task(&app, &db, &task_id, "completed", None);
        }
        Err(e) => fail(&app, &db, &task_id, e.to_string()),
    }
}

/// 读取生成图片为 data url（仅允许应用 images 目录）
#[tauri::command]
pub fn image_read_b64(app: AppHandle, path: String) -> AppResult<String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::General(e.to_string()))?
        .join("images");
    generate::read_file_b64(&root, &path)
}
