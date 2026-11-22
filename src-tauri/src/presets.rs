use crate::error::AppResult;
use crate::storage::Db;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

/// 一份可复用的生成配方：提示词 + 尺寸 + 质量 + 种子 + 供应商。
///
/// 与 `prompts` 表分开：提示词库存的是「一段文本」，配方存的是
/// 「文本 + 能让它复现的参数组合」。混在一张表里，套用时就得
/// 从标题/标签里反推参数，迟早出偏差。
#[derive(Debug, Clone, Serialize)]
pub struct PresetItem {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub seed: Option<u64>,
    pub provider_id: Option<String>,
    pub created_at: String,
}

/// 列出配方（核心逻辑；命令层只包一层 State）。
pub fn list_presets(conn: &rusqlite::Connection) -> AppResult<Vec<PresetItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, prompt, size, quality, seed, provider_id, created_at
         FROM presets ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(PresetItem {
            id: r.get(0)?,
            name: r.get(1)?,
            prompt: r.get(2)?,
            size: r.get(3)?,
            quality: r.get(4)?,
            seed: r.get(5)?,
            provider_id: r.get(6)?,
            created_at: r.get(7)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn preset_list(db: State<Db>) -> AppResult<Vec<PresetItem>> {
    let conn = db.conn();
    list_presets(&conn)
}

#[derive(Debug, Deserialize)]
pub struct PresetSaveInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub prompt: String,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub provider_id: Option<String>,
}

/// 创建 / 更新一份配方。名字和提示词都不能为空——空配方的意义只是噪声。
pub fn save_preset(conn: &rusqlite::Connection, preset: PresetSaveInput) -> AppResult<String> {
    let name = preset.name.trim().to_string();
    if name.is_empty() {
        return Err(crate::error::AppError::General("配方名不能为空".into()));
    }
    let prompt = preset.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(crate::error::AppError::General("配方提示词不能为空".into()));
    }
    let now = chrono::Utc::now().to_rfc3339();
    let id = preset
        .id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let exists: Option<String> = conn
        .query_row("SELECT id FROM presets WHERE id = ?1", params![id], |r| {
            r.get(0)
        })
        .optional()?;
    if exists.is_some() {
        conn.execute(
            "UPDATE presets SET name=?2, prompt=?3, size=?4, quality=?5, seed=?6, provider_id=?7 WHERE id=?1",
            params![
                id,
                name,
                prompt,
                preset.size,
                preset.quality,
                preset.seed,
                preset.provider_id
            ],
        )?;
    } else {
        conn.execute(
            "INSERT INTO presets (id, name, prompt, size, quality, seed, provider_id, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                id,
                name,
                prompt,
                preset.size,
                preset.quality,
                preset.seed,
                preset.provider_id,
                now
            ],
        )?;
    }
    Ok(id)
}

#[tauri::command]
pub fn preset_save(db: State<Db>, preset: PresetSaveInput) -> AppResult<String> {
    let conn = db.conn();
    save_preset(&conn, preset)
}

#[tauri::command]
pub fn preset_delete(db: State<Db>, id: String) -> AppResult<()> {
    let conn = db.conn();
    conn.execute("DELETE FROM presets WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{list_presets, save_preset, PresetSaveInput};
    use rusqlite::Connection;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        conn
    }

    fn input(name: &str, prompt: &str) -> PresetSaveInput {
        PresetSaveInput {
            id: None,
            name: name.into(),
            prompt: prompt.into(),
            size: Some("1024x1024".into()),
            quality: Some("high".into()),
            seed: Some(42),
            provider_id: Some("p1".into()),
        }
    }

    #[test]
    fn preset_round_trips_through_db() {
        let conn = conn();
        let id = save_preset(&conn, input("电商白底", "一只猫")).unwrap();
        let items = list_presets(&conn).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, id);
        assert_eq!(items[0].name, "电商白底");
        assert_eq!(items[0].prompt, "一只猫");
        assert_eq!(items[0].size.as_deref(), Some("1024x1024"));
        assert_eq!(items[0].quality.as_deref(), Some("high"));
        assert_eq!(items[0].seed, Some(42));
        assert_eq!(items[0].provider_id.as_deref(), Some("p1"));
    }

    #[test]
    fn preset_update_reuses_the_same_row() {
        let conn = conn();
        let id = save_preset(&conn, input("旧名", "猫")).unwrap();
        let mut upd = input("新名", "狗");
        upd.id = Some(id.clone());
        save_preset(&conn, upd).unwrap();
        let items = list_presets(&conn).unwrap();
        assert_eq!(items.len(), 1, "更新不该多出一行");
        assert_eq!(items[0].name, "新名");
        assert_eq!(items[0].prompt, "狗");
    }

    #[test]
    fn preset_rejects_blank_name_and_prompt() {
        let conn = conn();
        let err = save_preset(&conn, input("  ", "猫")).unwrap_err();
        assert!(err.to_string().contains("配方名"), "实际：{err}");
        let err = save_preset(&conn, input("名字", "   ")).unwrap_err();
        assert!(err.to_string().contains("提示词"), "实际：{err}");
    }

    #[test]
    fn preset_delete_removes_only_that_row() {
        let conn = conn();
        let a = save_preset(&conn, input("A", "猫")).unwrap();
        let b = save_preset(&conn, input("B", "狗")).unwrap();
        conn.execute("DELETE FROM presets WHERE id = ?1", rusqlite::params![a])
            .unwrap();
        let items = list_presets(&conn).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, b);
    }

    #[test]
    fn preset_optional_fields_stay_empty() {
        let conn = conn();
        let mut p = input("裸配方", "猫");
        p.size = None;
        p.quality = None;
        p.seed = None;
        p.provider_id = None;
        save_preset(&conn, p).unwrap();
        let items = list_presets(&conn).unwrap();
        assert_eq!(items[0].size, None);
        assert_eq!(items[0].quality, None);
        assert_eq!(items[0].seed, None);
        assert_eq!(items[0].provider_id, None);
    }
}
