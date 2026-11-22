use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

pub struct Db(pub Mutex<Connection>);

impl Db {
    /// 取库连接。锁被毒化（之前有线程持锁时 panic）**不**直接 panic：
    /// 毒化只说明"上一次持锁的线程崩了"，不代表数据坏了，直接 unwrap 会让那一次
    /// panic 传染给之后所有命令，整条链路全部失效。这里取回锁内已有的连接继续用。
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub fn db_path(app_dir: &Path) -> PathBuf {
    app_dir.join("image-studio.db")
}

/// GUI 与无头 CLI 是两个进程写同一张 SQLite。WAL 只解决读写冲突，不解决写写冲突，
/// 所以两边都必须带锁等待——超时设在 init() 里，避免"只有 CLI 设了、GUI 没设"。
pub const BUSY_TIMEOUT_MS: u64 = 10_000;

pub fn init(conn: &Connection) -> AppResult<()> {
    // 不依赖 rusqlite 的 5s 默认值：GUI 与无头进程同时写库，统一按 BUSY_TIMEOUT_MS 排队
    conn.busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS))?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;

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
            session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
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
            task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
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

        CREATE TABLE IF NOT EXISTS presets (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            prompt TEXT NOT NULL,
            size TEXT,
            quality TEXT,
            seed INTEGER,
            provider_id TEXT,
            model_id TEXT,
            created_at TEXT NOT NULL
        );

        -- 供应商 × 模型 多对多。协议归模型（unique 键含 protocol），
        -- 同一模型名可按不同协议各存一条；能力仍由 capabilities 动态算，
        -- 只有用户显式覆写时才落 capabilities_json。
        CREATE TABLE IF NOT EXISTS models (
            id TEXT PRIMARY KEY,
            protocol TEXT NOT NULL,
            model_name TEXT NOT NULL,
            label TEXT NOT NULL DEFAULT '',
            capabilities_json TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            UNIQUE(protocol, model_name)
        );

        CREATE TABLE IF NOT EXISTS provider_models (
            provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
            model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
            is_default INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            PRIMARY KEY (provider_id, model_id)
        );

        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_images_created ON images(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_tasks_created ON tasks(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_images_task ON images(task_id);
        CREATE INDEX IF NOT EXISTS idx_tasks_session ON tasks(session_id);
        CREATE INDEX IF NOT EXISTS idx_provider_models_model ON provider_models(model_id);
        "#,
    )?;
    migrate_legacy_columns(conn)?;
    // presets 是后加的表，老库里没有 model_id 列；新表已在 CREATE TABLE 里带上，
    // add_column_if_absent 对老库补列、对新库 no-op。
    add_column_if_absent(
        conn,
        "presets",
        "model_id",
        "ALTER TABLE presets ADD COLUMN model_id TEXT",
    )?;
    // models.capabilities_json：用户对单个模型的能力覆写（中转站非标准尺寸用）。
    add_column_if_absent(
        conn,
        "models",
        "capabilities_json",
        "ALTER TABLE models ADD COLUMN capabilities_json TEXT NOT NULL DEFAULT ''",
    )?;
    migrate_provider_models(conn)?;
    // 外键迁移属于启动期写库：失败不阻断启动（崩在 setup 里，从 Finder 启动只表现为
    // 「双击没反应」），但必须打 stderr——旧结构会继续跑，只是引用完整性没有兜底。
    if let Err(e) = migrate_foreign_keys(conn) {
        eprintln!("[startup] 外键迁移失败，本次仍按旧结构运行：{e}");
    }
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
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

/// 把旧 providers 行里的 (protocol, model) 拆成 models + provider_models。
/// 用 meta 表的一次性标记做幂等：迁移只跑一次，跑完即使用户主动解绑了全部关联，
/// 下次启动也不会重新挂回（不能用「是否有关联」当标记，那样解绑会被当成未迁移）。
/// INSERT OR IGNORE + 按 UNIQUE(protocol, model_name) 反查，保证 GUI/CLI
/// 两个进程同时首次启动时也不重复建模型行。
fn migrate_provider_models(conn: &Connection) -> AppResult<()> {
    let done: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM meta WHERE key = 'provider_models_migrated'",
            [],
            |r| r.get::<_, i64>(0).map(|n| n > 0),
        )
        .unwrap_or(false);
    if done {
        return Ok(());
    }

    let legacy: Vec<(String, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT p.id, p.protocol, p.model FROM providers p
             WHERE NOT EXISTS (SELECT 1 FROM provider_models pm WHERE pm.provider_id = p.id)
             ORDER BY p.created_at ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        out
    };
    let now = chrono::Utc::now().to_rfc3339();
    for (pid, protocol, model) in legacy {
        // 旧供应商没设模型时仍建一条空名模型，保证迁移后仍能出图（能力走宽松兜底）
        let model_name = if model.trim().is_empty() {
            String::new()
        } else {
            model
        };
        let model_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT OR IGNORE INTO models (id, protocol, model_name, label, created_at)
             VALUES (?1, ?2, ?3, '', ?4)",
            rusqlite::params![model_id, protocol, model_name, now],
        )?;
        let resolved: String = conn.query_row(
            "SELECT id FROM models WHERE protocol = ?1 AND model_name = ?2",
            rusqlite::params![protocol, model_name],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO provider_models (provider_id, model_id, is_default, created_at)
             VALUES (?1, ?2, 1, ?3)",
            rusqlite::params![pid, resolved, now],
        )?;
    }
    // 标记迁移完成。必须在全部成功后写，失败要留到下次重试。
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('provider_models_migrated', ?1)",
        rusqlite::params![now],
    )?;
    Ok(())
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
        // GUI 与 CLI 同时首次启动时可能双双跑到这里，后到的一方报 duplicate
        Err(e) if is_duplicate_column(&e) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// 旧库没有真外键：`PRAGMA foreign_keys = ON` 只在表声明了 REFERENCES 时才起作用，
