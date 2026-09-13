use crate::error::AppResult;
use crate::storage::Db;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct SessionItem {
    pub id: String,
    pub name: String,
    pub draft: serde_json::Value,
    pub updated_at: String,
}

#[tauri::command]
pub fn session_list(db: State<Db>) -> AppResult<Vec<SessionItem>> {
    let conn = db.0.lock().unwrap();
    let mut stmt = conn.prepare(
        // 按创建时间升序：新会话永远排在最右（浏览器 tab 语义）。
        // 不用 updated_at——草稿自动保存会频繁 UPDATE，tab 顺序会跳动。
        "SELECT id, name, draft_json, updated_at FROM sessions ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        let draft_json: String = r.get(2)?;
        Ok(SessionItem {
            id: r.get(0)?,
            name: r.get(1)?,
            draft: serde_json::from_str(&draft_json).unwrap_or_default(),
            updated_at: r.get(3)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[derive(Debug, Deserialize)]
pub struct SessionSaveInput {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub draft: Option<serde_json::Value>,
}

/// 创建 / 更新（改名 / 存草稿）。草稿自动保存由前端 debounce 调用。
#[tauri::command]
pub fn session_save(db: State<Db>, session: SessionSaveInput) -> AppResult<String> {
    let now = chrono::Utc::now().to_rfc3339();
    let id = session.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let conn = db.0.lock().unwrap();
    let exists: Option<String> = conn
        .query_row("SELECT id FROM sessions WHERE id = ?1", params![id], |r| r.get(0))
        .optional()?;
    if exists.is_some() {
        conn.execute(
            "UPDATE sessions SET
                name = COALESCE(?2, name),
                draft_json = COALESCE(?3, draft_json),
                updated_at = ?4
             WHERE id = ?1",
            params![
                id,
                session.name,
                session.draft.map(|d| d.to_string()),
                now
            ],
        )?;
    } else {
        conn.execute(
            "INSERT INTO sessions (id, name, draft_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![
                id,
                session.name.unwrap_or_else(|| "未命名会话".into()),
                session.draft.map(|d| d.to_string()).unwrap_or_else(|| "{}".into()),
                now
            ],
        )?;
    }
    Ok(id)
}

#[tauri::command]
pub fn session_delete(db: State<Db>, id: String) -> AppResult<()> {
    let conn = db.0.lock().unwrap();
    conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
    Ok(())
}
