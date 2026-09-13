// crypto / error / generate / provider 设为 pub：examples/（如 gen_icon）需要
// 复用 app 自己的生成链路（provider 配置 + 加密 key + adapters）来出图。
pub mod crypto;
pub mod error;
pub mod generate;
/// 无头 MCP 模式（`--mcp`）：纯 stdio，不占端口。
pub mod mcp;
pub mod provider;

mod gallery;
mod mcp_protocol;
mod media;
mod prompts;
mod sessions;
mod storage;

use gallery::{image_delete, image_list, image_reveal, image_set_favorite};
use generate::commands::{generate_cancel, generate_submit, image_read_b64, mj_action, task_delete, task_list};
use generate::QueueState;
use media::{tools_convert, tools_file_b64, tools_gif, tools_set_ffmpeg_path, tools_status, tools_video_frames};
use prompts::{prompt_delete, prompt_list, prompt_save};
use provider::commands::{provider_delete, provider_discover_models, provider_list, provider_save, provider_test};
use sessions::{session_delete, session_list, session_save};
use std::path::PathBuf;
use storage::Db;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_dir: PathBuf = app
                .path()
                .app_data_dir()
                .expect("无法获取应用数据目录");
            std::fs::create_dir_all(&app_dir)?;
            let conn = rusqlite::Connection::open(storage::db_path(&app_dir))?;
            storage::init(&conn)?;
            app.manage(Db(std::sync::Mutex::new(conn)));
            app.manage(QueueState::new());

            // 启动时清理孤儿任务：进程重启意味着所有 in-process worker 都已死，
            // DB 里残留 status='running' 的行不可能有人推进，批量标记 canceled
            // （保留错误信息以备排查），否则 UI 会卡在「生成中」无法清理。
            if let Some(db) = app.try_state::<Db>() {
                if let Ok(conn) = db.0.lock() {
                    // 区分两种情况，别把"其实已经出好图"的任务错标成纯失败：
                    // - 该任务已经有产出图片 → completed_with_errors（图保留，只是没跑满 n 张）
                    // - 完全没有任何产出      → canceled
                    let _ = conn.execute(
                        "UPDATE tasks SET \
                           status = CASE WHEN (SELECT COUNT(*) FROM images i WHERE i.task_id = tasks.id) > 0 \
                                         THEN 'completed_with_errors' ELSE 'canceled' END, \
                           error = COALESCE(NULLIF(error,''), \
                                     CASE WHEN (SELECT COUNT(*) FROM images i WHERE i.task_id = tasks.id) > 0 \
                                          THEN '进程重启，任务中断（已产出的图片已保留）' \
                                          ELSE '进程重启，遗留任务已自动取消' END), \
                           finished_at = COALESCE(finished_at, ?1) \
                         WHERE status='running'",
                        rusqlite::params![chrono::Utc::now().to_rfc3339()],
                    );
                }
            }

            // 一次性迁移：把存量明文 API Key 加密回写。
            // 幂等（已加密的跳过），所以每次启动跑一遍也无害——
            // 没有这一步，"加密存储"就只对之后新保存的 key 生效，老数据继续裸奔。
            if let Some(db) = app.try_state::<Db>() {
                if let Ok(conn) = db.0.lock() {
                    let mut plain: Vec<(String, String)> = Vec::new();
                    if let Ok(mut stmt) =
                        conn.prepare("SELECT id, api_key FROM providers WHERE api_key <> ''")
                    {
                        if let Ok(rows) = stmt.query_map([], |r| {
                            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                        }) {
                            plain = rows.filter_map(|r| r.ok()).collect();
                        }
                    }
                    for (id, key) in plain {
                        if key.starts_with("enc:v1:") {
                            continue;
                        }
                        if let Ok(enc) = crate::crypto::encrypt(&key) {
                            let _ = conn.execute(
                                "UPDATE providers SET api_key = ?2 WHERE id = ?1",
                                rusqlite::params![id, enc],
                            );
                        }
                    }
                }
            }

            // 系统标题栏窗口（decorations:true）自带 macOS 圆角与阴影。
            // 显式 show()+set_focus() 防裸 binary（无 .app bundle）启动时主窗口
            // IsOnscreen=false——窗口创建了、内容也在渲染，但 macOS 没把它放上屏。
            // （startHidden + 前端 show 的写法在 dev 模式下窗口拉不上屏，已弃用。）
            #[cfg(target_os = "macos")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                    let _ = window.set_shadow(true);

                    // show()+set_focus() 不跨 Space：从 shell/后台启动时 macOS 会把
                    // 窗口放回「上次运行所在的桌面」，AX/Quartz 里窗口完全不可见，
                    // 看起来就像没启动。MoveToActiveSpace 强制窗口跟到当前桌面。
                    if let Ok(ns_win) = window.ns_window() {
                        #[link(name = "objc", kind = "dylib")]
                        extern "C" {
                            fn objc_msgSend(receiver: *mut std::ffi::c_void, sel: *const std::ffi::c_void, behavior: usize) -> *mut std::ffi::c_void;
                            fn sel_registerName(name: *const std::ffi::c_char) -> *const std::ffi::c_void;
                        }
                        unsafe {
                            let sel = sel_registerName(b"setCollectionBehavior:\0".as_ptr() as *const std::ffi::c_char);
                            // NSWindowCollectionBehaviorMoveToActiveSpace = 1 << 2
                            objc_msgSend(ns_win, sel, 1usize << 2);
                        }
                    }
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            provider_list,
            provider_save,
            provider_delete,
            provider_test,
            provider_discover_models,
            generate_submit,
            generate_cancel,
            task_list,
            task_delete,
            image_read_b64,
            image_list,
            image_set_favorite,
            image_delete,
            image_reveal,
            prompt_list,
            prompt_save,
            prompt_delete,
            session_list,
            session_save,
            session_delete,
            mj_action,
            tools_status,
            tools_convert,
            tools_gif,
            tools_video_frames,
            tools_file_b64,
            tools_set_ffmpeg_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
