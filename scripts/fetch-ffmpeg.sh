#!/usr/bin/env bash
# 准备 ffmpeg sidecar（Tauri externalBin 用）。
#
# 为什么它不在 git 仓库里：43MB、平台专属、且是二进制——塞进 git 后每个 clone 都要
# 背着它，历史还会永久留存。改为托管在 GitHub Releases，**只在要打包 .app 时**拉取。
#
# 日常 `tauri dev` 不需要跑这个脚本：media.rs 的 ffmpeg 查找顺序是
#   用户自定义 > 内置 sidecar > app_data/bin > Homebrew > PATH
# 本机有 ffmpeg（brew install ffmpeg）就能直接用。
#
# 注意：必须用 Release 上那份自包含构建。Homebrew 版的 ffmpeg 动态链接
# /opt/homebrew/Cellar/... 的 dylib，拷进 .app 发给别人会 dyld 报错。

set -euo pipefail

REPO="xiic-monkey/xiic-image-studio"
TAG="ffmpeg-v1"
DEST_DIR="src-tauri/binaries"
# Release 上目前只有 macOS arm64 这一份
TRIPLE="aarch64-apple-darwin"
SHA256="a90e3db6a3fd35f6074b013f948b1aa45b31c6375489d39e572bea3f18336584"

if [ "$(uname -s)" != "Darwin" ] || [ "$(uname -m)" != "arm64" ]; then
  echo "⚠️  当前平台 $(uname -s)/$(uname -m) 没有预置的 ffmpeg sidecar。"
  echo "   自用无妨：media.rs 会回退到系统 ffmpeg（brew install ffmpeg）。"
  echo "   要分发 .app 的话，请自行下载静态构建放到 $DEST_DIR/ffmpeg-<target-triple>。"
  exit 0
fi

DEST="$DEST_DIR/ffmpeg-$TRIPLE"
if [ -x "$DEST" ]; then
  echo "✓ ffmpeg sidecar 已存在，跳过：$DEST"
  exit 0
fi

URL="https://github.com/$REPO/releases/download/$TAG/ffmpeg-$TRIPLE"
echo "⬇  下载 ffmpeg ($TRIPLE)，约 43MB ..."
mkdir -p "$DEST_DIR"
curl -fL --progress-bar -o "$DEST" "$URL"
chmod +x "$DEST"

ACTUAL=$(shasum -a 256 "$DEST" | awk '{print $1}')
if [ "$ACTUAL" != "$SHA256" ]; then
  echo "✗ sha256 校验失败（期望 $SHA256，实得 $ACTUAL），已删除下载文件" >&2
  rm -f "$DEST"
  exit 1
fi
echo "✓ ffmpeg 就绪：$DEST"
