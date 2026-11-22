use crate::error::{AppError, AppResult};
use crate::storage::Db;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, State};

/// 导出上限。画廊列表一次最多 500 条，导出要覆盖「筛选后的全部」，放到 5000。
pub const MAX_EXPORT_ROWS: u32 = 5000;

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

/// 画廊筛选条件。列表与导出走同一套条件，导出的才是「你正在看的那批图」。
#[derive(Debug, Clone, Default)]
pub struct GalleryFilter {
    pub q: Option<String>,
    pub favorite_only: bool,
    pub model: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

/// 导出的一行。字段名刻意与前端 `lib/batch-parse.ts` 的 BatchItem 对齐
/// （prompt / size / quality / seed），这样导出的文件能被「导入批量」原样读回，
/// 形成「导出 → 再批量跑一遍」的闭环，而不是一份只能看的存档。
#[derive(Debug, Clone, Serialize)]
pub struct ExportRow {
    pub prompt: String,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub seed: Option<u64>,
    pub model: String,
    pub path: String,
    pub created_at: String,
    pub favorite: bool,
}

fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<GalleryItem> {
    let params_json: String = row.get(9)?;
    // width/height/size 三列可空。按 i64 直接取遇 NULL 会整行报错，
    // 而上层 filter_map(|r| r.ok()) 会把错误行静默丢掉——历史数据缺这三列时
    // 图就凭空消失。缺值按 0 处理，图照样能显示。
    let num = |i: usize| row.get::<_, Option<i64>>(i).ok().flatten().unwrap_or(0);
    Ok(GalleryItem {
        id: row.get(0)?,
        path: row.get(1)?,
        // thumb_path 同样可空（历史行没缩略图），空串表示"没有缩略图"
        thumb_path: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        prompt: row.get(3)?,
        model: row.get(4)?,
        favorite: row.get::<_, i64>(5).unwrap_or(0) != 0,
        width: num(6),
        height: num(7),
        size: num(8),
        params: serde_json::from_str(&params_json).unwrap_or_default(),
        created_at: row.get(10)?,
    })
}

/// 按筛选条件取图。列表与导出共用，避免两处 SQL 漂移。
pub fn query_images(
    conn: &rusqlite::Connection,
    filter: &GalleryFilter,
) -> AppResult<Vec<GalleryItem>> {
    let mut sql = String::from(
        "SELECT id, path, thumb_path, prompt, model, favorite, width, height, size, params_json, created_at FROM images WHERE 1=1",
    );
    let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(qv) = filter.q.as_ref().filter(|s| !s.trim().is_empty()) {
        sql.push_str(&format!(" AND prompt LIKE ?{}", args.len() + 1));
        args.push(Box::new(format!("%{}%", qv.trim())));
    }
    if filter.favorite_only {
        sql.push_str(" AND favorite = 1");
    }
    if let Some(m) = filter.model.as_ref().filter(|s| !s.is_empty()) {
        sql.push_str(&format!(" AND model = ?{}", args.len() + 1));
        args.push(Box::new(m.clone()));
    }
    sql.push_str(&format!(
        " ORDER BY created_at DESC LIMIT {} OFFSET {}",
        filter.limit.clamp(1, MAX_EXPORT_ROWS),
        filter.offset
    ));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())),
        row_to_item,
    )?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// 从画廊项取出导出所需的生成参数。params_json 里存的是实际下发的
/// size / quality / seed（缺失即 None），导出要照实反映，不替用户补默认值。
pub fn export_rows(items: &[GalleryItem]) -> Vec<ExportRow> {
    items
        .iter()
        .map(|it| {
            let str_of = |k: &str| {
                it.params
                    .get(k)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            };
            ExportRow {
                prompt: it.prompt.clone(),
                size: str_of("size"),
                quality: str_of("quality"),
                seed: it.params.get("seed").and_then(|v| v.as_u64()),
                model: it.model.clone(),
                path: it.path.clone(),
                created_at: it.created_at.clone(),
                favorite: it.favorite,
            }
        })
        .collect()
}

/// 把导出行序列化成 JSONL（每行一个 JSON 对象）。
pub fn rows_to_jsonl(rows: &[ExportRow]) -> AppResult<String> {
    let mut out = String::new();
    for r in rows {
        out.push_str(
            &serde_json::to_string(r)
                .map_err(|e| AppError::General(format!("序列化导出行失败：{e}")))?,
        );
        out.push('\n');
    }
    Ok(out)
}

