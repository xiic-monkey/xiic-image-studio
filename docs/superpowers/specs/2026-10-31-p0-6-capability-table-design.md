# P0-6 协议 × 模型能力表 设计

> 状态：已定稿（2026-10-31）。用户已确认两项关键决策：
> ① openai_images 协议下 seed 一律不发；② imagen-* 本次实现 `:predict` 端点分支。

**问题来源**：项目审核报告 P0-6「参数面板没有按协议/模型收敛，部分预设会直接报错」。

---

## 1. 背景

当前四个协议适配器各自决定"能发什么参数"，前端参数面板对所有协议共用同一组选项：

| 问题 | 现状 | 后果 |
|---|---|---|
| 尺寸 | 固定 `1024x1024 / 1536x1024 / 1024x1536 / 2048x2048` | `2048x2048` 对 `gpt-image-1` / `dall-e-3` 非法 → 官方 400 |
| seed | `openai_images` 无条件附带 | 官方 Images API 没有 `seed` 字段 → 400 |
| seed | gemini 放在 body 顶层 | 合法位置是 `generationConfig.seed` → 严格接口报错 |
| size / quality | gemini、chat 适配器直接忽略 | 用户以为设了尺寸/质量，结果没变 |
| Imagen | `imagen-4.0-generate-001` 走 `:generateContent` | Imagen 需要 `:predict`，该预设必定失败 |

根因不是某一行写错，而是**没有"这个协议+模型接受什么"的唯一真源**：适配器各自猜测，前端无从得知。

## 2. 目标 / 非目标

**目标**
1. 能力成为唯一真源，适配器与前端读同一份数据。
2. 不再向任何上游发送该协议/模型不支持的参数（含 MCP、重试、外部调用三条入口）。
3. 前端把不支持的能力**明确置灰并说明原因**，而不是静默忽略。
4. 修好 `imagen-*` 预设（`:predict` 分支）。

**非目标**
- 不做「协议 × 模型」的完整矩阵（模型名单持续增长，表按模型族匹配 + 未知模型宽松兜底）。
- 不改 P1 项：`parseBatch` 逗号解析、画廊分页/筛选/导出、任务与图片参数快照、Header 加密、MCP 并发闸门。
- 不引入任何新 Rust crate 或 npm 依赖。

## 3. 能力表（唯一真源）

新增 `src-tauri/src/capabilities.rs`（放 crate 根而非 `generate/` 下：`provider/types.rs` 要用它来给 `ProviderListItem` 挂字段，放根模块可避免 provider→generate 的反向依赖）：

```rust
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// 允许的尺寸取值；空 = 该协议/模型不接受尺寸参数
    pub sizes: &'static [&'static str],
    /// 允许的质量取值；空 = 不接受 quality
    pub quality: &'static [&'static str],
    pub seed: bool,
    pub refs: bool,
    pub mask: bool,
    /// 上游端点形态：images | chat | generate_content | predict | mj
    pub endpoint: &'static str,
    /// 前端尺寸控件的标签：尺寸 | 画面比例
    pub size_label: &'static str,
}

pub fn for_model(protocol: Protocol, model: &str) -> Capabilities;
```

### 3.1 表内容

| 协议 | 模型匹配 | sizes | quality | seed | refs | mask | endpoint | size_label |
|---|---|---|---|---|---|---|---|---|
| openai_images | `gpt-image*` | `1024x1024`, `1536x1024`, `1024x1536` | `auto`,`low`,`medium`,`high` | ✗ | ✓ | ✓ | images | 尺寸 |
| openai_images | `dall-e-3` | `1024x1024`, `1792x1024`, `1024x1792` | `standard`,`hd` | ✗ | ✓ | ✓ | images | 尺寸 |
| openai_images | 其它（中转自定义） | 上面 gpt-image 三项 + `2048x2048` | `auto`,`low`,`medium`,`high` | ✗ | ✓ | ✓ | images | 尺寸 |
| openai_chat_image | 全部 | — | — | ✓ | ✓ | ✗ | chat | 尺寸 |
| gemini_native | `gemini-*` | `1:1`,`3:4`,`4:3`,`9:16`,`16:9` | — | ✓ | ✓ | ✗ | generate_content | 画面比例 |
| gemini_native | `imagen-*` | `1:1`,`3:4`,`4:3`,`9:16`,`16:9` | — | ✗ | ✗ | ✗ | predict | 画面比例 |
| midjourney_proxy | 全部 | — | — | ✗ | ✓ | ✗ | mj | 尺寸 |

