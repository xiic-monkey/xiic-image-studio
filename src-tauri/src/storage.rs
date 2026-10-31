use crate::error::AppResult;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Db(pub Mutex<Connection>);

pub fn db_path(app_dir: &PathBuf) -> PathBuf {
    app_dir.join("image-studio.db")
}

/// GUI 与无头 MCP 是两个进程写同一张 SQLite。WAL 只解决读写冲突，不解决写写冲突，
/// 所以两边都必须带锁等待——超时设在 init() 里，避免"只有 MCP 设了、GUI 没设"。
pub const BUSY_TIMEOUT_MS: u64 = 10_000;

pub fn init(conn: &Connection) -> AppResult<()> {
    // 不依赖 rusqlite 的 5s 默认值：GUI 与无头进程同时写库，统一按 BUSY_TIMEOUT_MS 排队
    conn.busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS))?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS providers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            base_url TEXT NOT NULL,
            protocol TEXT NOT NULL,
            model TEXT NOT NULL DEFAULT '',
            custom_headers TEXT NOT NULL DEFAULT '{}',
            api_key TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            provider_id TEXT,
            draft_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            session_id TEXT,
            provider_id TEXT NOT NULL,
            protocol TEXT NOT NULL,
            request_json TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            error TEXT,
            concurrency INTEGER NOT NULL DEFAULT 1,
            total INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            finished_at TEXT
        );

        CREATE TABLE IF NOT EXISTS images (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            path TEXT NOT NULL,
            thumb_path TEXT,
            prompt TEXT NOT NULL DEFAULT '',
            model TEXT NOT NULL DEFAULT '',
            params_json TEXT NOT NULL DEFAULT '{}',
            favorite INTEGER NOT NULL DEFAULT 0,
            width INTEGER,
            height INTEGER,
            size INTEGER,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS prompts (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_images_created ON images(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_tasks_created ON tasks(created_at DESC);
        "#,
    )?;
    // 旧库迁移：providers 补 api_key 列（已存在则忽略）
    let _ = conn.execute_batch(
        "ALTER TABLE providers ADD COLUMN api_key TEXT NOT NULL DEFAULT '';",
    );
    // 旧库迁移：tasks 补 total 列
    let _ = conn.execute_batch("ALTER TABLE tasks ADD COLUMN total INTEGER NOT NULL DEFAULT 1;");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{init, BUSY_TIMEOUT_MS};
    use rusqlite::Connection;

    fn busy_timeout_ms(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA busy_timeout", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn init_sets_busy_timeout_for_every_opener() {
        // GUI 与 MCP 都只调 init：锁等待必须在 init 里，否则先启动的一方撞写锁会立刻 SQLITE_BUSY
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        assert_eq!(busy_timeout_ms(&conn), BUSY_TIMEOUT_MS as i64);
    }

    #[test]
    fn init_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        init(&conn).unwrap();
    }
}
