use crate::error::{AppError, AppResult};
use base64::Engine;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn tools_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::General(e.to_string()))?
        .join("images")
        .join("tools");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 用户自定义 ffmpeg 路径（存 app_data/ffmpeg_path.txt，留空=自动查找）
fn custom_path_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("ffmpeg_path.txt"))
}

fn read_custom_path(app: &AppHandle) -> Option<String> {
    let p = custom_path_file(app)?;
    let s = std::fs::read_to_string(p).ok()?;
    let s = s.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// ffmpeg 查找顺序：用户自定义路径 > 内置 sidecar（exe 同目录）> 应用数据目录 bin/ > Homebrew > PATH
/// 返回 (路径, 来源)
fn ffmpeg_bin_with_source(app: &AppHandle) -> (String, &'static str) {
    if let Some(custom) = read_custom_path(app) {
        if PathBuf::from(&custom).exists() {
            return (custom, "custom");
        }
    }
    // 内置 sidecar：tauri 会把 externalBin 复制到可执行文件同目录（dev 在 target/debug/）
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let p = exe_dir.join("ffmpeg");
            if p.exists() {
                return (p.to_string_lossy().to_string(), "bundled");
            }
        }
    }
    if let Ok(dir) = app.path().app_data_dir() {
        let p = dir.join("bin").join("ffmpeg");
        if p.exists() {
            return (p.to_string_lossy().to_string(), "manual");
        }
    }
    for p in ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"] {
        if PathBuf::from(p).exists() {
            return (p.to_string(), "system");
        }
    }
    ("ffmpeg".to_string(), "path")
}

fn ffmpeg_bin(app: &AppHandle) -> String {
    ffmpeg_bin_with_source(app).0
}

/// 设置/清除自定义 ffmpeg 路径；空串表示恢复自动查找
#[tauri::command]
pub fn tools_set_ffmpeg_path(app: AppHandle, path: String) -> AppResult<serde_json::Value> {
    let target = path.trim();
    if !target.is_empty() {
        let p = PathBuf::from(target);
        if !p.exists() {
            return Err(AppError::General(format!("路径不存在: {target}")));
        }
        // 粗校验：可执行且确实是 ffmpeg
        let out = std::process::Command::new(&p).arg("-version").output();
        match out {
            Ok(o) if o.status.success() => {}
            Ok(_) => return Err(AppError::General("该文件无法执行，请确认是 ffmpeg 二进制".into())),
            Err(e) => return Err(AppError::General(format!("无法执行: {e}"))),
        }
    }
    let file = custom_path_file(&app).ok_or_else(|| AppError::General("无法定位应用数据目录".into()))?;
    std::fs::write(file, target)?;
    // 清掉编码器缓存，让新的 ffmpeg 生效
    Ok(tools_status(app)?)
}

async fn run_ffmpeg(app: &AppHandle, args: &[String]) -> AppResult<()> {
    let bin = ffmpeg_bin(app);
    let out = tokio::process::Command::new(bin)
        .args(args)
        .arg("-y")
        .output()
        .await
        .map_err(|e| AppError::General(format!("无法启动 ffmpeg: {e}（请确认已安装，或将 ffmpeg 放入应用数据目录 bin/ 下）")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::General(format!(
            "ffmpeg 失败: {}",
            stderr.lines().rev().take(3).collect::<Vec<_>>().join(" | ")
        )));
    }
    Ok(())
}

/// 可用的图像编码器（按当前 ffmpeg 路径缓存，切换路径后自动失效）
fn encoders(app: &AppHandle) -> Vec<String> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Option<(String, Vec<String>)>>> =
        std::sync::OnceLock::new();

    let bin = ffmpeg_bin(app);
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = cache.lock().unwrap();
    if let Some((cached_bin, list)) = guard.as_ref() {
        if *cached_bin == bin {
            return list.clone();
        }
    }
    let list: Vec<String> = match std::process::Command::new(&bin)
        .args(["-hide_banner", "-encoders"])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| {
                let parts: Vec<&str> = l.split_whitespace().collect();
                if parts.len() >= 2 && parts[0].starts_with('V') {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    *guard = Some((bin, list.clone()));
    list
}

fn encoder_for(format: &str) -> &'static str {
    match format.to_lowercase().as_str() {
        "jpg" | "jpeg" => "mjpeg",
        "webp" => "webp",
        "gif" => "gif",
        _ => "png",
    }
}

fn format_supported(app: &AppHandle, format: &str) -> bool {
    let need = encoder_for(format);
    let available = encoders(app);
    available
        .iter()
        .any(|e| e == need || e.contains(need))
}

fn scale_filter(max_side: Option<u32>) -> Option<String> {
    max_side.map(|m| {
        format!("scale='min(iw,{m})':'min(ih,{m})':force_original_aspect_ratio=decrease")
    })
}

fn out_path(dir: &PathBuf, src: &str, ext: &str) -> PathBuf {
    let stem = PathBuf::from(src)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "out".into());
    dir.join(format!("{stem}-{}.{}", chrono::Utc::now().timestamp_millis(), ext))
}

#[tauri::command]
pub fn tools_status(app: AppHandle) -> AppResult<serde_json::Value> {
    let (bin, source) = ffmpeg_bin_with_source(&app);
    let exists = PathBuf::from(&bin).exists();
    let available = exists || bin == "ffmpeg";
    let formats = if available {
        serde_json::json!({
            "png": format_supported(&app, "png"),
            "jpg": format_supported(&app, "jpg"),
            "webp": format_supported(&app, "webp"),
            "gif": format_supported(&app, "gif"),
        })
    } else {
        serde_json::json!(null)
    };
    Ok(serde_json::json!({ "available": available, "path": bin, "source": source, "formats": formats }))
}

