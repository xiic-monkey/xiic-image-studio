use crate::error::AppResult;
use crate::provider::types::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeMap;

const COLS: &str = "id, name, base_url, protocol, model, custom_headers, api_key, created_at, updated_at";

fn row_to_provider(row: &rusqlite::Row) -> rusqlite::Result<Provider> {
    let protocol_str: String = row.get(3)?;
    let headers_json: String = row.get(5)?;
    Ok(Provider {
        id: row.get(0)?,
        name: row.get(1)?,
        base_url: row.get(2)?,
        protocol: Protocol::parse(&protocol_str).unwrap_or(Protocol::OpenAiImages),
        model: row.get(4)?,
        custom_headers: serde_json::from_str(&headers_json).unwrap_or_default(),
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn api_key_of(row: &rusqlite::Row) -> rusqlite::Result<String> {
    row.get(6)
}

pub fn list(conn: &Connection) -> AppResult<Vec<ProviderListItem>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM providers ORDER BY created_at ASC"
    ))?;
    let rows = stmt.query_map([], |row| {
        let p = row_to_provider(row)?;
        let has_key = !api_key_of(row)?.is_empty();
        Ok((p, has_key))
    })?;
    let mut out = Vec::new();
    for item in rows {
        let (p, has_key) = item?;
        out.push(ProviderListItem {
            id: p.id,
            name: p.name,
            base_url: p.base_url,
            protocol: p.protocol,
            model: p.model,
            custom_headers: p.custom_headers,
            has_key,
            created_at: p.created_at,
            updated_at: p.updated_at,
        });
    }
    Ok(out)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Provider>> {
    let mut stmt =
        conn.prepare(&format!("SELECT {COLS} FROM providers WHERE id = ?1"))?;
    let found = stmt.query_row(params![id], row_to_provider).optional()?;
    Ok(found)
}

/// 取已存 Key（库里是密文，取出来解密；仅后端使用，不出 IPC）
pub fn get_key(conn: &Connection, id: &str) -> AppResult<Option<String>> {
    let mut stmt = conn.prepare("SELECT api_key FROM providers WHERE id = ?1")?;
    let found: Option<String> = stmt.query_row(params![id], |r| r.get(0)).optional()?;
    match found.filter(|k| !k.is_empty()) {
        Some(k) => Ok(Some(crate::crypto::decrypt(&k)?)),
        None => Ok(None),
    }
}

/// 取 provider 及其**解密后的** api_key（UI 与无头 MCP 模式共用同一条取用路径）。
/// key 只在进程内使用，不会随任何对外响应出去。
pub fn get_with_key(conn: &Connection, id: &str) -> AppResult<Option<(Provider, String)>> {
    let Some(provider) = get(conn, id)? else {
        return Ok(None);
    };
    let Some(key) = get_key(conn, id)? else {
        return Ok(None);
    };
    Ok(Some((provider, key)))
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SaveInput {
    #[serde(default = "new_id")]
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub protocol: Protocol,
    pub model: String,
    pub custom_headers: BTreeMap<String, String>,
    /// Some(非空)=覆盖；Some("")=清除；None=保持不变
    pub api_key: Option<String>,
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn save(conn: &Connection, input: &SaveInput) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM providers WHERE id = ?1",
            params![input.id],
            |r| r.get(0),
        )
        .optional()?;
    let headers_json = serde_json::to_string(&input.custom_headers).unwrap_or_default();
    // 落库前加密：Some(非空) → 密文；Some("") = 显式清除；None = 保持原值（由 SQL 的 CASE 处理）
    let new_key = match &input.api_key {
        Some(k) if !k.is_empty() => crate::crypto::encrypt(k)?,
        _ => String::new(),
    };

    if existing.is_some() {
        // api_key：传入非空则覆盖，传空串则清除；前端编辑时传 None 表示不改
        conn.execute(
            "UPDATE providers SET name=?2, base_url=?3, protocol=?4, model=?5,
             custom_headers=?6, updated_at=?7,
             api_key = CASE WHEN ?8 = '' AND ?9 = 1 THEN api_key ELSE ?8 END
             WHERE id=?1",
            params![
                input.id,
                input.name,
                input.base_url,
                input.protocol.as_str(),
                input.model,
                headers_json,
                now,
                new_key,
                input.api_key.is_none()
            ],
        )?;
    } else {
        conn.execute(
            &format!(
                "INSERT INTO providers ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?8,?7,?7)"
            ),
            params![
                input.id,
                input.name,
                input.base_url,
                input.protocol.as_str(),
                input.model,
                headers_json,
                now,
                new_key
            ],
        )?;
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM providers WHERE id = ?1", params![id])?;
    Ok(())
}
