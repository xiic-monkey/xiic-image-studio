//! 无头 CLI 模式：`xiic-image-studio generate` / `xiic-image-studio providers`
//!
//! # 能力归属：仍然是 studio 自己
//! 这里不复制任何"底层能力"——不重新实现适配器、不自己解析 provider、
//! 不走另一套落库逻辑。它只是把 app 自己的生成链路（同一个 SQLite、
//! 同一套 adapters、同一套任务/图片落库规则）在**没有 GUI** 的情况下跑一遍。
//! 所以 CLI 产出的图，和工作室里手动点「生成」的产物完全一致：
//! 同一张 tasks/images 表、同一个 images 目录，工作台打开就看得见。
//!
//! # 与 GUI 并发
//! CLI 是独立进程，与 GUI 写同一张 SQLite。两边都走 `storage::init()`，
//! 锁等待（busy_timeout）统一在那里设置，不会撞 `SQLITE_BUSY`。
//!
//! # 输出契约
//! stdout 只出结果（纯文本或 `--json`），错误一律走 stderr + 非零退出码，
//! 便于脚本与管道消费。无常驻进程、不占端口、不 emit 进度。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use rusqlite::Connection;
use serde_json::{json, Value};
use tokio::runtime::Runtime;

use crate::error::{AppError, AppResult};
use crate::generate::pipeline;
use crate::generate::types::{
    build_request_json, clamp_n, GenerateRequest, ImageProducer, RefImage, TaskOutcome, MAX_N,
};
use crate::provider::db as provider_db;
use crate::storage::{self, Db};

/// 单次任务最多几张参考图。与前端 `src/lib/ref-image.ts` 的 `MAX_REFS` 保持一致。
pub const MAX_REFS: usize = 4;

/// 单张参考图的字节上限（16MB）。和前端读本地文件的上限同量级。
const MAX_REF_BYTES: u64 = 16 * 1024 * 1024;

/// 无 Tauri 运行时时的应用数据目录（与 `app.path().app_data_dir()` 一致）。
fn app_data_dir() -> AppResult<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| AppError::General("无法定位应用数据目录".into()))?;
    Ok(base.join("com.xiic.image-studio"))
}

/// 打开同一个库并完成建表/迁移。CLI 与 GUI 写的是同一个文件。
fn open_db() -> Result<(PathBuf, Db)> {
    let dir = app_data_dir()?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("无法创建应用数据目录：{}", dir.display()))?;
    let conn = Connection::open(storage::db_path(&dir))
        .with_context(|| format!("无法打开数据库：{}", dir.display()))?;
    // 锁等待（busy_timeout）由 storage::init 统一设置，GUI 侧走同一条路
    storage::init(&conn)?;
    Ok((dir, Db(Mutex::new(conn))))
}

// ---------------------------------------------------------------- 参数解析

#[derive(Debug, Default, PartialEq)]
struct GenerateArgs {
    prompt: String,
    refs: Vec<String>,
    provider: Option<String>,
    n: Option<u32>,
    size: Option<String>,
    quality: Option<String>,
    seed: Option<u64>,
    json: bool,
}

/// 取参数值：优先 `--flag=value` 的内联形式，否则吃下一个位置参数。
fn value_for(args: &[String], i: &mut usize, inline: Option<String>, name: &str) -> Result<String> {
    if let Some(v) = inline {
        return Ok(v);
    }
    *i += 1;
    args.get(*i)
        .cloned()
        .ok_or_else(|| anyhow!("参数 {name} 缺少取值"))
}

/// 把 `--flag=value` 拆成 (name, Some(value))；其余原样返回 (name, None)。
fn split_inline(arg: &str) -> (&str, Option<String>) {
    if let Some((name, value)) = arg.split_once('=') {
        if name.starts_with("--") {
            return (name, Some(value.to_string()));
        }
    }
    (arg, None)
}

fn parse_generate(args: &[String]) -> Result<GenerateArgs> {
    let mut out = GenerateArgs::default();
    let mut i = 0;
    while i < args.len() {
        let (name, inline) = split_inline(args[i].as_str());
        match name {
            "--prompt" => out.prompt = value_for(args, &mut i, inline, "--prompt")?,
            "--ref" => out.refs.push(value_for(args, &mut i, inline, "--ref")?),
            "--provider" => out.provider = Some(value_for(args, &mut i, inline, "--provider")?),
            "-n" | "--n" | "--count" => {
                let v = value_for(args, &mut i, inline, "-n")?;
                out.n = Some(
                    v.parse::<u32>()
                        .with_context(|| format!("-n 需要正整数，收到 '{v}'"))?,
                );
            }
            "--size" => out.size = Some(value_for(args, &mut i, inline, "--size")?),
            "--quality" => out.quality = Some(value_for(args, &mut i, inline, "--quality")?),
            "--seed" => {
                let v = value_for(args, &mut i, inline, "--seed")?;
                out.seed = Some(
                    v.parse::<u64>()
                        .with_context(|| format!("--seed 需要整数，收到 '{v}'"))?,
                );
            }
            "--json" => out.json = true,
            other => bail!("未知参数 {other}（用 --help 查看用法）"),
        }
        i += 1;
    }
    Ok(out)
}

