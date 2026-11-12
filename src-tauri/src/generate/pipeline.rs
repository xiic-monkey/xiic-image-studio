//! GUI / 无头 MCP / MJ U-V 三条入口共用的任务编排层。
//!
//! 底层原语（落库、落盘、适配器）仍在 `super`，这里只负责「建任务之后的全部流程」。
//! 三处入口的差别收敛成 `RunMode`：无头进程没有 AppHandle，类型上就不可能误发
//! Tauri 事件，也不会误用只存在于 GUI 进程内的并发闸门。

use crate::error::AppResult;
use crate::generate::types::{
    GenCtx, GenerateRequest, ImageMeta, ImageProducer, TaskOutcome, TaskRequest, FANOUT,
};
use crate::provider::types::Provider;
use crate::storage::Db;
use futures::stream::{self, StreamExt};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::runtime::Runtime;

/// 进度/闸门/取消的物理差异。Headless 分支拿不到 AppHandle，也没有 GUI 进程的 QueueState。
pub(crate) enum RunMode {
    Gui(AppHandle),
    Headless,
}

impl RunMode {
    pub(crate) fn app(&self) -> Option<&AppHandle> {
        match self {
            RunMode::Gui(app) => Some(app),
            RunMode::Headless => None,
        }
    }

    /// Semaphore 是进程内的：MCP 与 GUI 是两个进程，本就共享不了，也不能假装在排队。
    pub(crate) fn inflight(&self) -> Option<Arc<tokio::sync::Semaphore>> {
        match self {
            RunMode::Gui(app) => Some(app.state::<super::QueueState>().inflight.clone()),
            RunMode::Headless => None,
        }
    }
}

/// 从（已按能力表裁剪的）请求构造适配器上下文。
pub fn build_ctx(provider: &Provider, key: String, req: &GenerateRequest, prompt: &str) -> GenCtx {
    GenCtx {
        base_url: provider.base_url.clone(),
        key,
        model: provider.model.clone(),
        custom_headers: provider.custom_headers.clone(),
        prompt: prompt.to_string(),
        size: req.size.clone(),
        quality: req.quality.clone(),
        seed: req.seed,
        refs: req.refs.clone(),
        mask: req.mask.clone(),
        cancel: None,
    }
}

/// prepare_task 的入参：9 个位置参数在调用点看不出谁是谁，聚合后调用方可读。
pub struct TaskSpec<'a> {
    pub session_id: Option<&'a str>,
    pub provider: &'a Provider,
    pub n: u32,
    pub request_json: &'a str,
    pub producer: ImageProducer,
    pub ctx: GenCtx,
    pub prompt: String,
    pub model: String,
}

/// 唯一建任务行的地方：写会话归属，返回可直接交给编排层的 TaskRequest。
pub fn prepare_task(conn: &rusqlite::Connection, spec: TaskSpec<'_>) -> AppResult<TaskRequest> {
    let TaskSpec {
        session_id,
        provider,
        n,
        request_json,
        producer,
        ctx,
        prompt,
        model,
    } = spec;
    let task_id = super::create_task(
        conn,
        session_id,
        &provider.id,
        producer.protocol().as_str(),
        request_json,
        n,
    )?;
    Ok(TaskRequest {
        task_id,
        n,
        prompt,
        model,
        ctx,
        producer,
    })
}

/// 任务终态：取消优先；全错才算 failed，出过图就是 completed(_with_errors)。
pub(crate) fn aggregate_status(
    image_count: usize,
    error_count: usize,
    canceled: bool,
) -> &'static str {
    if canceled {
        "canceled"
    } else if error_count == 0 {
        "completed"
    } else if image_count == 0 {
        "failed"
    } else {
        "completed_with_errors"
    }
}

/// GUI / MJ：后台跑完，进度通过 Tauri 事件推给前端，命令立即返回 task_id。
pub fn spawn_task(app: AppHandle, req: TaskRequest) {
    tauri::async_runtime::spawn(async move {
        let db = app.state::<Db>();
        let dir = match super::images_dir(&app) {
            Ok(dir) => dir,
            Err(e) => {
                super::finish_task(&app, &db, &req.task_id, "failed", Some(e.to_string()));
                return;
            }
        };
        let _ = execute(&db, &dir, &req, RunMode::Gui(app.clone())).await;
    });
}

/// 无头 MCP：在 stdio 进程的 Runtime 上跑完，结果直接回 tools/call 响应。
pub fn run_task_headless(
    db: &Db,
    images_dir: &Path,
    runtime: &Runtime,
    req: &TaskRequest,
) -> TaskOutcome {
    runtime.block_on(execute(db, images_dir, req, RunMode::Headless))
}

