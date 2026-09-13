use crate::error::AppResult;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Db(pub Mutex<Connection>);

pub fn db_path(app_dir: &PathBuf) -> PathBuf {
    app_dir.join("image-studio.db")
}

pub fn init(conn: &Connection) -> AppResult<()> {
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