// ---------------------------------------------------------------- 参考图

/// 按扩展名推 mime；不支持的格式返回 None（调用方据此报错，不静默跳过）。
fn mime_for_path(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

/// 读一张本地参考图，转成适配器认的 `RefImage`。
///
/// 与 `generate::read_file_b64` 不同：那个只认 images_dir 内的相对路径（画廊回读用），
/// 这里要读用户显式指定的任意本地图，所以不做目录越界校验，改用
/// 扩展名白名单 + 大小上限兜底。
fn ref_from_path(path: &str) -> Result<RefImage> {
    let p = PathBuf::from(path);
    let meta = std::fs::metadata(&p)
        .with_context(|| format!("无法读取参考图（不存在或不可读）：{path}"))?;
    if !meta.is_file() {
        bail!("参考图不是普通文件：{path}");
    }
    if meta.len() > MAX_REF_BYTES {
        bail!("参考图超过 {}MB 上限：{path}", MAX_REF_BYTES / 1024 / 1024);
    }
    let mime = mime_for_path(&p)
        .ok_or_else(|| anyhow!("参考图格式不支持：{path}（支持 png / jpg / jpeg / webp / gif）"))?;
    let bytes = std::fs::read(&p).with_context(|| format!("无法读取参考图：{path}"))?;
    // RefImage.data 是不带 data: 前缀的纯 base64
    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let name = p
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    Ok(RefImage {
        name,
        mime: mime.to_string(),
        data,
    })
}

/// 参考图能力守卫：`capabilities::sanitize` 只裁 size/quality/seed/mask，**不裁 refs**。
/// 给 `refs=false` 的协议带图会打到上游不存在的端点并被 400，所以这里必须自己拦。
fn check_refs_supported(
    caps: &crate::capabilities::Capabilities,
    refs: &[RefImage],
    provider_name: &str,
    model: &str,
) -> Result<()> {
    if caps.refs || refs.is_empty() {
        return Ok(());
    }
    bail!(
        "provider '{provider_name}' 的模型 '{model}' 不支持参考图（该协议走 {} 端点，带图会被上游拒绝）。\
         请换一个支持参考图的 provider，或去掉 --ref。",
        caps.endpoint
    );
}

// ---------------------------------------------------------------- 输出组装

/// 组装 `--json` 的结果结构。与旧 MCP 的 result 同构，去掉了 data_url（CLI 走路径）。
fn build_result_json(outcome: &TaskOutcome) -> Value {
    let images: Vec<Value> = outcome
        .images
        .iter()
        .map(|img| {
            json!({
                "id": img.id,
                "path": img.path,
                "thumb_path": img.thumb_path,
            })
        })
        .collect();
    let mut result = json!({
        "task_id": outcome.task_id,
        "status": outcome.status,
        "count": images.len(),
        "images": images,
    });
    if let Some(err) = outcome.errors.first() {
        result["error"] = json!(err);
    }
    if let Some(e) = &outcome.db_error {
        result["warning"] = json!(format!("图片已入库，但任务状态写入失败：{e}"));
    }
    result
}

/// 人类可读输出：每图一行绝对路径 + task_id。
fn print_result_text(outcome: &TaskOutcome) {
    println!(
        "已生成 {} 张图片，已写入 image-studio 的任务与图库（打开工作室即可看到，task_id: {}）",
        outcome.images.len(),
        outcome.task_id
    );
    for img in &outcome.images {
        println!("- {}", img.path);
    }
    if let Some(err) = outcome.errors.first() {
        println!("注意：{err}");
    }
    if let Some(e) = &outcome.db_error {
        println!("注意：图片已入库，但任务状态写入失败：{e}");
    }
}

// ---------------------------------------------------------------- 子命令

/// 生成：与 GUI 共用 prepare_task + run_task_headless，落库、落盘、收尾全在编排层。
fn generate(args: &[String]) -> Result<()> {
    let parsed = parse_generate(args)?;
    let prompt = parsed.prompt.trim().to_string();
    if prompt.is_empty() {
        bail!("prompt 不能为空");
    }
    if parsed.refs.len() > MAX_REFS {
        bail!("参考图最多 {MAX_REFS} 张，收到 {} 张", parsed.refs.len());
    }
    let n = clamp_n(parsed.n.unwrap_or(1), MAX_N);

    let (dir, db) = open_db()?;
    let images_dir = dir.join("images");
    std::fs::create_dir_all(&images_dir)?;

    // 参考图先读：文件有问题要早失败，别先建了任务再失败
    let mut refs = Vec::with_capacity(parsed.refs.len());
    for path in &parsed.refs {
        refs.push(ref_from_path(path)?);
    }

    // 选 provider：名称或 id 都接受，不给就用第一个配好 key 的
    let (provider, key) = {
        let conn = db.conn();
        let items = provider_db::list(&conn)?;
        if items.is_empty() {
            bail!("还没有配置任何 provider，请先在 image-studio 应用里添加一个");
        }
        let chosen = match parsed
            .provider
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty())
        {
            Some(hint) => items
                .iter()
                .find(|p| p.id == hint || p.name.eq_ignore_ascii_case(hint))
                .ok_or_else(|| {
                    anyhow!(
                        "找不到 provider '{hint}'（可用：{}）",
                        items
                            .iter()
                            .map(|p| p.name.as_str())
                            .collect::<Vec<_>>()
                            .join("、")
                    )
                })?,
            None => items.iter().find(|p| p.has_key).ok_or_else(|| {
                anyhow!("所有 provider 都还没填 API Key，请先在 image-studio 应用里配置")
            })?,
        };
        if !chosen.has_key {
            bail!("provider '{}' 还没填 API Key", chosen.name);
        }
        let (provider, key) = provider_db::get_with_key(&conn, &chosen.id)?
            .ok_or_else(|| anyhow!("provider '{}' 不存在或没有可用的 API Key", chosen.name))?;
        (provider, key)
    };

    // 与 GUI 同一个裁剪入口；refs 由上面的守卫单独把关
    let caps = crate::capabilities::for_model(provider.protocol, &provider.model);
    check_refs_supported(&caps, &refs, &provider.name, &provider.model)?;

    let mut req = GenerateRequest {
        provider_id: provider.id.clone(),
        prompt: prompt.clone(),
        n,
        size: parsed.size.clone(),
        quality: parsed.quality.clone(),
        seed: parsed.seed,
        refs,
        mask: None,
        session_id: None,
    };
    crate::capabilities::sanitize(&caps, &mut req);

    let ctx = pipeline::build_ctx(&provider, key, &req, &prompt);
    let request_json = build_request_json(&req, n);
    let task = {
        let conn = db.conn();
        let spec = pipeline::TaskSpec {
            session_id: None,
            provider: &provider,
            n,
            request_json: &request_json,
            producer: ImageProducer::Protocol(provider.protocol),
            ctx,
            prompt,
            model: provider.model.clone(),
        };
        pipeline::prepare_task(&conn, spec)?
    };
    let runtime = Runtime::new().context("无法创建 tokio runtime")?;
    let outcome = pipeline::run_task_headless(&db, &images_dir, &runtime, &task);

    if outcome.images.is_empty() {
        let reason = outcome
            .db_error
            .clone()
            .or_else(|| outcome.errors.first().cloned())
            .unwrap_or_else(|| "生成失败".into());
        bail!("{reason}");
    }

    if parsed.json {
        println!("{}", build_result_json(&outcome));
    } else {
        print_result_text(&outcome);
    }
    Ok(())
}

