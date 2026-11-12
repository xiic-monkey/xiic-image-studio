pub mod adapters;
pub mod client;
pub mod commands;
pub mod pipeline;
pub mod types;

use crate::error::{AppError, AppResult};
use crate::generate::types::*;
use crate::provider::types::Protocol;
use crate::storage::Db;
use base64::Engine;
use rusqlite::params;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

/// 全局同时在飞的上游请求上限。
///
/// 单个任务内部已按 `n.clamp(1,4)` 限流，但那是"每个任务"的上限——
/// 批量提交 5 行就是 5 个任务，各自还能开 4 个，合起来 20 个请求同时打上游，
/// 极易撞 429 限流。这里加一道总闸门兜底，不管提交多少任务，
/// 同时在飞的上游请求都不超过这个数。
const MAX_GLOBAL_INFLIGHT: usize = 4;

pub struct QueueState {
    pub cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// 全局并发闸门（见 MAX_GLOBAL_INFLIGHT）
    pub inflight: Arc<tokio::sync::Semaphore>,
}

impl QueueState {
    pub fn new() -> Self {
        QueueState {
            cancels: Mutex::new(HashMap::new()),
            inflight: Arc::new(tokio::sync::Semaphore::new(MAX_GLOBAL_INFLIGHT)),
        }
    }

    /// 取取消标志表。锁被毒化（之前有线程持锁时 panic）不直接 panic：
    /// 取消表本身没坏，毒化只说明那次崩溃发生在持锁期间。
    pub fn cancels(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<AtomicBool>>> {
        self.cancels.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub(crate) fn images_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::General(e.to_string()))?
        .join("images"))
}

fn dispatch_producer<'a>(
    ctx: &'a GenCtx,
    producer: &'a ImageProducer,
) -> Pin<Box<dyn Future<Output = AppResult<ImageOut>> + Send + 'a>> {
    match producer {
        ImageProducer::Protocol(Protocol::OpenAiImages) => Box::pin(adapters::openai_images(ctx)),
        ImageProducer::Protocol(Protocol::OpenAiChatImage) => Box::pin(adapters::openai_chat_image(ctx)),
        ImageProducer::Protocol(Protocol::GeminiNative) => Box::pin(adapters::gemini_native(ctx)),
        ImageProducer::Protocol(Protocol::MidjourneyProxy) => Box::pin(adapters::midjourney_proxy(ctx)),
        ImageProducer::MjAction { mj_task_id, command } => {
            Box::pin(adapters::midjourney_action(ctx, mj_task_id, command))
        }
    }
}

pub(crate) fn save_image(
    dir: &Path,
    task_id: &str,
    index: u32,
    out: &ImageOut,
) -> AppResult<(String, String)> {
    let task_dir = dir.join(task_id);
    std::fs::create_dir_all(&task_dir)?;
    let ext = match out.mime.as_str() {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    };
    let id = uuid::Uuid::new_v4().to_string();
    let file_path = task_dir.join(format!("{index:02}-{id}.{ext}"));
    std::fs::write(&file_path, &out.bytes)?;

    let thumb_path = task_dir.join(format!("{index:02}-{id}.thumb.jpg"));
    if let Ok(img) = image::load_from_memory(&out.bytes) {
        let thumb = img.thumbnail(384, 384);
        let _ = thumb.save_with_format(&thumb_path, image::ImageFormat::Jpeg);
    } else {
        let _ = std::fs::write(&thumb_path, &out.bytes);
    }
    Ok((
        file_path.to_string_lossy().to_string(),
        thumb_path.to_string_lossy().to_string(),
    ))
}

/// 一条待写入 `images` 表的记录（字段与表列一一对应）。
///
/// 三处写库点（UI 生成、MJ 放大、无头 MCP）以前各写一份 `conn.execute`
/// 且都 `let _ =` 吞掉错误——图已经在磁盘上，画廊却看不见，用户以为没生成。
/// 收成一份，并把写失败冒泡给调用方。
pub(crate) struct NewImage<'a> {
    pub id: &'a str,
    pub task_id: &'a str,
    pub path: &'a str,
    pub thumb_path: &'a str,
    pub prompt: &'a str,
    pub model: &'a str,
    pub params_json: &'a str,
    pub width: i64,
    pub height: i64,
    pub size: i64,
}

