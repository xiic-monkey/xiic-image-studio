//! 用 app 自己的生成链路出一张图（给自己生成 app 图标用）。
//!
//! 它走的是和 UI 完全相同的路径：读 providers 表里的配置 → 解密 API key
//! → 构造 GenCtx → 调 openai_images 适配器。所以这个 example 同时也是
//! 一条「不依赖 GUI 的端到端链路自检」。
//!
//! 用法：
//!   cargo run --example gen_icon -- "<prompt>" [out.png] [size]
//!
//! 默认输出 /tmp/app-icon-source.png，尺寸 1024x1024。

use xiic_image_studio_lib::generate::adapters::openai_images;
use xiic_image_studio_lib::generate::types::GenCtx;
use xiic_image_studio_lib::provider::db;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let prompt = args
        .get(1)
        .cloned()
        .ok_or("用法: cargo run --example gen_icon -- \"<prompt>\" [out.png] [size]")?;
    let out = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "/tmp/app-icon-source.png".to_string());
    let size = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| "1024x1024".to_string());

    let home = std::env::var("HOME")?;
    let db_path =
        format!("{home}/Library/Application Support/com.xiic.image-studio/image-studio.db");
    let conn = rusqlite::Connection::open(&db_path)?;

    let (id, name, base_url, model): (String, String, String, String) = conn.query_row(
        "SELECT id, name, base_url, model FROM providers ORDER BY created_at ASC LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    let key = db::get_key(&conn, &id)?.ok_or("该 provider 没有可用的 API key")?;

    println!("provider: {name} | model: {model} | base: {base_url}");
    println!("size: {size} | out: {out}");

    let ctx = GenCtx {
        base_url,
        key,
        model,
        custom_headers: Default::default(),
        prompt: prompt.clone(),
        size: Some(size),
        quality: None,
        seed: None,
        refs: Vec::new(),
        mask: None,
        cancel: None,
    };

    let started = std::time::Instant::now();
    let img = openai_images(&ctx).await?;
    println!(
        "出图成功: {} bytes ({}) 用时 {:.1}s",
        img.bytes.len(),
        img.mime,
        started.elapsed().as_secs_f32()
    );

    std::fs::write(&out, &img.bytes)?;
    println!("已写入 {out}");
    Ok(())
}