/// 列出工作室里已配置的 provider。
fn providers(args: &[String]) -> Result<()> {
    let json_out = args.iter().any(|a| a == "--json");
    for arg in args {
        if arg != "--json" {
            bail!("未知参数 {arg}（providers 目前只支持 --json）");
        }
    }
    let (_dir, db) = open_db()?;
    let items = {
        let conn = db.conn();
        provider_db::list(&conn)?
    };
    if json_out {
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }
    if items.is_empty() {
        println!("还没有配置任何 provider（请先在 image-studio 应用里添加一个）");
        return Ok(());
    }
    println!("可用 provider：");
    for p in &items {
        println!(
            "- {} [{}] model={} key={}",
            p.name,
            p.protocol.as_str(),
            if p.model.is_empty() {
                "默认"
            } else {
                &p.model
            },
            if p.has_key { "已配置" } else { "未配置" }
        );
    }
    Ok(())
}

fn print_help() {
    println!(
        "xiic-image-studio —— 本地 AI 图片工作室

用法：
  xiic-image-studio generate --prompt <提示词> [选项]
  xiic-image-studio providers [--json]
  xiic-image-studio --help | --version
  xiic-image-studio              （不带参数 = 启动图形界面）

generate 选项：
  --prompt <文本>        必填，画面描述
  --ref <图片路径>       参考图，可重复（最多 {MAX_REFS} 张，用于以图生图）
  --provider <名称|id>   不填则用第一个已配好 API Key 的
  -n <1..{MAX_N}>             生成张数，默认 1
  --size <1024x1024>     分辨率（按模型能力裁剪）
  --quality <high>       质量档（按模型能力裁剪）
  --seed <整数>          随机种子（部分模型不支持）
  --json                 以 JSON 输出，便于脚本消费

产物会写入工作室的同一个数据库与图库，打开图形界面即可看到。"
    );
}

