// crypto / error / generate / provider 设为 pub：examples/（如 gen_icon）需要
// 复用 app 自己的生成链路（provider 配置 + 加密 key + adapters）来出图。
pub mod capabilities;
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
use provider::db as provider_db;
use sessions::{session_delete, session_list, session_save};
use std::path::PathBuf;
use storage::Db;
use tauri::Manager;
use crate::error::AppResult;

/// 读取用户通过文件对话框选中的文本文件（批量导入用）。仅读用户显式选中的路径，
/// 上限 16MB，避免误读大文件卡住 UI。
#[tauri::command]
fn read_text_file(path: String) -> AppResult<String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(crate::error::AppError::General(format!("文件不存在: {path}")));
    }
    let meta = std::fs::metadata(&p).map_err(|e| crate::error::AppError::General(e.to_string()))?;
    if meta.len() > 16 * 1024 * 1024 {
        return Err(crate::error::AppError::General("文件过大（超过 16MB），请拆小后再导入".into()));
    }
    std::fs::read_to_string(&p).map_err(|e| crate::error::AppError::General(format!("读取失败: {e}")))
}

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

            // 启动期两项写库：失败不 panic、不阻断启动（从 Finder 启动时崩在 setup 里
            // 只表现为「双击没反应」），但必须打到 stderr——此后每一次写库都会显式
            // 报错（见 generate::insert_image / finish_task_row），不再静默吞掉。
            if let Some(db) = app.try_state::<Db>() {
                match db.0.lock() {
                    Ok(conn) => {
                        if let Err(e) = generate::reclaim_orphan_tasks(&conn) {
                            eprintln!(
                                "[startup] 遗留任务清理失败，任务队列可能仍显示「生成中」：{e}"
                            );
                        }
                        match provider_db::encrypt_plaintext_keys(&conn) {
                            Ok(0) => {}
                            Ok(n) => eprintln!("[startup] 已把 {n} 个明文 API Key 加密回写"),
                            Err(e) => {
                                eprintln!("[startup] API Key 加密回写失败（下次启动会重试）：{e}")
                            }
                        }
                    }
                    Err(e) => eprintln!(
                        "[startup] 数据库互斥锁已损坏（启动时另一个线程 panic 过）：{e}"
                    ),
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
            read_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