/// 写出 JSONL 文件。只接受 .jsonl / .ndjson 后缀：路径由用户通过
/// 文件对话框给出，限制后缀能挡住「导出」被误当成「覆盖别的文件」。
pub fn write_jsonl(path: &str, rows: &[ExportRow]) -> AppResult<usize> {
    let p = std::path::Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "jsonl" | "ndjson") {
        return Err(AppError::General(
            "导出文件必须是 .jsonl 或 .ndjson 后缀".into(),
        ));
    }
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            return Err(AppError::General(format!(
                "目标目录不存在：{}",
                parent.to_string_lossy()
            )));
        }
    }
    let content = rows_to_jsonl(rows)?;
    std::fs::write(p, content)?;
    Ok(rows.len())
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
    let filter = GalleryFilter {
        q,
        favorite_only: favorite_only.unwrap_or(false),
        model,
        limit: limit.unwrap_or(60).clamp(1, 500),
        offset: offset.unwrap_or(0),
    };
    let conn = db.conn();
    query_images(&conn, &filter)
}

/// 把当前筛选条件下的图导出成 JSONL 文件，返回写出的行数。
#[tauri::command]
pub fn gallery_export_jsonl(
    db: State<Db>,
    path: String,
    q: Option<String>,
    favorite_only: Option<bool>,
    model: Option<String>,
    limit: Option<u32>,
) -> AppResult<usize> {
    let filter = GalleryFilter {
        q,
        favorite_only: favorite_only.unwrap_or(false),
        model,
        limit: limit.unwrap_or(MAX_EXPORT_ROWS).clamp(1, MAX_EXPORT_ROWS),
        offset: 0,
    };
    let rows = {
        let conn = db.conn();
        let items = query_images(&conn, &filter)?;
        export_rows(&items)
    };
    write_jsonl(&path, &rows)
}

#[tauri::command]
pub fn image_set_favorite(db: State<Db>, id: String, favorite: bool) -> AppResult<()> {
    let conn = db.conn();
    conn.execute(
        "UPDATE images SET favorite = ?2 WHERE id = ?1",
        params![id, favorite as i64],
    )?;
    Ok(())
}