/// 写入画廊一行；失败返回 `AppError::Db`（不可重试，见 error.rs）。
pub(crate) fn insert_image(conn: &rusqlite::Connection, img: &NewImage) -> AppResult<()> {
    conn.execute(
        "INSERT INTO images (id, task_id, path, thumb_path, prompt, model, params_json, favorite, width, height, size, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,0,?8,?9,?10,?11)",
        params![
            img.id,
            img.task_id,
            img.path,
            img.thumb_path,
            img.prompt,
            img.model,
            img.params_json,
            img.width,
            img.height,
            img.size,
            chrono::Utc::now().to_rfc3339()
        ],
    )
    .map(|_| ())
    .map_err(|e| AppError::Db(format!("images 入库失败：{e}")))
}

pub(crate) fn count_done(conn: &rusqlite::Connection, task_id: &str) -> u32 {
    conn.query_row(
        "SELECT COUNT(*) FROM images WHERE task_id = ?1",
        params![task_id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

pub(crate) fn emit_progress(app: &AppHandle, db: &Db, task_id: &str, status: &str, error: Option<String>, image: Option<ImageMeta>) {
    let (done, total) = {
        let conn = db.conn();
        let done = count_done(&conn, task_id);
        let total: u32 = conn
            .query_row("SELECT total FROM tasks WHERE id = ?1", params![task_id], |r| r.get(0))
            .unwrap_or(0);
        (done, total)
    };
    let _ = app.emit(
        "task-progress",
        &TaskProgress {
            task_id: task_id.to_string(),
            status: status.to_string(),
            done,
            total,
            error,
            image,
        },
    );
}

pub(crate) fn finish_task(app: &AppHandle, db: &Db, task_id: &str, status: &str, error: Option<String>) {
    let result = {
        let conn = db.conn();
        finish_task_row(&conn, task_id, status, error.clone())
    };
    match result {
        Ok(()) => emit_progress(app, db, task_id, status, error, None),
        // 库没写进去，界面上却显示"完成"——任务列表和 tasks 表就对不上了。
        // 这种情况直接把失败原因推给前端，让卡片显示为失败。
        Err(e) => emit_progress(
            app,
            db,
            task_id,
            "failed",
            Some(format!("任务状态写入数据库失败：{e}")),
            None,
        ),
    }
}

async fn wait_cancel(cancel: &Arc<AtomicBool>) {
    // 50ms 轮询，cancel 是 UI 操作触发的罕见事件，CPU 开销可忽略
    loop {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// 生成一张图并落库——**不依赖 AppHandle**，UI 任务队列与无头 MCP 模式共用。
///
/// 两边唯一差别：UI 侧额外 emit 进度事件、走全局并发闸门；无头侧传 `None`
/// 闸门直接跑。这样"外部调用出的图"和"手动点生成出的图"是同一份代码产出，
/// 落在同一张表、同一个目录，工作台不需要任何特殊适配就能看到。
///
/// 返回 `Ok(None)` 表示被取消（不是错误）。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_one_core(
    db: &Db,
    dir: &Path,
    task_id: &str,
    producer: &ImageProducer,
    ctx: &GenCtx,
    index: u32,
    prompt: &str,
    model: &str,
    cancel: Arc<AtomicBool>,
    inflight: Option<Arc<tokio::sync::Semaphore>>,
) -> AppResult<Option<ImageMeta>> {
    let mut last_err = String::new();

    for attempt in 1..=3 {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        // 等一个"在飞"名额；无头模式没有闸门（None）则直接跑
        let _permit = match &inflight {
            Some(sem) => {
                let permit = tokio::select! {
                    p = sem.clone().acquire_owned() => {
                        p.map_err(|e| AppError::General(format!("并发闸门异常: {e}")))?
                    }
                    _ = wait_cancel(&cancel) => return Ok(None),
                };
                Some(permit)
            }
            None => None,
        };

        let work = dispatch_producer(ctx, producer);
        tokio::pin!(work);
        let result = tokio::select! {
            r = &mut work => r,
            // cancel 分支：丢弃 work 即 abort 底层 reqwest 请求
            _ = wait_cancel(&cancel) => return Ok(None),
        };
        match result {
            Ok(out) => {
                let (path, thumb_path) = save_image(dir, task_id, index, &out)?;
                let img_id = uuid::Uuid::new_v4().to_string();
                let (w, h) = image::load_from_memory(&out.bytes)
                    .map(|i| (i.width() as i64, i.height() as i64))
                    .unwrap_or((0, 0));
                let mut params_val = serde_json::json!({
                    "size": ctx.size, "quality": ctx.quality, "seed": ctx.seed,
                });
                if let Some(meta) = &out.meta {
                    if let Some(obj) = params_val.as_object_mut() {
                        for (k, v) in meta.as_object().unwrap_or(&serde_json::Map::new()) {
                            obj.insert(k.clone(), v.clone());
                        }
                    }
                }
                let params_json = params_val.to_string();
                let inserted = {
                    let conn = db.conn();
                    insert_image(
                        &conn,
                        &NewImage {
                            id: &img_id,
                            task_id,
                            path: &path,
                            thumb_path: &thumb_path,
                            prompt,
                            model,
                            params_json: &params_json,
                            width: w,
                            height: h,
                            size: out.bytes.len() as i64,
                        },
                    )
                };
                // 图已经落盘，这里失败只可能是写库失败：包成 Db（不可重试），
                // 否则会回头重跑整条 adapter 再扣一次费。
                inserted.map_err(|e| {
                    AppError::Db(format!("图片已保存到 {path}，但写入画廊失败：{e}"))
                })?;
                return Ok(Some(ImageMeta { id: img_id, path, thumb_path }));
            }
            Err(e) => {
                last_err = e.to_string();
                // 只重试"可能自愈"的错误（连接抖动 / 429 限流 / 5xx）。
                // 超时（请求已送达上游、图还在生成）、4xx 业务错误、内容安全拦截
                // 重试一律无意义：只会拖延时间、重复扣费，并让上游同时压着多个请求。
                // MJ 是异步协议（提交 + 轮询），重试整个 adapter 等于重新提交一个
                // 付费任务，因此对它一律不重试。
                let retryable = producer.is_retryable() && e.is_retryable();
                if !retryable {
                    break;
                }
                if attempt < 3 {
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(attempt as u64)) => {},
                        _ = wait_cancel(&cancel) => return Ok(None),
                    }
                }
            }
        }
    }
    Err(AppError::General(last_err))
}

/// 建一条任务记录（UI 与无头 MCP 共用，保证两边的任务卡片长得一样）。
pub fn create_task(
    conn: &rusqlite::Connection,
    session_id: Option<&str>,
    provider_id: &str,
    protocol: &str,
    request_json: &str,
    n: u32,
) -> AppResult<String> {
    let task_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tasks (id, session_id, provider_id, protocol, request_json, status, concurrency, total, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6, ?6, ?7)",
        params![
            task_id,
            session_id,
            provider_id,
            protocol,
            request_json,
            n,
            chrono::Utc::now().to_rfc3339()
        ],
    )?;
    Ok(task_id)
}

/// 进程重启 = 所有 in-process worker 已死，库里残留的 `status='running'`
/// 不可能再有人推进，必须批量收尾，否则 UI 永远卡在「生成中」无法清理。
/// 有产出的标 completed_with_errors（图片保留），没产出的标 canceled。
pub fn reclaim_orphan_tasks(conn: &rusqlite::Connection) -> AppResult<u32> {
    let n = conn.execute(
        "UPDATE tasks SET \
           status = CASE WHEN (SELECT COUNT(*) FROM images i WHERE i.task_id = tasks.id) > 0 \
                         THEN 'completed_with_errors' ELSE 'canceled' END, \
           error = COALESCE(NULLIF(error,''), \
                     CASE WHEN (SELECT COUNT(*) FROM images i WHERE i.task_id = tasks.id) > 0 \
                          THEN '进程重启，任务中断（已产出的图片已保留）' \
                          ELSE '进程重启，遗留任务已自动取消' END), \
           finished_at = COALESCE(finished_at, ?1) \
         WHERE status='running'",
        params![chrono::Utc::now().to_rfc3339()],
    )?;
    Ok(n as u32)
}

/// 任务收尾（UI 与无头 MCP 共用）。
pub fn finish_task_row(
    conn: &rusqlite::Connection,
    task_id: &str,
    status: &str,
    error: Option<String>,
) -> AppResult<()> {
    conn.execute(
        "UPDATE tasks SET status=?2, error=?3, finished_at=?4 WHERE id=?1",
        params![task_id, status, error, chrono::Utc::now().to_rfc3339()],
    )
    .map(|_| ())
    .map_err(|e| AppError::Db(format!("tasks 收尾写入失败：{e}")))
}

pub fn read_file_b64(images_root: &Path, rel_or_abs: &str) -> AppResult<String> {
    let path = PathBuf::from(rel_or_abs);
    let path = if path.is_absolute() { path } else { images_root.join(rel_or_abs) };
    let canonical = path.canonicalize().map_err(|e| AppError::General(e.to_string()))?;
    let root_canonical = images_root.canonicalize().unwrap_or_else(|_| images_root.to_path_buf());
    if !canonical.starts_with(&root_canonical) {
        return Err(AppError::General("路径越界".into()));
    }
    let bytes = std::fs::read(&canonical)?;
    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    };
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::{create_task, finish_task_row, insert_image, reclaim_orphan_tasks, NewImage};
    use crate::error::AppError;
    use rusqlite::{params, Connection};

    fn sample<'a>(id: &'a str, task_id: &'a str) -> NewImage<'a> {
        NewImage {
            id,
            task_id,
            path: "/tmp/images/task-1/00-id.png",
            thumb_path: "/tmp/images/task-1/00-id.thumb.jpg",
            prompt: "一只猫",
            model: "gpt-image-1",
            params_json: "{}",
            width: 1024,
            height: 1024,
            size: 1234,
        }
    }

    fn mark_running(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO tasks (id, session_id, provider_id, protocol, request_json, status, concurrency, total, created_at)
             VALUES (?1, NULL, 'p-1', 'openai_images', '{}', 'running', 1, 1, '2026-01-01T00:00:00+00:00')",
            params![id],
        )
        .unwrap();
    }

    #[test]
    fn image_insert_writes_a_row() {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        mark_running(&conn, "task-1");
        insert_image(&conn, &sample("img-1", "task-1")).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn image_insert_without_parent_task_is_rejected() {
        // images.task_id 是真外键：任务行没建就想入库图片，数据库直接拒
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        let err = insert_image(&conn, &sample("img-x", "no-such-task")).unwrap_err();
        assert!(
            err.to_string().contains("FOREIGN KEY"),
            "外键必须拦得住，实际：{err}"
        );
    }

    #[test]
    fn image_insert_failure_is_db_error_and_not_retryable() {
        // 没建表：模拟"图片已落盘但入库失败"。这条错误必须冒出来，
        // 否则文件在磁盘上、画廊里看不见，用户以为图没生成。
        let conn = Connection::open_in_memory().unwrap();
        let err = insert_image(&conn, &sample("img-2", "task-1")).unwrap_err();
        assert!(matches!(err, AppError::Db(_)), "实际：{err}");
        assert!(!err.is_retryable(), "重试整条 adapter 会重复扣费，入库失败一律不可重试");
    }

    #[test]
    fn finish_task_row_reports_write_failure() {
        let conn = Connection::open_in_memory().unwrap();
        let err = finish_task_row(&conn, "task-1", "completed", None).unwrap_err();
        assert!(err.to_string().contains("no such table"), "实际：{err}");
    }

    #[test]
    fn finish_task_row_writes_status() {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        let task_id = crate::generate::create_task(&conn, None, "p-1", "openai_images", "{}", 1).unwrap();
        finish_task_row(&conn, &task_id, "failed", Some("上游炸了".into())).unwrap();
        let (status, error): (String, Option<String>) = conn
            .query_row(
                "SELECT status, error FROM tasks WHERE id = ?1",
                [task_id.as_str()],
                |r| Ok((r.get(0).unwrap(), r.get(1).unwrap())),
            )
            .unwrap();
        assert_eq!(status, "failed");
        assert_eq!(error.as_deref(), Some("上游炸了"));
    }

    #[test]
    fn orphan_running_task_without_image_is_canceled() {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        mark_running(&conn, "t-empty");
        let n = reclaim_orphan_tasks(&conn).unwrap();
        assert_eq!(n, 1);
        let status: String = conn
            .query_row("SELECT status FROM tasks WHERE id = ?1", ["t-empty"], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "canceled");
    }

    #[test]
    fn orphan_task_with_produced_images_keeps_completed_with_errors() {
        // 已经出好图的任务不能被误报成纯失败：图还在磁盘上，只是没跑满 n 张
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        mark_running(&conn, "t-partial");
        insert_image(&conn, &sample("img-partial", "t-partial")).unwrap();
        let n = reclaim_orphan_tasks(&conn).unwrap();
        assert_eq!(n, 1);
        let (status, error): (String, String) = conn
            .query_row(
                "SELECT status, COALESCE(error,'') FROM tasks WHERE id = ?1",
                ["t-partial"],
                |r| Ok((r.get(0).unwrap(), r.get(1).unwrap())),
            )
            .unwrap();
        assert_eq!(status, "completed_with_errors");
        assert!(error.contains("已产出的图片已保留"), "实际：{error}");
    }

    #[test]
    fn reclaim_orphan_tasks_reports_write_failure() {
        let conn = Connection::open_in_memory().unwrap();
        let err = reclaim_orphan_tasks(&conn).unwrap_err();
        assert!(err.to_string().contains("no such table"), "实际：{err}");
    }

    fn session_row(conn: &Connection) {
        conn.execute(
            "INSERT INTO sessions (id, name, created_at, updated_at) VALUES ('s1','会话','now','now')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn create_task_persists_session_id() {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        session_row(&conn);
        let id = create_task(&conn, Some("s1"), "p-1", "openai_images", "{}", 1).unwrap();
        let sid: Option<String> = conn
            .query_row("SELECT session_id FROM tasks WHERE id=?1", [id.as_str()], |r| r.get(0))
            .unwrap();
        assert_eq!(sid.as_deref(), Some("s1"));
    }

    #[test]
    fn create_task_with_null_session_id_succeeds() {
        // MCP 与旧前端没有会话概念，列必须继续可空
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        let id = create_task(&conn, None, "p-1", "openai_images", "{}", 1).unwrap();
        let sid: Option<String> = conn
            .query_row("SELECT session_id FROM tasks WHERE id=?1", [id.as_str()], |r| r.get(0))
            .unwrap();
        assert_eq!(sid, None);
    }

    #[test]
    fn create_task_with_unknown_session_id_is_rejected_by_fk() {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        let err = create_task(&conn, Some("gone"), "p-1", "openai_images", "{}", 1).unwrap_err();
        assert!(
            err.to_string().contains("FOREIGN KEY"),
            "外键必须真的拦住陈旧会话 id，实际：{err}"
        );
    }

    #[test]
    fn deleting_session_clears_task_session_id_but_keeps_task() {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        session_row(&conn);
        let id = create_task(&conn, Some("s1"), "p-1", "openai_images", "{}", 1).unwrap();
        conn.execute("DELETE FROM sessions WHERE id='s1'", []).unwrap();
        let sid: Option<String> = conn
            .query_row("SELECT session_id FROM tasks WHERE id=?1", [id.as_str()], |r| r.get(0))
            .unwrap();
        assert_eq!(sid, None, "删会话只清归属，任务与图片都要留下");
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}