/// 旧库没声明，这行 pragma 形同虚设——删掉一个任务会留下指向不存在父行的图片。
///
/// SQLite 不能 ALTER TABLE 加约束，只能重建表。这里把 tasks / images 重建到带
/// REFERENCES 的结构，并把指向已删父行的孤儿引用置 NULL：图片仍留在画廊里，
/// 只是不再声称自己属于某个任务。
pub fn migrate_foreign_keys(conn: &Connection) -> AppResult<()> {
    let tasks_has = has_foreign_key(conn, "tasks", "session_id")?;
    let images_has = has_foreign_key(conn, "images", "task_id")?;
    if tasks_has && images_has {
        return Ok(());
    }
    // 迁移期间关掉外键检查：重建要 DROP 父表，子表里指向它的旧引用是合法的中间态。
    // PRAGMA 在事务内是空操作，所以切换必须在事务外。
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let result = (|| -> AppResult<()> {
        conn.execute_batch("BEGIN;")?;
        match rebuild_with_foreign_keys(conn, tasks_has, images_has) {
            Ok(()) => {
                conn.execute_batch("COMMIT;")?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                Err(e)
            }
        }
    })();
    // 成功失败都要把外键检查恢复回去，否则这条连接之后写库不再受约束
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    result?;

    let violations = foreign_key_violations(conn)?;
    if !violations.is_empty() {
        return Err(AppError::Db(format!(
            "外键迁移后仍存在 {violations:?} 上的引用违规"
        )));
    }
    Ok(())
}

fn rebuild_with_foreign_keys(
    conn: &Connection,
    tasks_has: bool,
    images_has: bool,
) -> AppResult<()> {
    // 顺序要紧：tasks 先重建，images 的外键指向的父表才存在
    if !tasks_has {
        rebuild_tasks(conn)?;
    }
    if !images_has {
        rebuild_images(conn)?;
    }
    Ok(())
}

