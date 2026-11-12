use crate::error::AppResult;
use crate::provider::types::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeMap;

const COLS: &str =
    "id, name, base_url, protocol, model, custom_headers, api_key, created_at, updated_at";

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
        // 先算能力再进结构体字面量：字面量里 model: p.model 会把 p.model 移走
        let capabilities = crate::capabilities::for_model(p.protocol, &p.model);
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
            capabilities,
        });
    }
    Ok(out)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Provider>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM providers WHERE id = ?1"))?;
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
            &format!("INSERT INTO providers ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?8,?7,?7)"),
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

/// 一次性把存量明文 API Key 加密回写（已带 `enc:v1:` 前缀的跳过）。
/// 幂等，所以每次启动跑一遍；失败必须冒出去——静默跳过等于"加密存储"只对
/// 之后新存的 key 生效，老数据一直裸奔而没人知道。
pub fn encrypt_plaintext_keys(conn: &Connection) -> AppResult<usize> {
    let mut plain: Vec<(String, String)> = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT id, api_key FROM providers WHERE api_key <> ''")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            plain.push(row?);
        }
    }
    let mut updated = 0;
    for (id, key) in plain {
        if key.starts_with(crate::crypto::PREFIX) {
            continue;
        }
        let enc = crate::crypto::encrypt(&key)?;
        conn.execute(
            "UPDATE providers SET api_key = ?2 WHERE id = ?1",
            params![id, enc],
        )?;
        updated += 1;
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::encrypt_plaintext_keys;
    use rusqlite::params;

    fn insert_provider(conn: &rusqlite::Connection, id: &str, key: &str) {
        conn.execute(
            "INSERT INTO providers (id, name, base_url, protocol, model, custom_headers, api_key, created_at, updated_at)
             VALUES (?1, ?2, 'https://x.test', 'openai_images', '', '{}', ?3, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00')",
            params![id, id, key],
        )
        .unwrap();
    }

    fn key_of(conn: &rusqlite::Connection, id: &str) -> String {
        conn.query_row("SELECT api_key FROM providers WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
    }

    #[test]
    fn plaintext_keys_are_encrypted_in_place() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        insert_provider(&conn, "p-plain", "sk-abcdef");
        let n = encrypt_plaintext_keys(&conn).unwrap();
        assert_eq!(n, 1);
        let stored = key_of(&conn, "p-plain");
        assert!(stored.starts_with("enc:v1:"), "实际存的是：{stored}");
        assert_eq!(crate::crypto::decrypt(&stored).unwrap(), "sk-abcdef");
    }

    #[test]
    fn already_encrypted_keys_are_skipped() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        insert_provider(&conn, "p-enc", "sk-abcdef");
        encrypt_plaintext_keys(&conn).unwrap();
        // 幂等：第二次启动不该再动任何行，否则每次都重写一遍密文
        assert_eq!(encrypt_plaintext_keys(&conn).unwrap(), 0);
    }

    #[test]
    fn key_migration_reports_missing_table() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let err = encrypt_plaintext_keys(&conn).unwrap_err();
        assert!(err.to_string().contains("no such table"), "实际：{err}");
    }

    #[test]
    fn list_items_carry_capabilities() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::init(&conn).unwrap();
        insert_provider(&conn, "p1", "sk-abcdef");
        let items = super::list(&conn).unwrap();
        assert_eq!(items.len(), 1);
        // model 为空 → 走中转兜底行：OpenAI Images 不发 seed、支持蒙版
        assert!(!items[0].capabilities.seed);
        assert!(items[0].capabilities.mask);
        assert_eq!(items[0].capabilities.endpoint, "images");
    }
}
