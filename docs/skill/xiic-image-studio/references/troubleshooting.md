# 排查

CLI 的错误文本一律走 stderr。把它原样转给用户，别改写。

## "还没有配置任何 provider，请先在 image-studio 应用里添加一个"

工作室的库 `~/Library/Application Support/com.xiic.image-studio/image-studio.db` 里没有 provider。
需要用户打开 GUI 配置，CLI 不负责建 provider。

## "所有 provider 都还没填 API Key，请先在 image-studio 应用里配置"

有 provider 但都没配 Key。同样要用户在 GUI 里填。

## "provider 'X' 还没填 API Key" / "找不到 provider 'X'（可用：A、B）"

`--provider` 传的名字没匹配上，或匹配上了但那个 provider 没 Key。
错误里会列出可用的 provider 名字，改用其中一个即可。

## "provider 'X' 的模型 'Y' 不支持参考图……"

能力表判定该模型 `refs=false`。两个选择：换支持参考图的 provider，或去掉 `--ref`。
不要悄悄去掉参考图继续跑——那会给用户一张完全不同的图。

## "参考图格式不支持：X（支持 png / jpg / jpeg / webp / gif）"

按扩展名判定。确认文件确实是图片，或先转成 png。

## "参考图超过 16MB 上限：X"

压缩或缩小后再传。

## "参考图最多 4 张，收到 N 张"

去掉多余的，或分批生成。

## "找不到 xiic-image-studio 可执行文件"（退出码 127）

脚本按 `XIIC_IMAGE_STUDIO_BIN` → `PATH` → 仓库内 `src-tauri/target/release/` →
`/Applications/xiic-image-studio.app/Contents/MacOS/` 的顺序找。
先看它找过哪些路径：

```bash
node scripts/xiic.mjs --which
```

还是找不到，就用环境变量指过去：

```bash
export XIIC_IMAGE_STUDIO_BIN=/path/to/xiic-image-studio
```

如果本机从没编译过，在仓库里跑 `cargo build --release` 生成二进制。

## "上游超时" / "上游返回错误（HTTP 429）"

上游侧问题：限流或超时。**不要立刻重复提交**——请求可能已经送达上游、图还在生成，
重复提交会重复扣费。先等一会儿，或换 provider。

## "上游返回错误（HTTP 400）"

参数被上游拒了。常见原因是 size/quality 取值该模型不接受。
跑 `providers --json` 看 `capabilities` 里允许的取值，再用合法的值重试。

## "图片已入库，但任务状态写入失败"

`warning` 字段出现这条。图已经生成并入库了，能正常用；只是任务卡片的状态可能停在"生成中"。
不影响交付，把路径给用户即可。

## "数据库错误: ..."

写库失败。常见原因是 GUI 正占着写锁——两边都设了 `busy_timeout(10s)` 排队，
偶发撞锁重试一次通常就好。持续失败说明库文件有问题，需要进一步查。

## 命令跑通了，但 GUI 里看不到新图

正常：GUI 靠轮询发现外部进程写入的任务（工作台 3s、画廊 5s）。
等几秒或切一下页面就会出来。
