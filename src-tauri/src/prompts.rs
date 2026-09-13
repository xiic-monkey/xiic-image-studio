use crate::error::AppResult;
use crate::storage::Db;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct PromptItem {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

#[tauri::command]
pub fn prompt_list(db: State<Db>) -> AppResult<Vec<PromptItem>> {
    let conn = db.0.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, title, content, tags, created_at FROM prompts ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        let tags_json: String = r.get(3)?;
        Ok(PromptItem {
            id: r.get(0)?,
            title: r.get(1)?,
            content: r.get(2)?,
            tags: serde_json::from_str(&tags_json).unwrap_or_default(),
            created_at: r.get(4)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[derive(Debug, Deserialize)]
pub struct PromptSaveInput {
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[tauri::command]
pub fn prompt_save(db: State<Db>, prompt: PromptSaveInput) -> AppResult<String> {
    let now = chrono::Utc::now().to_rfc3339();
    let tags_json = serde_json::to_string(&prompt.tags).unwrap_or_default();
    let id = prompt.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let conn = db.0.lock().unwrap();
    let exists: Option<String> = conn
        .query_row("SELECT id FROM prompts WHERE id = ?1", params![id], |r| r.get(0))
        .optional()?;
    if exists.is_some() {
        conn.execute(
            "UPDATE prompts SET title=?2, content=?3, tags=?4 WHERE id=?1",
            params![id, prompt.title, prompt.content, tags_json],
        )?;
    } else {
        conn.execute(
            "INSERT INTO prompts (id, title, content, tags, created_at) VALUES (?1,?2,?3,?4,?5)",
            params![id, prompt.title, prompt.content, tags_json, now],
        )?;
    }
    Ok(id)
}

#[tauri::command]
pub fn prompt_delete(db: State<Db>, id: String) -> AppResult<()> {
    let conn = db.0.lock().unwrap();
    conn.execute("DELETE FROM prompts WHERE id = ?1", params![id])?;
    Ok(())
}