/// tasks 重建：session_id 变成 REFERENCES sessions(id) ON DELETE SET NULL。
/// 删会话时任务的会话归属被清空，任务本身留在队列里（历史不该因为删会话消失）。
fn rebuild_tasks(conn: &Connection) -> AppResult<()> {
    // 孤儿 session_id 会让新约束立刻违反，先置 NULL（session_id 本来就可空）
    conn.execute(
        "UPDATE tasks SET session_id = NULL
         WHERE session_id IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM sessions s WHERE s.id = tasks.session_id)",
        [],
    )?;
    conn.execute_batch("DROP TABLE IF EXISTS tasks_new;")?;
    conn.execute_batch(
        r#"CREATE TABLE tasks_new (
            id TEXT PRIMARY KEY,
            session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
            provider_id TEXT NOT NULL,
            protocol TEXT NOT NULL,
            request_json TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            error TEXT,
            concurrency INTEGER NOT NULL DEFAULT 1,
            total INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            finished_at TEXT
        );"#,
    )?;
    conn.execute(
        "INSERT INTO tasks_new
           (id, session_id, provider_id, protocol, request_json, status, error,
            concurrency, total, created_at, finished_at)
         SELECT id, session_id, provider_id, protocol, request_json, status, error,
                concurrency, total, created_at, finished_at
         FROM tasks",
        [],
    )?;
    conn.execute_batch("DROP TABLE tasks;")?;
    conn.execute_batch("ALTER TABLE tasks_new RENAME TO tasks;")?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_tasks_created ON tasks(created_at DESC);
         CREATE INDEX IF NOT EXISTS idx_tasks_session ON tasks(session_id);",
    )?;
    Ok(())
}

/// images 重建：task_id 变成可空的 REFERENCES tasks(id) ON DELETE SET NULL。
/// 删任务不删图（画廊里的图不该因为清任务队列就没了），只把归属清空。
fn rebuild_images(conn: &Connection) -> AppResult<()> {
    conn.execute_batch("DROP TABLE IF EXISTS images_new;")?;
    conn.execute_batch(
        r#"CREATE TABLE images_new (
            id TEXT PRIMARY KEY,
            task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
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
        );"#,
    )?;
    conn.execute(
        "INSERT INTO images_new
           (id, task_id, path, thumb_path, prompt, model, params_json,
            favorite, width, height, size, created_at)
         SELECT id, task_id, path, thumb_path, prompt, model, params_json,
                favorite, width, height, size, created_at
         FROM images",
        [],
    )?;
    conn.execute_batch("DROP TABLE images;")?;
    conn.execute_batch("ALTER TABLE images_new RENAME TO images;")?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_images_created ON images(created_at DESC);
         CREATE INDEX IF NOT EXISTS idx_images_task ON images(task_id);",
    )?;
    // 孤儿 task_id（父任务已被删）置 NULL：图片留在画廊，但不再指向不存在的任务
    conn.execute(
        "UPDATE images SET task_id = NULL
         WHERE task_id IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.id = images.task_id)",
        [],
    )?;
    Ok(())
}

/// 该表是否已对指定列声明了外键。PRAGMA foreign_key_list 的第 4 列是「从表列名」。
fn has_foreign_key(conn: &Connection, table: &str, column: &str) -> AppResult<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA foreign_key_list({table})"))?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(3))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(names.iter().any(|n| n == column))
}