/// 格式转换 / 压缩 / 改尺寸
#[tauri::command]
pub async fn tools_convert(
    app: AppHandle,
    inputs: Vec<String>,
    format: String,
    quality: Option<u32>,
    max_side: Option<u32>,
) -> AppResult<Vec<String>> {
    let dir = tools_dir(&app)?;
    if !format_supported(&app, &format) {
        return Err(AppError::General(format!(
            "当前 ffmpeg 不支持输出 {format}（缺少 {} 编码器）。brew 版 ffmpeg 默认不带 webp，可改用 png / jpg，或自行编译带 libwebp 的 ffmpeg 放到应用数据目录 bin/ 下。",
            encoder_for(&format)
        )));
    }
    let mut outputs = Vec::new();
    for src in &inputs {
        if !PathBuf::from(src).exists() {
            return Err(AppError::General(format!("文件不存在: {src}")));
        }
        let out = out_path(&dir, src, &format);
        let mut args: Vec<String> = vec!["-i".into(), src.clone()];
        if let Some(f) = scale_filter(max_side) {
            args.extend(["-vf".into(), f]);
        }
        // 质量仅对有损格式生效：1-100 → -q:v 2(best)~10(worst)
        if matches!(format.as_str(), "jpg" | "jpeg" | "webp") {
            let q = quality.unwrap_or(85).clamp(1, 100);
            let qv = (2.0 + (100 - q) as f64 / 100.0 * 8.0).round() as u32;
            args.extend(["-q:v".into(), qv.to_string()]);
        }
        args.push(out.to_string_lossy().to_string());
        run_ffmpeg(&app, &args).await?;
        outputs.push(out.to_string_lossy().to_string());
    }
    Ok(outputs)
}

/// 多图合成 GIF（按传入顺序播放）
#[tauri::command]
pub async fn tools_gif(
    app: AppHandle,
    inputs: Vec<String>,
    fps: Option<f64>,
    max_side: Option<u32>,
) -> AppResult<String> {
    if inputs.len() < 2 {
        return Err(AppError::General("GIF 至少需要 2 张图".into()));
    }
    let dir = tools_dir(&app)?;
    for src in &inputs {
        if !PathBuf::from(src).exists() {
            return Err(AppError::General(format!("文件不存在: {src}")));
        }
    }
    let out = dir.join(format!("gif-{}.gif", chrono::Utc::now().timestamp_millis()));

    let fps = fps.unwrap_or(4.0).clamp(0.5, 30.0);
    let ms = max_side.unwrap_or(512);
    // 每路输入作为持续 1 秒的静态帧（-loop 1 -t 1），再统一 scale+pad 到 ms×ms 公共画布后 concat。
    // 用 concat filter（而非 demuxer）：跨 ffmpeg 版本更稳，且能统一不一致尺寸。
    let mut args: Vec<String> = Vec::new();
    for src in &inputs {
        args.push("-loop".into());
        args.push("1".into());
        args.push("-t".into());
        args.push("1".into());
        args.push("-i".into());
        args.push(src.clone());
    }
    let mut fc = String::new();
    let mut scaled: Vec<String> = Vec::new();
    for i in 0..inputs.len() {
        fc.push_str(&format!(
            "[{i}:v]scale='min(iw,{ms})':'min(ih,{ms})':force_original_aspect_ratio=decrease:flags=lanczos,pad={ms}:{ms}:(ow-iw)/2:(oh-ih)/2:color=white[s{i}];"
        ));
        scaled.push(format!("[s{i}]"));
    }
    fc.push_str(&format!(
        "{}{}concat=n={}:v=1:a=0,fps={},split[a][b];[a]palettegen[p];[b][p]paletteuse",
        scaled.concat(),
        "",
        inputs.len(),
        fps
    ));
    args.push("-filter_complex".into());
    args.push(fc);
    args.push(out.to_string_lossy().to_string());
    run_ffmpeg(&app, &args).await?;
    Ok(out.to_string_lossy().to_string())
}

/// 视频抽帧 → PNG 列表
#[tauri::command]
pub async fn tools_video_frames(
    app: AppHandle,
    video: String,
    interval_sec: Option<f64>,
    max_frames: Option<u32>,
    max_side: Option<u32>,
) -> AppResult<Vec<String>> {
    if !PathBuf::from(&video).exists() {
        return Err(AppError::General(format!("文件不存在: {video}")));
    }
    let dir = tools_dir(&app)?;
    let frame_dir = dir.join(format!("frames-{}", chrono::Utc::now().timestamp_millis()));
    std::fs::create_dir_all(&frame_dir)?;
    let interval = interval_sec.unwrap_or(1.0).clamp(0.1, 60.0);
    let mut args: Vec<String> = vec!["-i".into(), video.clone()];
    let mut vf = format!("fps=1/{interval}");
    if let Some(f) = scale_filter(max_side) {
        vf.push_str(&format!(",{f}"));
    }
    args.extend(["-vf".into(), vf, "-frames:v".into(), max_frames.unwrap_or(12).clamp(1, 60).to_string()]);
    args.push(frame_dir.join("f-%03d.png").to_string_lossy().to_string());
    run_ffmpeg(&app, &args).await?;

    let mut outs: Vec<String> = std::fs::read_dir(&frame_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".png"))
        .collect();
    outs.sort();
    Ok(outs)
}

/// 本地文件转 base64 data url（供抽帧/转换结果直接作为参考图）
#[tauri::command]
pub fn tools_file_b64(path: String) -> AppResult<String> {
    let bytes = std::fs::read(&path)?;
    let ext = PathBuf::from(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    };
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}