#[tauri::command]
pub fn image_delete(db: State<Db>, id: String) -> AppResult<()> {
    let (path, thumb) = {
        let conn = db.conn();
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
    let conn = db.conn();
    let path: String = conn
        .query_row("SELECT path FROM images WHERE id = ?1", params![id], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or_else(|| AppError::General("图片不存在".into()))?;
    drop(conn);
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| AppError::General(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        export_rows, query_images, rows_to_jsonl, write_jsonl, GalleryFilter, GalleryItem,
        MAX_EXPORT_ROWS,
    };
    use rusqlite::Connection;

    fn item(prompt: &str, params: serde_json::Value) -> GalleryItem {
        GalleryItem {
            id: "i1".into(),
            path: "/tmp/a.png".into(),
            thumb_path: "/tmp/a.thumb.jpg".into(),
            prompt: prompt.into(),
            model: "gpt-image-1".into(),
            favorite: false,
            width: 1024,
            height: 1024,
            size: 100,
            params,
            created_at: "2026-01-01T00:00:00+00:00".into(),
        }
    }

    fn seed_db(conn: &Connection) {
        crate::storage::init(conn).unwrap();
        // width/height/size 三列可空，但 row_to_item 按 i64 取值——留 NULL 会让这行
        // 被 row.get 判错、再被 filter_map(|r| r.ok()) 静默丢掉。入库一律写全。
        conn.execute(
            "INSERT INTO images (id, path, thumb_path, prompt, model, params_json, favorite, width, height, size, created_at)
             VALUES ('i1','/tmp/a.png','/tmp/a.thumb.jpg','一只猫','gpt-image-1','{\"size\":\"1024x1024\",\"quality\":\"high\",\"seed\":42}',0,1024,1024,100,'2026-01-01T00:00:00+00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO images (id, path, thumb_path, prompt, model, params_json, favorite, width, height, size, created_at)
             VALUES ('i2','/tmp/b.png','/tmp/b.thumb.jpg','一只狗','gpt-image-1','{}',1,512,512,50,'2026-01-02T00:00:00+00:00')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn query_filters_by_prompt_and_favorite() {
        let conn = Connection::open_in_memory().unwrap();
        seed_db(&conn);
        let all = query_images(
            &conn,
            &GalleryFilter {
                limit: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(all.len(), 2);

        let by_q = query_images(
            &conn,
            &GalleryFilter {
                q: Some("猫".into()),
                limit: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_q.len(), 1);
        assert_eq!(by_q[0].prompt, "一只猫");

        let fav = query_images(
            &conn,
            &GalleryFilter {
                favorite_only: true,
                limit: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(fav.len(), 1);
        assert_eq!(fav[0].prompt, "一只狗");
    }

    #[test]
    fn export_rows_carry_generation_params() {
        let rows = export_rows(&[item(
            "一只猫",
            serde_json::json!({"size":"1024x1024","quality":"high","seed":42}),
        )]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].prompt, "一只猫");
        assert_eq!(rows[0].size.as_deref(), Some("1024x1024"));
        assert_eq!(rows[0].quality.as_deref(), Some("high"));
        assert_eq!(rows[0].seed, Some(42));
    }

    #[test]
    fn export_rows_leave_missing_params_empty() {
        // 旧图 / MJ 放大图没有 size/quality/seed，导出要照实留空，不补默认值
        let rows = export_rows(&[item("一只猫", serde_json::json!({}))]);
        assert_eq!(rows[0].size, None);
        assert_eq!(rows[0].quality, None);
        assert_eq!(rows[0].seed, None);
    }

    #[test]
    fn jsonl_is_one_object_per_line_and_round_trips_params() {
        let rows = export_rows(&[item(
            "一只猫",
            serde_json::json!({"size":"1024x1024","quality":"high","seed":42}),
        )]);
        let text = rows_to_jsonl(&rows).unwrap();
        assert_eq!(text.lines().count(), 1);
        // 字段名必须与前端 batch-parse 的 BatchItem 对齐，否则导回不去
        let v: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(v["prompt"], "一只猫");
        assert_eq!(v["size"], "1024x1024");
        assert_eq!(v["quality"], "high");
        assert_eq!(v["seed"], 42);
    }

    #[test]
    fn write_jsonl_rejects_non_jsonl_suffix() {
        let dir = std::env::temp_dir().join(format!("xiic-gallery-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("a.txt");
        let err = write_jsonl(bad.to_str().unwrap(), &[]).unwrap_err();
        assert!(err.to_string().contains(".jsonl"), "实际：{err}");
    }

    #[test]
    fn write_jsonl_reports_missing_directory() {
        let dir = std::env::temp_dir().join(format!("xiic-gallery-{}", uuid::Uuid::new_v4()));
        let missing = dir.join("nope").join("a.jsonl");
        let err = write_jsonl(missing.to_str().unwrap(), &[]).unwrap_err();
        assert!(err.to_string().contains("目录不存在"), "实际：{err}");
    }

    #[test]
    fn write_jsonl_creates_file_and_reports_count() {
        let dir = std::env::temp_dir().join(format!("xiic-gallery-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.jsonl");
        let rows = export_rows(&[item("一只猫", serde_json::json!({}))]);
        let n = write_jsonl(path.to_str().unwrap(), &rows).unwrap();
        assert_eq!(n, 1);
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 1);
    }

    #[test]
    fn export_limit_is_capped() {
        let conn = Connection::open_in_memory().unwrap();
        seed_db(&conn);
        let items = query_images(
            &conn,
            &GalleryFilter {
                limit: MAX_EXPORT_ROWS + 100,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn rows_with_null_metrics_are_not_silently_dropped() {
        // width/height/size/thumb_path 可空。这几列是 NULL 的历史行不能被
        // filter_map 吞掉——那会让图在画廊里凭空消失。
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        conn.execute(
            "INSERT INTO images (id, path, prompt, model, created_at)
             VALUES ('i-null','/tmp/a.png','一只猫','gpt-image-1','now')",
            [],
        )
        .unwrap();
        let items = query_images(
            &conn,
            &GalleryFilter {
                limit: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(items.len(), 1, "缺尺寸的行不能被丢掉");
        assert_eq!(items[0].width, 0);
        assert_eq!(items[0].thumb_path, "");
    }
}
