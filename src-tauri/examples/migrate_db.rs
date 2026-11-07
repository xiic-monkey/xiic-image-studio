//! 对指定的 SQLite 库跑一遍结构迁移（建表 / 补列 / 加外键），不用启动 GUI。
//!
//! 用法：
//!   cargo run --example migrate_db -- "<db 路径>"
//!
//! 外键重建在一个事务里：要么整体生效，要么整体回滚，不会留下半张表。
//! 迁移只加约束、不删数据——指向已删父行的孤儿引用会被置 NULL，
//! 对应的图片仍留在画廊里，只是不再声称自己属于某个任务。

use xiic_image_studio_lib::storage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .cloned()
        .ok_or("用法: cargo run --example migrate_db -- \"<db 路径>\"")?;

    let conn = rusqlite::Connection::open(&path)?;
    storage::init(&conn)?;

    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    println!("表: {tables:?}");
    for t in ["providers", "sessions", "tasks", "images", "prompts"] {
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))?;
        println!("  {t}: {n} 行");
    }
    for (table, column) in [("tasks", "session_id"), ("images", "task_id")] {
        let fk = conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM pragma_foreign_key_list('{table}') WHERE \"from\" = '{column}'"
            ),
            [],
            |r| r.get::<_, i64>(0),
        )?;
        println!("  {table}.{column} 外键约束: {fk} 条");
    }
    Ok(())
}