**匹配规则**（按顺序，先命中先用；模型名大小写不敏感、去掉 `models/` 前缀）：
1. `openai_images`：`starts_with("dall-e-3")` → dall-e-3 行；`starts_with("gpt-image")` → gpt-image 行；否则中转兜底行。
2. `gemini_native`：`starts_with("imagen")` → imagen 行；否则 gemini 行。
3. `openai_chat_image` / `midjourney_proxy`：不区分模型，固定行。

**兜底原则**：未知模型走宽松行（多给选项），宁可让上游报错也不要静默吞掉用户设定的参数；但 `seed` 在 `openai_images` 下一律 ✗（用户已确认），因为该字段不属于 Images API 规范。

### 3.2 端点形态

`endpoint` 决定 gemini 走哪个分支（`generate_content` / `predict`）。其它协议端点固定，不靠这个字段分支。

## 4. 三个消费点

### 4.1 适配器（`src-tauri/src/generate/adapters.rs`）

每个适配器在构造请求体前先取 `let caps = capabilities::for_model(协议, &ctx.model);`，只写 `caps` 允许的字段。

**openai_images**（`/v1/images/generations` 或 `/v1/images/edits`）
- `size`：仅当 `caps.sizes` 含该值时写入。
- `quality`：仅当 `caps.quality` 含该值时写入。
- `seed`：**永不写入**（multipart 与 JSON 两条路径都不写）。
- `mask`：仅当 `caps.mask` 为真时作为 multipart part 附加。

**openai_chat_image**（`/v1/chat/completions`）
- 不写 `size` / `quality`（`caps.sizes` 为空）。
- `seed` 保留在 body 顶层（Chat Completions 的标准参数，未实现的模型会忽略而非报错）。

**gemini_native · generate_content**（`/v1beta/models/{model}:generateContent`）
- `size` → `generationConfig.aspectRatio`。
- `seed` → `generationConfig.seed`（**修正顶层位置错误**）。
- 参考图 → `contents[0].parts[].inlineData`。

**gemini_native · predict**（`/v1beta/models/{model}:predict`，Imagen 新分支）
- 请求体：`{"instances":[{"prompt":…}],"parameters":{"aspectRatio":…,"sampleImageSize":"1K"}}`。
- 响应解析：`predictions[]` 每项的 `bytesBase64Encoded`（base64 解码），可选 `mimeType`。
- 不支持参考图 / 蒙版 / seed。

**midjourney_proxy**：不变（现状已不发送 size/quality/seed/mask）。

### 4.2 服务端兜底（`src-tauri/src/capabilities.rs`）

裁剪逻辑写成**纯函数**，便于单测；`submit_task`（`src-tauri/src/generate/commands.rs`）在构造 `GenCtx` 之前调用它，保证 MCP、重试、外部调用三条入口都发不出非法参数：

```rust
/// 按能力表裁剪请求：丢弃该协议/模型不支持的参数。
pub fn sanitize(caps: &Capabilities, req: &mut GenerateRequest) {
    req.size    = req.size.take().filter(|s| caps.sizes.contains(&s.as_str()));
    req.quality = req.quality.take().filter(|q| caps.quality.contains(&q.as_str()));
    if !caps.seed { req.seed = None; }
    if !caps.mask { req.mask = None; }
}
```

