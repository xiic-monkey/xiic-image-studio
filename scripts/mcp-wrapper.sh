#!/bin/bash
#
# xiic-image-mcp 入口：studio 无头 MCP 模式的自愈式启动器。
#
# 为什么不用软链指向 target/release：
#   cargo clean / rm -rf target 会把二进制删掉，MCP 端就变成"找不到可执行文件"，
#   而且这种失效很隐蔽，排查成本高。
# 为什么也不直接静态复制：
#   副本不会跟代码更新，会出现"改完了但 agent 调的还是老逻辑"。
#
# 这里的做法：两份都要。
#   - 真身在 target/release（跟着 build 走，最新的）
#   - 持久副本在 app_data_dir/headless-mcp（cargo clean 删不掉）
#   每次启动对比一次，真身更新就原子同步到持久副本；真身没了就用持久副本顶上；
#   两份都没有才报错，并且把恢复命令直接打出来。
#
# 对 MCP 协议零侵入：参数原样透传，最后 exec 成目标二进制，
# stdin/stdout 直接继承，所以 stdio 传输完全不受这一层包装影响。

set -uo pipefail

BUILD_BIN="${HOME}/RustroverProjects/xiic-image-studio/src-tauri/target/release/xiic-image-studio"
CACHE_BIN="${HOME}/Library/Application Support/com.xiic.image-studio/headless-mcp/xiic-image-studio"

# target 里的比持久副本新 -> 同步过来。
# 先写临时文件再 mv，保证多 agent 并发调用时不会 exec 到拷贝了一半的文件。
if [[ -x "$BUILD_BIN" && ( ! -x "$CACHE_BIN" || "$BUILD_BIN" -nt "$CACHE_BIN" ) ]]; then
    tmp="${CACHE_BIN}.tmp.$$"
    if cp -f "$BUILD_BIN" "$tmp" 2>/dev/null && mv -f "$tmp" "$CACHE_BIN" 2>/dev/null; then
        chmod +x "$CACHE_BIN" 2>/dev/null
    else
        rm -f "$tmp" 2>/dev/null
        # 同步失败不致命：只要那份还没被换掉，继续用旧的即可。
        echo "[xiic-image-mcp] 警告：同步新二进制失败，继续使用上一份" >&2
    fi
fi

if [[ ! -x "$CACHE_BIN" ]]; then
    echo "[xiic-image-mcp] 找不到可执行的 MCP 二进制。" >&2
    echo "  恢复方法（任选其一）：" >&2
    echo "    在项目根目录跑：cargo build --release   （src-tauri/ 下）" >&2
    echo "  或把 build 出来的 xiic-image-studio 放到：" >&2
    echo "    $CACHE_BIN" >&2
    exit 1
fi

exec "$CACHE_BIN" "$@"