/// 仍有引用违规的表名列表（正常迁移后应为空）
fn foreign_key_violations(conn: &Connection) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tables)
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
        add_column_if_absent, column_exists, foreign_key_violations, has_foreign_key, init,
        is_duplicate_column, migrate_foreign_keys, migrate_legacy_columns, migrate_provider_models,
        Db, BUSY_TIMEOUT_MS,
    };
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    fn busy_timeout_ms(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA busy_timeout", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn init_sets_busy_timeout_for_every_opener() {
        // GUI 与 CLI 都只调 init：锁等待必须在 init 里，否则先启动的一方撞写锁会立刻 SQLITE_BUSY
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
        conn.execute_batch("CREATE TABLE t (a TEXT NOT NULL)")
            .unwrap();
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

    #[test]
    fn fresh_db_declares_real_foreign_keys() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        assert!(
            has_foreign_key(&conn, "tasks", "session_id").unwrap(),
            "tasks.session_id 必须真的声明 REFERENCES"
        );
        assert!(
            has_foreign_key(&conn, "images", "task_id").unwrap(),
            "images.task_id 必须真的声明 REFERENCES"
        );
    }

    #[test]
    fn foreign_key_rejects_image_without_existing_task() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        let err = conn
            .execute(
                "INSERT INTO images (id, task_id, path, created_at) VALUES ('i1', 'nope', '/tmp/x.png', 'now')",
                [],
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("FOREIGN KEY"),
            "外键必须真的拦得住，实际错误：{err}"
        );
    }

    #[test]
    fn deleting_task_keeps_image_but_clears_its_task_id() {
        // task_delete 的语义：删任务不删画廊里的图，只把归属清空（ON DELETE SET NULL）
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        conn.execute(
            "INSERT INTO tasks (id, provider_id, protocol, request_json, created_at)
             VALUES ('t1','p','openai_images','{}','now')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO images (id, task_id, path, created_at) VALUES ('i1','t1','/tmp/x.png','now')",
            [],
        )
        .unwrap();
        conn.execute("DELETE FROM tasks WHERE id = 't1'", [])
            .unwrap();

        let task_id: Option<String> = conn
            .query_row("SELECT task_id FROM images WHERE id = 'i1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(task_id, None, "图片要留在画廊里，但不能再指向已删的任务");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "图片行不能被级联删掉");
    }

    #[test]
    fn images_have_index_on_task_id() {
        // task_list 按 task_id 回填图片、count_done 按 task_id 计数，没索引就是全表扫
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        let mut stmt = conn.prepare("PRAGMA index_list(images)").unwrap();
        let names: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(
            names.iter().any(|n| n == "idx_images_task"),
            "images.task_id 缺索引，实际索引：{names:?}"
        );
    }

    /// 旧库结构：没有 REFERENCES，images.task_id 是 NOT NULL
    fn legacy_schema(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, name TEXT NOT NULL, provider_id TEXT,
                draft_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
             CREATE TABLE tasks (id TEXT PRIMARY KEY, session_id TEXT, provider_id TEXT NOT NULL,
                protocol TEXT NOT NULL, request_json TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending',
                error TEXT, concurrency INTEGER NOT NULL DEFAULT 1, total INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL, finished_at TEXT);
             CREATE TABLE images (id TEXT PRIMARY KEY, task_id TEXT NOT NULL, path TEXT NOT NULL,
                thumb_path TEXT, prompt TEXT NOT NULL DEFAULT '', model TEXT NOT NULL DEFAULT '',
                params_json TEXT NOT NULL DEFAULT '{}', favorite INTEGER NOT NULL DEFAULT 0,
                width INTEGER, height INTEGER, size INTEGER, created_at TEXT NOT NULL);",
        )
        .unwrap();
    }

    #[test]
    fn legacy_db_gains_foreign_keys_and_orphan_refs_are_cleared() {
        let conn = Connection::open_in_memory().unwrap();
        legacy_schema(&conn);
        conn.execute(
            "INSERT INTO tasks (id, session_id, provider_id, protocol, request_json, created_at)
             VALUES ('t1','gone-session','p','openai_images','{}','now')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (id, provider_id, protocol, request_json, created_at)
             VALUES ('t2','p','openai_images','{}','now')",
            [],
        )
        .unwrap();
        for (id, task_id) in [("i1", "t1"), ("i2", "t2"), ("i3", "dead-task")] {
            conn.execute(
                "INSERT INTO images (id, task_id, path, created_at) VALUES (?1, ?2, '/tmp/x.png', 'now')",
                rusqlite::params![id, task_id],
            )
            .unwrap();
        }

        migrate_foreign_keys(&conn).unwrap();

        assert!(has_foreign_key(&conn, "tasks", "session_id").unwrap());
        assert!(has_foreign_key(&conn, "images", "task_id").unwrap());
        assert!(
            foreign_key_violations(&conn).unwrap().is_empty(),
            "迁移后不该还有引用违规"
        );

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3, "迁移不能丢数据");
        let task_id_of = |id: &str| -> Option<String> {
            conn.query_row("SELECT task_id FROM images WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
        };
        assert_eq!(task_id_of("i1").as_deref(), Some("t1"), "合法引用要保留");
        assert_eq!(task_id_of("i2").as_deref(), Some("t2"), "合法引用要保留");
        assert_eq!(task_id_of("i3"), None, "指向已删任务的孤儿引用要清空");
        let session_id: Option<String> = conn
            .query_row("SELECT session_id FROM tasks WHERE id = 't1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(session_id, None, "指向已删会话的孤儿引用要清空");
    }

    #[test]
    fn foreign_key_migration_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        legacy_schema(&conn);
        migrate_foreign_keys(&conn).unwrap();
        migrate_foreign_keys(&conn).unwrap();
        assert!(has_foreign_key(&conn, "images", "task_id").unwrap());
    }

    fn insert_legacy_provider(conn: &Connection, id: &str, protocol: &str, model: &str) {
        conn.execute(
            "INSERT INTO providers (id, name, base_url, protocol, model, custom_headers, api_key, created_at, updated_at)
             VALUES (?1, ?2, 'https://x.test', ?3, ?4, '{}', '', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00')",
            rusqlite::params![id, id, protocol, model],
        )
        .unwrap();
    }

    /// 模拟老库升级：init() 在空库上已写过迁移标记，这里清掉它让迁移真正跑一遍。
    fn reset_migration_marker(conn: &Connection) {
        conn.execute_batch("DELETE FROM meta WHERE key = 'provider_models_migrated'")
            .unwrap();
    }

    #[test]
    fn provider_model_migration_splits_legacy_rows() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        insert_legacy_provider(&conn, "p1", "openai_images", "gpt-image-1");
        insert_legacy_provider(&conn, "p2", "gemini_native", "nano-banana");
        reset_migration_marker(&conn);
        migrate_provider_models(&conn).unwrap();

        let models: i64 = conn
            .query_row("SELECT COUNT(*) FROM models", [], |r| r.get(0))
            .unwrap();
        let links: i64 = conn
            .query_row("SELECT COUNT(*) FROM provider_models", [], |r| r.get(0))
            .unwrap();
        assert_eq!(models, 2, "两条不同的 (protocol, model) 应建两条模型");
        assert_eq!(links, 2, "每个旧供应商各挂一条默认模型");
        let default: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM provider_models WHERE is_default = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(default, 2);
    }

    #[test]
    fn provider_model_migration_reuses_same_model_across_providers() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        // 两家供应商用同一个 (protocol, model) —— 应复用同一条 model，挂两条关联（多对多）
        insert_legacy_provider(&conn, "p1", "openai_images", "gpt-image-1");
        insert_legacy_provider(&conn, "p2", "openai_images", "gpt-image-1");
        reset_migration_marker(&conn);
        migrate_provider_models(&conn).unwrap();

        let models: i64 = conn
            .query_row("SELECT COUNT(*) FROM models", [], |r| r.get(0))
            .unwrap();
        let links: i64 = conn
            .query_row("SELECT COUNT(*) FROM provider_models", [], |r| r.get(0))
            .unwrap();
        assert_eq!(models, 1, "同一 (protocol, model) 只建一条模型");
        assert_eq!(links, 2, "两个供应商都关联到这条模型");
    }

    #[test]
    fn provider_model_migration_handles_empty_model_and_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        insert_legacy_provider(&conn, "p1", "openai_images", "");
        reset_migration_marker(&conn);
        migrate_provider_models(&conn).unwrap();
        let empty_name: String = conn
            .query_row("SELECT model_name FROM models", [], |r| r.get(0))
            .unwrap();
        assert_eq!(empty_name, "", "空模型名建兜底行");

        // 重复迁移不再新增（供应商已有链接）
        migrate_provider_models(&conn).unwrap();
        let models: i64 = conn
            .query_row("SELECT COUNT(*) FROM models", [], |r| r.get(0))
            .unwrap();
        assert_eq!(models, 1);
    }

    #[test]
    fn provider_model_migration_keeps_manually_unlinked_providers() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        insert_legacy_provider(&conn, "p1", "openai_images", "gpt-image-1");
        reset_migration_marker(&conn);
        migrate_provider_models(&conn).unwrap();
        // 用户主动解绑后再次迁移，不应把模型挂回来
        conn.execute_batch("DELETE FROM provider_models").unwrap();
        migrate_provider_models(&conn).unwrap();
        let links: i64 = conn
            .query_row("SELECT COUNT(*) FROM provider_models", [], |r| r.get(0))
            .unwrap();
        assert_eq!(links, 0, "已有过关联的供应商不该被重新挂模型");
    }

    #[test]
    fn conn_still_works_after_the_mutex_is_poisoned() {
        // 之前有线程持锁 panic 时锁会中毒；老实现 unwrap 会让之后所有命令连锁 panic
        let db = Arc::new(Db(Mutex::new(Connection::open_in_memory().unwrap())));
        let crashed = db.clone();
        let _ = std::thread::spawn(move || {
            let _guard = crashed.0.lock().unwrap();
            panic!("boom");
        })
        .join();

        let conn = db.conn();
        conn.execute_batch("SELECT 1").unwrap();
    }
}
