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
    migrate_legacy_columns(conn)?;
    Ok(())
}

/// 旧库补列。列已存在（duplicate column）是正常路径，跳过；
/// 其他错误（表不存在 / 库损坏 / busy）必须冒出来——否则老库静默缺列，
/// 要等到第一次 INSERT 才炸，那时已经看不出是迁移失败。
fn migrate_legacy_columns(conn: &Connection) -> AppResult<()> {
    add_column_if_absent(
        conn,
        "providers",
        "api_key",
        "ALTER TABLE providers ADD COLUMN api_key TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_absent(
        conn,
        "tasks",
        "total",
        "ALTER TABLE tasks ADD COLUMN total INTEGER NOT NULL DEFAULT 1",
    )
}

/// `table` / `column` / `ddl` 全部来自本模块写死的 `&'static str`，不接受任何外部输入，
/// 因此 `format!` 拼 PRAGMA 不存在注入面。
fn add_column_if_absent(
    conn: &Connection,
    table: &'static str,
    column: &'static str,
    ddl: &'static str,
) -> AppResult<()> {
    if column_exists(conn, table, column)? {
        return Ok(());
    }
    match conn.execute_batch(ddl) {
        Ok(()) => Ok(()),
        // GUI 与 MCP 同时首次启动时可能双双跑到这里，后到的一方报 duplicate
        Err(e) if is_duplicate_column(&e) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> AppResult<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(names.iter().any(|n| n == column))
}

fn is_duplicate_column(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(_, Some(msg)) if msg.contains("duplicate column name"))
}

#[cfg(test)]
mod tests {
    use super::{
        add_column_if_absent, column_exists, init, is_duplicate_column, migrate_legacy_columns,
        BUSY_TIMEOUT_MS,
    };
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

    #[test]
    fn legacy_migration_skips_existing_column() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        // init 建表时已带这两列，再跑一次迁移必须是 no-op（旧实现靠 duplicate column 报错吞掉）
        migrate_legacy_columns(&conn).unwrap();
        assert!(column_exists(&conn, "providers", "api_key").unwrap());
        assert!(column_exists(&conn, "tasks", "total").unwrap());
    }

    #[test]
    fn legacy_migration_on_missing_table_is_reported() {
        let conn = Connection::open_in_memory().unwrap();
        let err = add_column_if_absent(
            &conn,
            "tasks",
            "total",
            "ALTER TABLE tasks ADD COLUMN total INTEGER NOT NULL DEFAULT 1",
        )
        .unwrap_err();
        assert!(err.to_string().contains("no such table"), "实际错误：{err}");
        assert!(!err.is_retryable(), "库结构问题重试无意义");
    }

    #[test]
    fn duplicate_column_error_is_recognised() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (a TEXT NOT NULL)").unwrap();
        let dup = conn
            .execute_batch("ALTER TABLE t ADD COLUMN a TEXT NOT NULL DEFAULT ''")
            .unwrap_err();
        assert!(is_duplicate_column(&dup), "实际错误：{dup}");
        // busy 必须走"冒出去"那一支，不能被当成"列已存在"而静默略过
        let busy = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ffi::ErrorCode::DatabaseBusy,
                extended_code: 5,
            },
            Some("database is locked".into()),
        );
        assert!(!is_duplicate_column(&busy));
    }
}