裁剪是静默的（不报错）——用户设了不支持的参数属于 UI 层已告知的既成事实，服务端只需保证不把非法参数发出去。

### 4.3 前端

**契约**：`ProviderListItem` 增加 `capabilities: Capabilities` 字段，由 `src-tauri/src/provider/db.rs::list()` 在列举时计算。**不新增 IPC 命令**，前端 store 零改动即可拿到。

**工作台（`src/views/WorkbenchView.tsx`）**
- 尺寸下拉选项 = `caps.sizes`；当前 `size` 不在表内时回落到 `caps.sizes[0]`。
- `caps.sizes` 为空（chat / MJ）→ **不显示尺寸控件**（不是显示空下拉）。
- 尺寸控件标签 = `caps.sizeLabel`（gemini 显示"画面比例"）。
- 质量下拉：`caps.quality` 为空则**不显示该控件**；非空则选项 = `caps.quality`。
- 种子输入框：`caps.seed` 为假时 `disabled` + title 说明「当前协议/模型不支持种子」。
- 蒙版入口：`refs.length > 0 && caps.mask`（取代现在的"按协议判断"）。
- 提交时按能力裁剪后再发（与服务端兜底一致，避免发无意义字段）。

**新增前端纯函数 `src/lib/capabilities.ts`**（可 `node --test`）：
- `normalizeSize(caps, current)` → 当前值合法则原样，否则取表内第一个。
- `normalizeQuality(caps, current)` → 同上（表空时返回 `undefined`）。
- `isSeedEnabled(caps)` → `caps.seed`。

## 5. 错误处理

- 能力表查不到模型 → 走宽松兜底行，**不报错**。
- 上游因参数非法返回 4xx → 沿用现有 `AppError::Status(u16, msg)`，错误文案原样透出，不做二次包装。
- 前端裁剪后若某参数被丢弃 → UI 上该项已置灰/不显示，用户可从提示中看到原因，不额外弹提示。

## 6. 测试策略

**Rust（`cargo test`）**
- `capabilities.rs` 单测：逐协议逐模型族断言 `sizes` / `quality` / `seed` / `endpoint`；未知模型走兜底行；大小写与 `models/` 前缀归一。
- 适配器映射单测：mock HTTP（沿用 `adapters.rs` 现有 `TcpListener` 手写服务器模式）断言
  - openai_images 请求体**不含** `seed`，含合法 `size` / `quality`；
  - gemini 请求体 `generationConfig.seed` 存在且顶层无 `seed`，`generationConfig.aspectRatio` 正确；
  - imagen 请求 URL 以 `:predict` 结尾，响应 `predictions[].bytesBase64Encoded` 能解析出图片字节。
- `capabilities::sanitize` 单测：非法 `size` / `quality` 被丢弃；`caps.seed`/`caps.mask` 为假时 `seed`/`mask` 被清空。

**前端（`npm test`，Node 内置 runner）**
- `src/lib/capabilities.test.ts`：尺寸回落、质量回落、seed 置灰判定。

**回归**：`cargo test`、`cargo clippy --all-targets`、`npm test`、`npm run build` 全绿；`git diff --check` 无输出。

## 7. 约束

- 不新增 Rust crate，不新增 npm 依赖。
- 能力表是唯一真源：适配器不得自行判断"支不支持某参数"。
- 前端不做能力推断，只消费后端下发的 `capabilities`。
- API Key 仍留在后端，不经 MCP 暴露；本次不改动 `custom_headers` 暴露面。
- 数据库结构不变（能力表不落库，由 `(protocol, model)` 纯函数推导）。

## 8. 文档

`docs/DESIGN.md` 需同步：
- §参数说明：`seed` 下发范围收窄为 openai_chat_image / gemini_native（`generationConfig.seed`），openai_images 不再下发。
- 新增能力表说明与文件位置。
- §8 目录结构补 `src-tauri/src/capabilities.rs` 与 `src/lib/capabilities.ts`。
