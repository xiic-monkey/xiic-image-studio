---
name: xiic-image-studio
description: |
  Drive the local xiic-image-studio desktop app to generate images from the command
  line, including image-to-image generation with reference images, and to export the
  gallery's records as JSONL. Use this when the user asks to "generate an image",
  "用本地工作室生图", "以这张图为基础再生成一版", "把画廊导出成 JSONL", or wants to
  list the configured image providers. The results land in the same gallery the GUI
  shows. Do NOT use for cloud image APIs (DALL·E, Midjourney web), for video
  generation, or for editing pixels of an existing image locally.
---

# xiic-image-studio CLI

无头调用本地图片工作室：产物写进同一个 SQLite 与图库，GUI 打开即可看到。

## Inputs to collect

- **画面描述（prompt）**：用户没说就问，别自己编。
- **参考图路径**（可选）：以图生图时需要；最多 4 张，格式 png/jpg/jpeg/webp/gif，单张 ≤16MB。
- **用哪个 provider**（可选）：不指定就用第一个配好 API Key 的；用户明确点名时才传 `--provider`。
- **用哪个 model**（可选）：一家 provider 可挂多个不同协议的模型，不指定用默认模型；用户点名时传 `--model`。
  协议已覆盖 OpenAI Images、Chat 生图、Gemini 原生、MJ Proxy、火山方舟（豆包 Seedream）、
  阿里百炼（万相 / Qwen-Image）、Stability AI、Ideogram、Black Forest Labs FLUX、MiniMax 图像。

## Procedure

1. 调 `scripts/xiic.mjs` 执行对应子命令。
   原因：脚本负责定位二进制并转发参数，直接敲二进制路径在换机器/换安装方式后会失效。

   ```bash
   node scripts/xiic.mjs providers --json            # 先看有哪些 provider
   node scripts/xiic.mjs generate --prompt "一只戴帽子的猫" --json
   node scripts/xiic.mjs generate --prompt "改成夜景" --ref /abs/path/orig.png --json
   node scripts/xiic.mjs export --out /abs/path/out.jsonl --fav   # 导出画廊记录
   ```

   原因：先跑 `providers` 能避免"所有 provider 都没配 Key"这种错误在生成时才暴露。
   参数全集见 `references/cli-reference.md`。

2. 生成失败时读 stderr 的错误文本，按 `references/troubleshooting.md` 对号入座。
   原因：CLI 把错误原样打到 stderr 并以非零码退出，错误文本已经指向了具体原因。

3. 把返回的图片路径回报给用户，并说明产物已进入工作室图库。
   原因：用户要的是"能拿到图"，不只是"命令跑通了"。

## Output contract

- `--json` 输出结构：`{task_id, status, count, images: [{id, path, thumb_path}]}`。
  `path` 是本地绝对路径；可能有 `error`（部分失败）与 `warning`（图已入库但状态写回失败）。
- 不带 `--json` 时是纯文本：首行说明生成了几张图与 task_id，随后每行一个图片绝对路径。
- stdout 只有结果，错误一律在 stderr，失败时退出码非零。

## Failure handling

- **没配 provider / 没配 API Key**：把 stderr 原话转给用户，让他在 GUI 里配置后重试。
  不要自己造一个 provider 或替用户填 Key。
- **模型不支持参考图**：CLI 会明确报错（如 dall-e-3、Imagen）。改用支持参考图的 provider，
  或去掉 `--ref` 退回文生图。不要悄悄丢掉参考图继续生成——那会给用户一张完全不同的图。
- **参考图超 4 张 / 超 16MB / 格式不支持**：先按 `references/reference-images.md` 收敛输入再重试。
- **找不到二进制**：脚本会以退出码 127 报错。用 `node scripts/xiic.mjs --which` 看它找过哪些路径，
  再用 `XIIC_IMAGE_STUDIO_BIN` 环境变量指到正确的可执行文件。
- **上游超时/限流**：说明可能是上游问题，建议重试或换 provider；不要立即重复提交，
  上游可能仍在出图，重复提交会重复扣费。

## Examples

Input: 「用本地工作室画一只戴帽子的猫」
Output: 调 `generate --prompt "一只戴帽子的猫" --json`，回报图片绝对路径与 task_id。

Input: 「把这张图改成夜景」
Output: 调 `generate --prompt "改成夜景" --ref <该图路径> --json`，回报新图路径；
原图仍在图库里，新图作为另一条任务记录写入。

Input: 「把我收藏的那批图的提示词导出来」
Output: 调 `export --out <路径>.jsonl --fav`，回报导出的条数与文件路径。
导出的文件格式与 GUI「导入批量」一致，可以原样导回去再跑。

## Windows (win32) platform notes

脚本用 `node` 调用，路径探测兼容 Windows；唯一差异是找不到二进制时的报错里，
release 目录变成 `%USERPROFILE%\.cargo\bin` 与仓库内 `src-tauri\target\release\`。
用 `XIIC_IMAGE_STUDIO_BIN` 指定 `.exe` 路径可覆盖探测。
