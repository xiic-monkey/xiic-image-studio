use crate::error::{AppError, AppResult};
use crate::generate::pipeline;
use crate::generate::types::{
    GenerateRequest, ImageMeta, ImageProducer, MAX_N, TaskItem, build_mj_action_request_json,
    build_request_json, clamp_n,
};
use crate::generate::{self, QueueState};
use crate::provider::db as provider_db;
use crate::provider::types::Protocol;
use crate::storage::Db;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn generate_submit(app: AppHandle, request: GenerateRequest) -> AppResult<String> {
    submit_task(&app, request)
}

/// 提交生成任务：GUI 与 MJ U/V 共用 prepare_task + spawn_task，
/// 落库、图片目录、进度事件、取消与并发闸门全部由编排层保证。
pub fn submit_task(app: &AppHandle, mut request: GenerateRequest) -> AppResult<String> {
    let db = app.state::<Db>();
    let prompt = request.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AppError::General("prompt 不能为空".into()));
    }
    let n = clamp_n(request.n, MAX_N);

    let provider = {
        let conn = db.conn();
        provider_db::get(&conn, &request.provider_id)?
            .ok_or_else(|| AppError::General("供应商不存在".into()))?
    };
    let key = {
        let conn = db.conn();
        provider_db::get_key(&conn, &request.provider_id)?
            .ok_or_else(|| AppError::General("该供应商尚未保存 API Key".into()))?
    };

    let caps = crate::capabilities::for_model(provider.protocol, &provider.model);
    crate::capabilities::sanitize(&caps, &mut request);

    let ctx = pipeline::build_ctx(&provider, key, &request, &prompt);
    let request_json = build_request_json(&request, n);
    let task = {
        let conn = db.conn();
        pipeline::prepare_task(
            &conn,
            request.session_id.as_deref(),
            &provider,
            n,
            &request_json,
            ImageProducer::Protocol(provider.protocol),
            ctx,
            prompt,
            provider.model.clone(),
        )?
    };
    let task_id = task.task_id.clone();
    pipeline::spawn_task(app.clone(), task);
    Ok(task_id)
}