/// CLI 入口。有子命令时执行它并返回；错误由调用方打印到 stderr 并以非零码退出。
pub fn run(args: Vec<String>) -> Result<()> {
    let Some(first) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };
    match first {
        "--help" | "-h" | "help" => {
            print_help();
            Ok(())
        }
        "--version" | "-V" => {
            println!("xiic-image-studio {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "--mcp" => bail!(
            "MCP 模式已移除，请改用 CLI 子命令：`xiic-image-studio generate --prompt \"...\"`"
        ),
        "generate" => generate(&args[1..]),
        "providers" => providers(&args[1..]),
        other => bail!("未知子命令 '{other}'（可用：generate / providers / --help）"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::types::ImageMeta;
    use crate::provider::types::Protocol;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_all_options() {
        let a = parse_generate(&args(&[
            "--prompt",
            "  一只猫  ",
            "--ref",
            "/tmp/a.png",
            "--ref",
            "/tmp/b.png",
            "--provider",
            "Hubway",
            "-n",
            "3",
            "--size",
            "1024x1024",
            "--quality",
            "high",
            "--seed",
            "42",
            "--json",
        ]))
        .unwrap();
        assert_eq!(a.prompt, "  一只猫  ");
        assert_eq!(a.refs, vec!["/tmp/a.png", "/tmp/b.png"]);
        assert_eq!(a.provider.as_deref(), Some("Hubway"));
        assert_eq!(a.n, Some(3));
        assert_eq!(a.size.as_deref(), Some("1024x1024"));
        assert_eq!(a.quality.as_deref(), Some("high"));
        assert_eq!(a.seed, Some(42));
        assert!(a.json);
    }

    #[test]
    fn parses_inline_values() {
        let a = parse_generate(&args(&["--prompt=猫", "--seed=7", "--json"])).unwrap();
        assert_eq!(a.prompt, "猫");
        assert_eq!(a.seed, Some(7));
        assert!(a.json);
    }

    #[test]
    fn missing_ref_value_is_an_error() {
        let err = parse_generate(&args(&["--prompt", "猫", "--ref"])).unwrap_err();
        assert!(err.to_string().contains("缺少取值"), "实际：{err}");
    }

    #[test]
    fn missing_prompt_is_an_error() {
        let err = parse_generate(&args(&["--provider", "x"])).unwrap();
        assert!(err.prompt.is_empty());
        // 空 prompt 在 generate 里会被拦下
        let e = generate(&args(&["--provider", "x"])).unwrap_err();
        assert!(e.to_string().contains("prompt 不能为空"), "实际：{e}");
    }

    #[test]
    fn unknown_flag_is_an_error() {
        let err = parse_generate(&args(&["--prompt", "猫", "--wat"])).unwrap_err();
        assert!(err.to_string().contains("未知参数"), "实际：{err}");
    }

    #[test]
    fn n_is_clamped_to_max() {
        // 解析层只保证是整数，越界由 clamp_n 收敛
        let a = parse_generate(&args(&["--prompt", "猫", "-n", "99"])).unwrap();
        assert_eq!(clamp_n(a.n.unwrap(), MAX_N), MAX_N);
        assert_eq!(clamp_n(0, MAX_N), 1);
    }

    #[test]
    fn non_numeric_n_is_an_error() {
        let err = parse_generate(&args(&["--prompt", "猫", "-n", "abc"])).unwrap_err();
        assert!(err.to_string().contains("-n 需要正整数"), "实际：{err}");
    }

    #[test]
    fn unknown_subcommand_is_an_error() {
        let err = run(args(&["nope"])).unwrap_err();
        assert!(err.to_string().contains("未知子命令"), "实际：{err}");
    }

    #[test]
    fn mcp_flag_reports_removal() {
        let err = run(args(&["--mcp"])).unwrap_err();
        assert!(err.to_string().contains("MCP 模式已移除"), "实际：{err}");
    }

    #[test]
    fn mime_for_path_accepts_only_images() {
        assert_eq!(mime_for_path(Path::new("/a/b.PNG")), Some("image/png"));
        assert_eq!(mime_for_path(Path::new("/a/b.jpeg")), Some("image/jpeg"));
        assert_eq!(mime_for_path(Path::new("/a/b.webp")), Some("image/webp"));
        assert_eq!(mime_for_path(Path::new("/a/b.gif")), Some("image/gif"));
        assert_eq!(mime_for_path(Path::new("/a/b.txt")), None);
        assert_eq!(mime_for_path(Path::new("/a/b")), None);
    }

    #[test]
    fn ref_from_path_encodes_base64_without_data_prefix() {
        let dir = std::env::temp_dir().join(format!("xiic-cli-ref-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.png");
        std::fs::write(&path, b"hello").unwrap();
        let r = ref_from_path(path.to_str().unwrap()).unwrap();
        assert_eq!(r.name, "a.png");
        assert_eq!(r.mime, "image/png");
        assert_eq!(r.data, "aGVsbG8=");
        assert!(!r.data.starts_with("data:"), "data 字段不应带 data: 前缀");
    }

    #[test]
    fn ref_from_path_rejects_non_image_and_missing_file() {
        let dir = std::env::temp_dir().join(format!("xiic-cli-ref-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let txt = dir.join("a.txt");
        std::fs::write(&txt, b"x").unwrap();
        let err = ref_from_path(txt.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("格式不支持"), "实际：{err}");

        let missing = dir.join("nope.png");
        let err = ref_from_path(missing.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("不存在或不可读"), "实际：{err}");
    }

    fn ref_stub() -> RefImage {
        RefImage {
            name: "a.png".into(),
            mime: "image/png".into(),
            data: "aGk=".into(),
        }
    }

    #[test]
    fn refs_guard_blocks_models_without_refs() {
        let caps = crate::capabilities::for_model(Protocol::OpenAiImages, "dall-e-3");
        assert!(!caps.refs);
        let err = check_refs_supported(&caps, &[ref_stub()], "DallE", "dall-e-3").unwrap_err();
        assert!(err.to_string().contains("不支持参考图"), "实际：{err}");
    }

    #[test]
    fn refs_guard_allows_models_with_refs() {
        let caps = crate::capabilities::for_model(Protocol::OpenAiImages, "gpt-image-1");
        assert!(caps.refs);
        assert!(check_refs_supported(&caps, &[ref_stub()], "Hubway", "gpt-image-1").is_ok());
    }

    #[test]
    fn refs_guard_passes_when_no_refs_given() {
        let caps = crate::capabilities::for_model(Protocol::OpenAiImages, "dall-e-3");
        assert!(check_refs_supported(&caps, &[], "DallE", "dall-e-3").is_ok());
    }

    #[test]
    fn result_json_has_no_data_url() {
        let outcome = TaskOutcome {
            task_id: "t1".into(),
            status: "completed".into(),
            images: vec![ImageMeta {
                id: "i1".into(),
                path: "/tmp/a.png".into(),
                thumb_path: "/tmp/a.thumb.jpg".into(),
            }],
            errors: vec![],
            db_error: None,
        };
        let v = build_result_json(&outcome);
        assert_eq!(v["task_id"], "t1");
        assert_eq!(v["status"], "completed");
        assert_eq!(v["count"], 1);
        assert_eq!(v["images"][0]["path"], "/tmp/a.png");
        assert_eq!(v["images"][0]["thumb_path"], "/tmp/a.thumb.jpg");
        assert!(
            v["images"][0].get("data_url").is_none(),
            "CLI 输出不应带 data_url"
        );
        assert!(v.get("error").is_none());
    }

    #[test]
    fn result_json_carries_error_and_warning() {
        let outcome = TaskOutcome {
            task_id: "t2".into(),
            status: "completed_with_errors".into(),
            images: vec![],
            errors: vec!["第 2 张失败".into()],
            db_error: Some("写库失败".into()),
        };
        let v = build_result_json(&outcome);
        assert_eq!(v["error"], "第 2 张失败");
        assert!(v["warning"].as_str().unwrap().contains("写库失败"));
    }
}