async fn execute(db: &Db, images_dir: &Path, req: &TaskRequest, mode: RunMode) -> TaskOutcome {
    let app = mode.app().cloned();
    let inflight = mode.inflight();
    let cancel = Arc::new(AtomicBool::new(false));
    if let Some(app) = &app {
        let queue = app.state::<super::QueueState>();
        queue.cancels().insert(req.task_id.clone(), cancel.clone());
    }

    let results: Vec<AppResult<Option<ImageMeta>>> = stream::iter(0..req.n)
        .map(|index| {
            let cancel = cancel.clone();
            let inflight = inflight.clone();
            let app = app.clone();
            async move {
                // cancel 必须带进 GenCtx：MJ 长轮询期间只认这个标志
                let mut ctx = req.ctx.clone();
                ctx.cancel = Some(cancel.clone());
                let result = super::run_one_core(
                    db,
                    images_dir,
                    &req.task_id,
                    &req.producer,
                    &ctx,
                    index,
                    &req.prompt,
                    &req.model,
                    cancel,
                    inflight,
                )
                .await;
                if let Some(app) = &app {
                    match &result {
                        Ok(Some(meta)) => super::emit_progress(
                            app,
                            db,
                            &req.task_id,
                            "running",
                            None,
                            Some(meta.clone()),
                        ),
                        Ok(None) => {}
                        Err(e) => super::emit_progress(
                            app,
                            db,
                            &req.task_id,
                            "running",
                            Some(e.to_string()),
                            None,
                        ),
                    }
                }
                result
            }
        })
        .buffer_unordered(FANOUT)
        .collect()
        .await;

    let mut images = Vec::new();
    let mut errors = Vec::new();
    for r in results {
        match r {
            Ok(Some(meta)) => images.push(meta),
            Ok(None) => {}
            Err(e) => errors.push(e.to_string()),
        }
    }
    let status = aggregate_status(images.len(), errors.len(), cancel.load(Ordering::Relaxed));

    let mut db_error = None;
    match &app {
        Some(app) => super::finish_task(app, db, &req.task_id, status, errors.first().cloned()),
        None => {
            let conn = db.conn();
            if let Err(e) =
                super::finish_task_row(&conn, &req.task_id, status, errors.first().cloned())
            {
                // 图已经生成（已扣费），记账失败只能作为 warning 返回，不能丢掉结果
                db_error = Some(e.to_string());
            }
        }
    }
    if let Some(app) = &app {
        app.state::<super::QueueState>()
            .cancels()
            .remove(&req.task_id);
    }

    TaskOutcome {
        task_id: req.task_id.clone(),
        status: status.to_string(),
        images,
        errors,
        db_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::types::Protocol;
    use rusqlite::Connection;
    use std::collections::BTreeMap;

    fn provider() -> Provider {
        Provider {
            id: "p1".into(),
            name: "供应商".into(),
            base_url: "https://example.com/v1".into(),
            protocol: Protocol::OpenAiImages,
            model: "gpt-image-1".into(),
            custom_headers: BTreeMap::from([("X-A".to_string(), "b".to_string())]),
            created_at: "now".into(),
            updated_at: "now".into(),
        }
    }

    fn request() -> GenerateRequest {
        GenerateRequest {
            provider_id: "p1".into(),
            prompt: "  cat  ".into(),
            n: 2,
            size: Some("1024x1024".into()),
            quality: Some("high".into()),
            seed: Some(7),
            refs: vec![],
            mask: None,
            session_id: Some("s1".into()),
        }
    }

    #[test]
    fn aggregate_status_all_ok_is_completed() {
        assert_eq!(aggregate_status(2, 0, false), "completed");
    }

    #[test]
    fn aggregate_status_all_failed_is_failed() {
        assert_eq!(aggregate_status(0, 2, false), "failed");
    }

    #[test]
    fn aggregate_status_partial_is_completed_with_errors() {
        assert_eq!(aggregate_status(1, 1, false), "completed_with_errors");
    }

    #[test]
    fn aggregate_status_canceled_wins() {
        assert_eq!(aggregate_status(2, 0, true), "canceled");
    }

    #[test]
    fn build_ctx_maps_all_fields_from_request_and_provider() {
        let ctx = build_ctx(&provider(), "sk".into(), &request(), "cat");
        assert_eq!(ctx.base_url, "https://example.com/v1");
        assert_eq!(ctx.key, "sk");
        assert_eq!(ctx.model, "gpt-image-1");
        assert_eq!(ctx.custom_headers.get("X-A").map(String::as_str), Some("b"));
        assert_eq!(ctx.prompt, "cat");
        assert_eq!(ctx.size.as_deref(), Some("1024x1024"));
        assert_eq!(ctx.quality.as_deref(), Some("high"));
        assert_eq!(ctx.seed, Some(7));
        assert!(ctx.refs.is_empty() && ctx.mask.is_none() && ctx.cancel.is_none());
    }

    #[test]
    fn prepare_task_writes_session_id_and_returns_task_request() {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        conn.execute(
            "INSERT INTO sessions (id, name, created_at, updated_at) VALUES ('s1','会话','now','now')",
            [],
        )
        .unwrap();
        let p = provider();
        let r = request();
        let ctx = build_ctx(&p, "sk".into(), &r, "cat");
        let spec = TaskSpec {
            session_id: r.session_id.as_deref(),
            provider: &p,
            n: 2,
            request_json: "{}",
            producer: ImageProducer::Protocol(Protocol::OpenAiImages),
            ctx,
            prompt: "cat".into(),
            model: p.model.clone(),
        };
        let task = prepare_task(&conn, spec).unwrap();
        let sid: Option<String> = conn
            .query_row(
                "SELECT session_id FROM tasks WHERE id=?1",
                [task.task_id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sid.as_deref(), Some("s1"));
        assert_eq!(task.n, 2);
        assert_eq!(task.prompt, "cat");
    }

    #[test]
    fn headless_mode_has_no_app_and_no_inflight() {
        let mode = RunMode::Headless;
        assert!(mode.app().is_none());
        assert!(mode.inflight().is_none());
    }
}
