pub mod adapters;
pub mod client;
pub mod commands;
pub mod types;

use crate::error::{AppError, AppResult};
use crate::generate::types::*;
use crate::provider::types::Protocol;
use crate::storage::Db;
use base64::Engine;
use futures::stream::{self, StreamExt};
use rusqlite::params;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
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
}

pub(crate) fn images_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::General(e.to_string()))?
        .join("images"))
}

fn dispatch_adapter(ctx: &GenCtx, protocol: Protocol) -> Pin<Box<dyn Future<Output = AppResult<ImageOut>> + Send + '_>> {
    match protocol {
        Protocol::OpenAiImages => Box::pin(adapters::openai_images(ctx)),
        Protocol::OpenAiChatImage => Box::pin(adapters::openai_chat_image(ctx)),
        Protocol::GeminiNative => Box::pin(adapters::gemini_native(ctx)),
        Protocol::MidjourneyProxy => Box::pin(adapters::midjourney_proxy(ctx)),
    }
}

pub(crate) fn save_image(
    dir: &PathBuf,
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
        let conn = db.0.lock().unwrap();
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
    {
        let conn = db.0.lock().unwrap();
        let _ = conn.execute(
            "UPDATE tasks SET status=?2, error=?3, finished_at=?4 WHERE id=?1",
            params![task_id, status, error, chrono::Utc::now().to_rfc3339()],
        );
    }
    emit_progress(app, db, task_id, status, error, None);
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

async fn run_one(
    app: AppHandle,
    task_id: String,
    index: u32,
    _total: u32,
    protocol: Protocol,
    ctx: GenCtx,
    prompt: String,
    model: String,
    cancel: Arc<AtomicBool>,
) -> AppResult<()> {
    let db = app.state::<Db>();
    let inflight = app.state::<QueueState>().inflight.clone();
    let dir = images_dir(&app)?;
    match run_one_core(
        &db,
        &dir,
        &task_id,
        protocol,
        &ctx,
        index,
        &prompt,
        &model,
        cancel,
        Some(inflight),
    )
    .await
    {
        Ok(Some(meta)) => {
            emit_progress(&app, &db, &task_id, "running", None, Some(meta));
            Ok(())
        }
        // 取消不算错误：spawn_task 会把它汇总成 canceled
        Ok(None) => Ok(()),
        Err(e) => {
            emit_progress(&app, &db, &task_id, "running", Some(e.to_string()), None);
            Err(e)
        }
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
    dir: &PathBuf,
    task_id: &str,
    protocol: Protocol,
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

        let work = dispatch_adapter(ctx, protocol);
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
                {
                    let conn = db.0.lock().unwrap();
                    let _ = conn.execute(
                        "INSERT INTO images (id, task_id, path, thumb_path, prompt, model, params_json, favorite, width, height, size, created_at)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,0,?8,?9,?10,?11)",
                        params![
                            img_id, task_id, path, thumb_path, prompt, model, params_json,
                            w, h, out.bytes.len() as i64, chrono::Utc::now().to_rfc3339()
                        ],
                    );
                }
                return Ok(Some(ImageMeta { id: img_id, path, thumb_path }));
            }
            Err(e) => {
                last_err = e.to_string();
                // 只重试"可能自愈"的错误（连接抖动 / 429 限流 / 5xx）。
                // 超时（请求已送达上游、图还在生成）、4xx 业务错误、内容安全拦截
                // 重试一律无意义：只会拖延时间、重复扣费，并让上游同时压着多个请求。
                // MJ 是异步协议（提交 + 轮询），重试整个 adapter 等于重新提交一个
                // 付费任务，因此对它一律不重试。
                let retryable = !matches!(protocol, Protocol::MidjourneyProxy) && e.is_retryable();
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
    provider_id: &str,
    protocol: &str,
    request_json: &str,
    n: u32,
) -> AppResult<String> {
    let task_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tasks (id, session_id, provider_id, protocol, request_json, status, concurrency, total, created_at)
         VALUES (?1, NULL, ?2, ?3, ?4, 'running', ?5, ?5, ?6)",
        params![
            task_id,
            provider_id,
            protocol,
            request_json,
            n,
            chrono::Utc::now().to_rfc3339()
        ],
    )?;
    Ok(task_id)
}

/// 任务收尾（UI 与无头 MCP 共用）。
pub fn finish_task_row(
    conn: &rusqlite::Connection,
    task_id: &str,
    status: &str,
    error: Option<String>,
) {
    let _ = conn.execute(
        "UPDATE tasks SET status=?2, error=?3, finished_at=?4 WHERE id=?1",
        params![task_id, status, error, chrono::Utc::now().to_rfc3339()],
    );
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_task(
    app: AppHandle,
    task_id: String,
    protocol: Protocol,
    ctx: GenCtx,
    n: u32,
    prompt: String,
    model: String,
) {
    let queue = app.state::<QueueState>();
    let cancel = Arc::new(AtomicBool::new(false));
    queue
        .cancels
        .lock()
        .unwrap()
        .insert(task_id.clone(), cancel.clone());
    drop(queue);

    tauri::async_runtime::spawn(async move {
        let total = n;
        let results: Vec<Result<(), AppError>> = stream::iter(0..n)
            .map(|i| {
                let app = app.clone();
                let task_id = task_id.clone();
                let cancel = cancel.clone();
                let ctx = GenCtx {
                    base_url: ctx.base_url.clone(),
                    key: ctx.key.clone(),
                    model: ctx.model.clone(),
                    custom_headers: ctx.custom_headers.clone(),
                    prompt: ctx.prompt.clone(),
                    size: ctx.size.clone(),
                    quality: ctx.quality.clone(),
                    seed: ctx.seed,
                    refs: ctx.refs.clone(),
                    mask: ctx.mask.clone(),
                    cancel: Some(cancel.clone()),
                };
                let prompt = prompt.clone();
                let model = model.clone();
                async move { run_one(app, task_id, i, total, protocol, ctx, prompt, model, cancel).await }
            })
            .buffer_unordered(n.clamp(1, 4) as usize)
            .collect()
            .await;

        let failed = results.iter().filter(|r| r.is_err()).count();
        let status = if cancel.load(Ordering::Relaxed) {
            "canceled"
        } else if failed == total as usize {
            "failed"
        } else if failed > 0 {
            "completed_with_errors"
        } else {
            "completed"
        };
        let first_err = results.iter().find_map(|r| r.as_ref().err().map(|e| e.to_string()));
        let db = app.state::<Db>();
        finish_task(&app, &db, &task_id, status, first_err);

        let queue = app.state::<QueueState>();
        queue.cancels.lock().unwrap().remove(&task_id);
    });
}

pub fn read_file_b64(images_root: &PathBuf, rel_or_abs: &str) -> AppResult<String> {
    let path = PathBuf::from(rel_or_abs);
    let path = if path.is_absolute() { path } else { images_root.join(rel_or_abs) };
    let canonical = path.canonicalize().map_err(|e| AppError::General(e.to_string()))?;
    let root_canonical = images_root.canonicalize().unwrap_or(images_root.clone());
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
