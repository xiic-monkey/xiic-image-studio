# 参考图 / 以图生图

## 用法

```
xiic-image-studio generate --prompt "改成夜景" --ref /abs/path/orig.png --json
xiic-image-studio generate --prompt "融合这两张" --ref a.png --ref b.png --json
```

`--ref` 可重复，最多 4 张（与 GUI 的参考图上限一致）。

## 约束

| 约束 | 值 | 违反时的表现 |
|---|---|---|
| 最多几张 | 4 | 报错退出（不静默丢弃） |
| 单张大小 | ≤ 16MB | 报错退出 |
| 格式 | png / jpg / jpeg / webp / gif | 报错退出（按扩展名判定） |
| 必须是普通文件 | 是 | 报错退出 |

`--ref` 接受任意本地绝对路径，不限于工作室图库目录。
读取时只做扩展名白名单 + 大小上限，不做目录越界校验——
路径由调用方（用户/agent）显式给出，不是从远端拉来的 URL。

## 能力守卫（重要）

**不是所有模型都支持参考图。** 能力表 `caps.refs` 由协议 × 模型决定：

| 协议 / 模型 | 支持参考图 |
|---|---|
| openai_images + `gpt-image-*` | ✅（走 `/v1/images/edits`） |
| openai_images + `dall-e-2` | ✅ |
| openai_images + `dall-e-3` | ❌（DALL·E 3 没有 edits 端点） |
| openai_chat_image | ✅（inlineData） |
| gemini_native + `gemini-*` | ✅ |
| gemini_native + `imagen-*` | ❌（走 `:predict` 端点） |
| midjourney_proxy | ✅（`base64Array`） |
| volcengine_ark | ✅（`image` 字段，最多 10 张） |
| dashscope | ❌（文生图端点 `image-synthesis` 不收参考图） |

给不支持的模型带图，CLI 会**直接报错**并指出该走哪个端点：

```
provider 'Hubway' 的模型 'dall-e-3' 不支持参考图（该协议走 images 端点，带图会被上游拒绝）。
请换一个支持参考图的 provider，或去掉 --ref。
```

**为什么不能静默丢弃参考图**：带图和不带图是完全不同的产物。丢掉参考图却照常返回一张图，
用户会以为"参考生效了"，实际拿到的是另一张毫不相干的图。宁可失败也不给假成功。

先跑 `providers --json` 看 `capabilities.refs` 就能确认。

## 参考图不会被存进数据库

只存张数（`request_json` 里的 `ref_count`），不存图本体。
后果：这类任务**没有"一键重试"入口**——重试拿不回参考图，重试等于退化成文生图，
结果和原来不一样。GUI 与 CLI 行为一致。
