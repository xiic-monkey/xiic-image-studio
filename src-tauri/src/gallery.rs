use crate::error::{AppError, AppResult};
use crate::storage::Db;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize)]
pub struct GalleryItem {
    pub id: String,
    pub path: String,
    pub thumb_path: String,
    pub prompt: String,
    pub model: String,
    pub favorite: bool,
    pub width: i64,
    pub height: i64,
    pub size: i64,
    pub params: serde_json::Value,
    pub created_at: String,
}

fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<GalleryItem> {
    let params_json: String = row.get(9)?;
    Ok(GalleryItem {
        id: row.get(0)?,
        path: row.get(1)?,
        thumb_path: row.get(2)?,
        prompt: row.get(3)?,
        model: row.get(4)?,
        favorite: row.get::<_, i64>(5)? != 0,
        width: row.get(6)?,
        height: row.get(7)?,
        size: row.get(8)?,
        params: serde_json::from_str(&params_json).unwrap_or_default(),
        created_at: row.get(10)?,
    })
}

#[tauri::command]
pub fn image_list(
    db: State<Db>,
    q: Option<String>,
    favorite_only: Option<bool>,
    model: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> AppResult<Vec<GalleryItem>> {
    let conn = db.0.lock().unwrap();
    let mut sql = String::from(
        "SELECT id, path, thumb_path, prompt, model, favorite, width, height, size, params_json, created_at FROM images WHERE 1=1",
    );
    let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(qv) = q.filter(|s| !s.trim().is_empty()) {
        sql.push_str(&format!(" AND prompt LIKE ?{}", args.len() + 1));
        args.push(Box::new(format!("%{}%", qv.trim())));
    }
    if favorite_only.unwrap_or(false) {
        sql.push_str(" AND favorite = 1");
    }
    if let Some(m) = model.filter(|s| !s.is_empty()) {
        sql.push_str(&format!(" AND model = ?{}", args.len() + 1));
        args.push(Box::new(m));
    }
    sql.push_str(&format!(
        " ORDER BY created_at DESC LIMIT {} OFFSET {}",
        limit.unwrap_or(60).clamp(1, 500),
        offset.unwrap_or(0)
    ));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())),
        row_to_item,
    )?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn image_set_favorite(db: State<Db>, id: String, favorite: bool) -> AppResult<()> {
    let conn = db.0.lock().unwrap();
    conn.execute(
        "UPDATE images SET favorite = ?2 WHERE id = ?1",
        params![id, favorite as i64],
    )?;
    Ok(())
}

#[tauri::command]
pub fn image_delete(db: State<Db>, id: String) -> AppResult<()> {
    let (path, thumb) = {
        let conn = db.0.lock().unwrap();
        let found: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT path, thumb_path FROM images WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        conn.execute("DELETE FROM images WHERE id = ?1", params![id])?;
        found.ok_or_else(|| AppError::General("图片不存在".into()))?
    };
    let _ = std::fs::remove_file(&path);
    if let Some(t) = thumb {
        let _ = std::fs::remove_file(t);
    }
    Ok(())
}

/// 在 Finder / 资源管理器中显示原图
#[tauri::command]
pub fn image_reveal(app: AppHandle, id: String, db: State<Db>) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let conn = db.0.lock().unwrap();
    let path: String = conn
        .query_row("SELECT path FROM images WHERE id = ?1", params![id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| AppError::General("图片不存在".into()))?;
    drop(conn);
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| AppError::General(e.to_string()))?;
    Ok(())
}