#[tauri::command]
pub fn generate_cancel(queue: State<QueueState>, task_id: String) {
    if let Some(flag) = queue.cancels().get(&task_id) {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 从 request_json 读出重试所需字段。旧数据、MJ action 与坏 JSON 都要容忍：
/// 任务列表不能因为某条历史记录格式不同就整体挂掉。
#[derive(Default)]
struct ParsedTaskRequest {
    prompt: String,
    size: Option<String>,
    quality: Option<String>,
    n: u32,
    seed: Option<u64>,
    ref_count: u64,
}

fn parse_request_json(s: &str) -> ParsedTaskRequest {
    let req: serde_json::Value = serde_json::from_str(s).unwrap_or_default();
    let str_of = |k: &str| req.get(k).and_then(|v| v.as_str()).map(|x| x.to_string());
    ParsedTaskRequest {
        prompt: str_of("prompt").unwrap_or_default(),
        size: str_of("size"),
        quality: str_of("quality"),
        n: req.get("n").and_then(|v| v.as_u64()).unwrap_or(1) as u32,
        seed: req.get("seed").and_then(|v| v.as_u64()),
        ref_count: req.get("ref_count").and_then(|v| v.as_u64()).unwrap_or(0),
    }
}

#[tauri::command]
pub fn task_list(db: State<Db>, limit: Option<u32>) -> AppResult<Vec<TaskItem>> {
    let conn = db.conn();
    let limit = limit.unwrap_or(50).clamp(1, 500);
    let mut stmt = conn.prepare(
        "SELECT t.id, t.provider_id, t.protocol, t.status,
                COALESCE(t.total, 1),
                (SELECT COUNT(*) FROM images i WHERE i.task_id = t.id),
                t.error, t.created_at, t.request_json, t.session_id
         FROM tasks t ORDER BY t.created_at DESC LIMIT ?1",
    )?;
    let mut items: Vec<TaskItem> = stmt
        .query_map(params![limit], |r| {
            let req_json: String = r.get(8).unwrap_or_default();
            let parsed = parse_request_json(&req_json);
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
                prompt: parsed.prompt,
                size: parsed.size,
                quality: parsed.quality,
                n: parsed.n,
                seed: parsed.seed,
                ref_count: parsed.ref_count,
                images: Vec::new(),
                session_id: r.get::<_, Option<String>>(9)?,
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
/// 图片仍在画廊里，但 `images.task_id` 会被外键置空（ON DELETE SET NULL）——
/// 不再声称自己属于这个已删的任务。
#[tauri::command]
pub fn task_delete(db: State<Db>, id: String) -> AppResult<()> {
    let conn = db.conn();
    conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
    Ok(())
}

/// MJ U/V 放大变换：与常规生成共用编排层，因此同样有进度、可取消、受全局并发闸门约束。
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
    let (mj_task_id, provider_id, parent_session_id) = {
        let conn = db.conn();
        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT params_json, task_id FROM images WHERE id = ?1",
                params![image_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (params_json, parent_task_id) = row.ok_or_else(|| AppError::General("图片不存在".into()))?;
        let params: serde_json::Value = serde_json::from_str(&params_json).unwrap_or_default();
        let mj_task_id = params["mj_task_id"]
            .as_str()
            .ok_or_else(|| AppError::General("该图不是 Midjourney 生成，无法执行 U/V 操作".into()))?
            .to_string();
        let (provider_id, session_id): (String, Option<String>) = conn.query_row(
            "SELECT provider_id, session_id FROM tasks WHERE id = ?1",
            params![parent_task_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        (mj_task_id, provider_id, session_id)
    };
    let provider = {
        let conn = db.conn();
        provider_db::get(&conn, &provider_id)?
            .ok_or_else(|| AppError::General("原供应商已被删除".into()))?
    };
    let key = {
        let conn = db.conn();
        provider_db::get_key(&conn, &provider_id)?
            .ok_or_else(|| AppError::General("供应商缺少 API Key".into()))?
    };

    let prompt = format!("[{command}]");
    let req = GenerateRequest {
        provider_id: provider_id.clone(),
        prompt: prompt.clone(),
        n: 1,
        size: None,
        quality: None,
        seed: None,
        refs: vec![],
        mask: None,
        session_id: parent_session_id.clone(),
    };
    let ctx = pipeline::build_ctx(&provider, key, &req, &prompt);
    let request_json = build_mj_action_request_json(&command, &mj_task_id);
    let producer = ImageProducer::MjAction { mj_task_id, command: command.clone() };
    let task = {
        let conn = db.conn();
        pipeline::prepare_task(
            &conn,
            parent_session_id.as_deref(),
            &provider,
            1,
            &request_json,
            producer,
            ctx,
            prompt,
            provider.model.clone(),
        )?
    };
    let task_id = task.task_id.clone();
    pipeline::spawn_task(app.clone(), task);
    Ok(task_id)
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

#[cfg(test)]
mod tests {
    use super::parse_request_json;

    #[test]
    fn parse_request_json_reads_seed() {
        let p = parse_request_json(
            r#"{"prompt":"cat","n":2,"size":"1024x1024","quality":"high","seed":42,"ref_count":0}"#,
        );
        assert_eq!(p.prompt, "cat");
        assert_eq!(p.n, 2);
        assert_eq!(p.size.as_deref(), Some("1024x1024"));
        assert_eq!(p.quality.as_deref(), Some("high"));
        assert_eq!(p.seed, Some(42));
        assert_eq!(p.ref_count, 0);
    }

    #[test]
    fn parse_request_json_tolerates_legacy_request_without_seed() {
        let p = parse_request_json(r#"{"prompt":"old","n":1,"ref_count":1}"#);
        assert_eq!(p.prompt, "old");
        assert_eq!(p.seed, None);
        assert_eq!(p.ref_count, 1, "有参考图的旧任务仍要隐藏重试入口");
    }

    #[test]
    fn parse_request_json_tolerates_mj_action_shape() {
        let p = parse_request_json(r#"{"action":"U1","mj_task_id":"t1"}"#);
        assert_eq!(p.prompt, "", "MJ action 无 prompt，前端不显示重试");
        assert_eq!(p.n, 1);
    }

    #[test]
    fn parse_request_json_tolerates_malformed_json() {
        let p = parse_request_json("not json at all");
        assert_eq!(p.prompt, "");
        assert_eq!(p.n, 1);
        assert_eq!(p.seed, None);
    }
}
