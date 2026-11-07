# xiic-image-studio 设计文档

> 版本 v0.2 · 2026-09-25 · 状态：MVP + Phase 2 主要功能已实现（含 MCP 无头模式）

## 1. 定位

本地优先的 AI 生图工作台（桌面端，macOS 优先，兼容 Windows）。接任意供应商/中转站的生图 API，提示词、参数、任务、成品图全部留在本机。

一句话：**填 Key + Base URL，生图这件事在本地跑起来，图留在自己手里。**

## 2. 竞品调研结论

| 项目 | 技术栈 | 值得借鉴 | 缺陷 |
|---|---|---|---|
| PixAI | Electron+React | 多会话工作台、SQLite 历史、参数回填、提示词库 | 仅 OpenAI Images 协议 |
| Image_Gen Studio | Electron | 蒙版局部重绘、JSONL 批量任务 | 仅 Windows、单协议 |
| 云桥 Image Studio | Electron+React | 修图工具箱（扩图/换背景/去杂物）、批量生产、行业模板 | 偏电商垂直场景 |
| mj-studio | Nuxt 自部署 | **多协议适配**（MJ-Proxy / DALL-E / Gemini / Chat 生图）、任意中转站 | 需部署服务端，非桌面端 |
| syw2014/image-studio | Python+浏览器 | 模型自动发现（/v1/models 筛图像模型）、预设+模型名覆盖 | 浏览器形态，非桌面 |
| Cherry Studio | Electron | 多供应商管理、绘画板支持自定义 OpenAI 兼容生图端点（含中转） | 通用 AI 客户端：绘画板仅 OpenAI 生图格式，无 MJ 协议/蒙版重绘/并发队列/参数回填画廊 |

**核心借鉴点：多协议适配器 + 中转站友好 + 本地画廊 + 参数回填。**

## 3. 技术选型

| 层 | 选型 | 理由 |
|---|---|---|
| 桌面框架 | **Tauri 2** | 包体积/内存远小于 Electron；复用 voice-studio 经验；Rust 侧做并发队列和文件管理更稳 |
| 前端 | React 19 + TypeScript + Vite | 熟悉、生态成熟 |
| UI | Tailwind CSS v4 + shadcn/ui | 快速搭建工作台风格，暗色友好 |
| 状态 | Zustand + TanStack Query | 轻量，服务端状态（任务/画廊）好管理 |
| 本地存储 | SQLite（rusqlite，Rust 侧直连） | 画廊/任务/配置持久化，单文件无依赖 |
| 密钥存储 | keyring crate（系统钥匙串） | API Key 不落 SQLite 明文 |
| HTTP | Rust reqwest | 代理/超时/流式可控，支持自定义 baseUrl |
| ffmpeg | sidecar 二进制 + tauri-plugin-shell | 格式转换、压缩、视频抽帧 |
| 图片处理 | Rust image crate | 缩略图、EXIF、蒙版处理 |

## 4. 核心设计：Provider 多协议适配层

这是整个项目的灵魂。**不绑定任何官方端点，一切皆"供应商配置"**：

```
ProviderConfig {
  name, base_url, api_key(钥匙串引用),
  protocol: OpenAIImages | OpenAIChatImage | GeminiNative | MidjourneyProxy,
  model, extra_headers, custom_model_name   // 预设协议 + 手动覆盖模型名
}
```

四套协议适配器（对标 mj-studio，覆盖中转站全部主流形态）：

| 协议 | 端点 | 典型模型 |
|---|---|---|
| **OpenAIImages** | `POST {base}/v1/images/generations` / `edits` | gpt-image-1/2、dall-e-3、Flux、seedream |
| **OpenAIChatImage** | `POST {base}/v1/chat/completions`（回复中解析图片 URL/b64） | gpt-4o-image、Grok image、通义万相、nano-banana |
| **GeminiNative** | `POST {base}/v1beta/models/{m}:generateContent` | gemini-2.5-flash-image |
| **MidjourneyProxy** | `/mj/submit/imagine`、`/mj/task/{id}/fetch`（轮询） | midjourney 系（含垫图、U/V 操作） |

辅助能力：
- **模型发现**：拉 `/v1/models`，自动筛出疑似图像模型并猜协议（借鉴 image-studio）
- **连通性测试**：保存前一键测活
- **自定义 Header**：中转站要求的 `x-api-key`、鉴权头等自由加

## 5. 功能清单

### MVP（M1–M3）
1. **供应商管理**：多供应商 CRUD、协议选择、baseUrl/Key、连通测试、模型发现、协议预设一键填
2. **文生图**：prompt/负向词、尺寸预设+自定义、质量、n 张并发、seed
3. **图生图**：参考图（多图）上传、拖拽/粘贴/画廊选图
4. **任务队列**：提交→排队→并发执行→重试→取消，实时进度，失败原因落库
5. **本地图廊**：缩略墙、搜索（prompt/模型/日期）、收藏、批量导出、预览（滚轮缩放/平移/左右切换）、**参数回填**（点一张图恢复全部生成参数）
6. **多会话工作台**：会话级草稿（prompt、模型、参数自动保存）
7. **提示词库**：模板 CRUD、检索、一键套用

### Phase 2（M4+，已实现）
8. **局部重绘**：Canvas 蒙版涂抹 → images/edits（仅 OpenAI Images 协议精确蒙版，其他协议退化为普通图生图）
9. **图片工具箱**（ffmpeg sidecar）：webp/avif/png 转换、批量压缩、GIF 合成、视频抽帧做参考图、改尺寸
10. **批量任务**：工作台「按行批量」开关（默认关）——开着时一行一个任务、每行 1 张；
    关着时整段多行文本按**一条**提示词提交，`数量`生效。换行本身不再隐含批量，
    模板追加因此能把多段描述合成一条。另有 **JSONL/CSV 文件导入**（"导入批量"按钮）：
    按文件扩展名判格式（.csv / .jsonl / .txt，认不出扩展名时看内容），
    CSV 走标准引号规则（引号内的逗号不算分隔符、`""` 是转义），纯文本整行即一条提示词。
11. **MJ 扩展操作**：U/V 放大变换按钮（后端 `mj_action` worker + 前端 GalleryView）
12. **预设模板库**：内置电商/头像/插画/摄影四类场景模板，工作台「模板」按钮插入（追加到提示词，可选带 size/quality）

### Phase 3（M5+，已实现）
13. **MCP 无头模式**：`xiic-image-studio --mcp` 纯 stdio（不占端口、非常驻），对外暴露 `generate_image` / `list_providers`。
    与 GUI 共用 `run_one_core` / `create_task` / `finish_task_row`，写同一张 SQLite、同一个 `images/` 目录；
    前端靠轮询发现无头进程写入的任务。详见 §10。

### 参数说明（已对齐实现）
- **seed（随机种子）**：由「协议 × 模型能力表」（`src-tauri/src/capabilities.rs`）决定是否下发。openai_chat_image 走请求体顶层（Chat Completions 标准参数）；gemini_native 走 `generationConfig.seed`；**openai_images 不下发**（Images API 规范里没有该字段，官方与转 OpenAI 的中转站都会 400）。上游不支持时静默忽略，不影响其他参数。
- **n（数量）**：前端「数量」选择器 → 后端 `spawn_task` 按 `0..n` 并发 `buffer_unordered`（上限 4）各生成一张，任务 `total=n`。
- **重试**：仅 `ref_count==0` 的任务（纯文生图）提供重试入口；图生图因参考图未入库，重试会退化，前端据此隐藏入口。
- **能力表（协议 × 模型）**：`for_model(protocol, model)` 推导该组合实际接受的参数
  （尺寸取值 / 质量取值 / 是否支持 seed、参考图、蒙版 / 上游端点形态）。
  适配器据此发参，GUI 与 MCP 提交入口据此裁剪请求，前端据此收敛控件并把不支持项置灰。
  模型名匹配不到时走宽松兜底行（宁可上游报错，也不静默吞掉用户设的参数）。
  Imagen（`imagen-*`）走 `:predict` 端点分支。

## 6. 架构

```
┌─ 前端 React (WebView) ─────────────────────────┐
│  会话工作台 │ 任务队列面板 │ 画廊 │ 设置/供应商 │
└──────────────┬──────────────────────────────────┘
               │ Tauri IPC (invoke / event)
┌─ Rust 核心 ──┴──────────────────────────────────┐
│ provider 模块   四套协议适配器 + 模型发现        │
│ generate 模块   adapters / client / queue(并发)  │
│ storage 模块    rusqlite (任务/会话/图片元数据)  │
│ media 模块      image 缩略图 ｜ ffmpeg sidecar  │
│ crypto 模块     AES-256-GCM 本地密钥             │
│ mcp 模块        --mcp 无头 stdio（纯 stdio）     │
└─────────────────────────────────────────────────┘
产出 → workspace/images/{YYYY-MM-DD}/{task_id}/  本地落盘

无头模式与 GUI 是「两个进程、同一张库、同一个 images 目录」：
  xiic-image-studio --mcp  ←→  SQLite  ←→  GUI（前端轮询发现新任务）
```

## 7. 数据模型（SQLite）

```sql
providers   (id, name, base_url, protocol, model, custom_headers, created_at)  -- key 只存钥匙串引用
sessions    (id, name, provider_id, draft_json, created_at)
tasks       (id, session_id, provider_id, protocol, request_json, status,
             error, concurrency, created_at, finished_at)  -- session_id REFERENCES sessions(id) ON DELETE SET NULL
images      (id, task_id, path, thumb_path, prompt, model, params_json,
             favorite, width, height, size, created_at)    -- task_id REFERENCES tasks(id) ON DELETE SET NULL（可空）
prompts     (id, title, content, tags)
```

- 外键是真声明的（不是只开 `PRAGMA foreign_keys`）：`tasks.session_id` → `sessions(id)`、
  `images.task_id` → `tasks(id)`，都是 `ON DELETE SET NULL`。
  删任务/删会话**不删**图片与任务本身——画廊里的图不该因为清任务队列就没了，
  只是不再声称自己属于某个已删的任务。
- 旧库没有 REFERENCES，迁移由 `storage::migrate_foreign_keys` 重建表完成
  （SQLite 不能 ALTER TABLE 加约束）：重建在一个事务里，孤儿引用置 NULL，数据零丢失。
  不想开 GUI 也能升级库结构：`cargo run --example migrate_db -- "<db 路径>"`。
- 索引：`idx_images_created` / `idx_tasks_created`（列表排序）、
  `idx_images_task`（`task_list` 回填图片、`count_done` 计数）、
  `idx_tasks_session`（外键的 ON DELETE SET NULL 要扫子表）。

## 8. 目录结构

```
xiic-image-studio/
├── src-tauri/src/
│   ├── lib.rs / main.rs           # tauri 入口、命令注册、窗口上屏/跨 Space
│   ├── provider/{client,commands,db,mod,types}.rs   # 四协议 + 模型发现 + 测试
│   ├── generate/{adapters,client,commands,mod,types}.rs  # 协议分发 + 并发队列 + 重试/取消
│   ├── mcp.rs / mcp_protocol.rs   # --mcp 无头 stdio 模式
│   ├── media.rs                   # ffmpeg sidecar + 缩略图
│   ├── crypto.rs                  # AES-256-GCM 本地密钥
│   ├── capabilities.rs            # 协议 × 模型能力表（尺寸/质量/seed/参考图/蒙版/端点），唯一事实源
│   ├── {storage,sessions,gallery,prompts,error}.rs
├── src/
│   ├── views/{Workbench,Gallery,Providers,Prompts,Tools}View.tsx
│   ├── components/{TaskPanel,MaskEditor,ProviderForm,Select,TrafficLights}.tsx
│   ├── stores/{prefill,providers}.ts
│   ├── lib/{submission,mask-geometry,capabilities,batch-parse}.ts + 同名 .test.ts  # 纯函数（提交语义 / 蒙版几何 / 能力收敛 / 批量解析），node --test
│   └── ipc.ts
├── src-tauri/examples/{gen_icon,migrate_db}.rs  # 不依赖 GUI 的出图自检 / 库结构迁移
└── scripts/fetch-ffmpeg.sh        # 打包前拉 ffmpeg sidecar（GitHub Release tag ffmpeg-v1）
```

## 9. 里程碑

| 里程碑 | 内容 | 验收 |
|---|---|---|
| **M1 骨架** | Tauri 工程 + SQLite + 布局 + Provider 管理（四协议 + 测试 + 发现） | 能配通一个中转站 |
| **M2 生图核心** | 文生图/图生图 + 任务队列 + 本地落盘 | 中转站真实出图 |
| **M3 画廊与工作流** | 画廊/搜索/收藏/参数回填 + 多会话 + 提示词库 | 完整日常可用 |
| **M4 进阶** | 蒙版重绘 + ffmpeg 工具箱 + 批量任务 + MJ U/V | 功能齐（批量为显式开关；文件导入按扩展名判格式，CSV 支持引号字段） |
| **M5 无头 MCP** | `--mcp` 纯 stdio，对外 `generate_image` / `list_providers` | 外部 agent 能驱动生图、结果进同一画廊 |

## 10. MCP 无头模式（已实现）

- 形态：`xiic-image-studio --mcp`，纯 stdio（无 HTTP / SSE / socket / daemon、零端口非常驻）。
- 入口最简：直连 `src-tauri/target/release/xiic-image-studio`（MCP 配置 `command` 指向该二进制，`args: ["--mcp"]`）。
  `cargo clean` 后需 `cargo build --release` 恢复。
- 能力必须是 studio 自己的：GUI 的 `run_one` 与无头 `mcp.rs` 共用 `run_one_core` / `create_task` /
  `finish_task_row`（同库、同 adapters、同落库规则、同 images 目录）。**不在外部 crate 复制 DB schema / 解密 / adapters**，
  也**不开 unix socket / HTTP 端口**做 bridge。
- 无头进程与 GUI 是两个进程、写同一张 SQLite：`~/Library/Application Support/com.xiic.image-studio/image-studio.db`
  （无头侧靠 `dirs::data_dir()` + `com.xiic.image-studio` 定位，须与 tauri `app_data_dir()` 一致）。
- 写库三条硬约束：① 图片必须落在 `<data_dir>/images/<task_id>/`（否则 `image_read_b64` 越界校验拒绝）；
  ② 必须写 `tasks`（status running→completed/failed）与 `images`（含 thumb_path），工作台才可见；
  ③ `busy_timeout(10s)` 由 `storage::init()` 统一设置（GUI 与无头共用同一条约束；WAL 只解读写冲突，不解写写冲突）。
- 前端对无头写入的任务**靠轮询发现**（TaskPanel 3s / Gallery 5s，带指纹比对避免重渲染）。

## 11. 关键决策记录

- **Tauri 2 而非 Electron**：体积/内存优势 10 倍级，且团队有成熟经验
- **协议适配器模式**：新增中转形态 = 加一个 adapter，不动 UI
- **Key 本地加密存储（非 Keychain）**：AES-256-GCM + 机器绑定(IOPlatformUUID)做混淆级保护，不进 SQLite 明文；dev 裸 binary 签名会变，用 Keychain 会反复弹授权框
- **图片落盘 + 元数据进库**：库坏了好恢复，目录可以直接当相册用
- **ffmpeg 做 sidecar 而非编译进 Rust**：升级独立、体积可控；打包前由 `scripts/fetch-ffmpeg.sh` 从
  GitHub Release（tag `ffmpeg-v1`）拉取自包含二进制到 `src-tauri/binaries/`，**不进 git**（43.5MB）
- **库写失败一律冒泡，启动期例外只打 stderr**：图片入库/任务状态写不回会静默丢数据，必须报错；
  但 `setup` 里 panic 会让从 Finder 启动的用户只看到"双击没反应"，所以启动两项（遗留任务清理、
  明文 Key 加密回写）显式打日志后继续，后续每一次写库都会硬报错来暴露库不可用。
- **能力表而非逐处打补丁**：协议差异多且会变（中转站、模型版本），散落在适配器里迟早再次不一致。
  收敛成一张 `(protocol, model) → Capabilities` 的纯函数表，适配器、提交入口、前端读同一份，
  "协议不支持的参数"就只有一个地方能定义。
- **真外键而不是只开 PRAGMA**：`PRAGMA foreign_keys = ON` 在没声明 REFERENCES 的表上不起作用，
  等于给了一层假的安全感。引用完整性要么真声明（代价是旧库要重建表迁移），
  要么就承认没有——不能两头占。选 `ON DELETE SET NULL` 而不是级联删：
  画廊里的图不该因为清任务队列就消失，删任务只清空归属。
- **锁中毒不 panic**：某条命令 panic 时若正持着库锁，`Mutex` 会被毒化，
  之后所有命令连锁 panic——一次崩溃放大成整条链路失效。
  `Db::conn()` 在毒化时取回锁内连接继续用：毒化只说明上次崩过，不代表数据坏了。

## 12. 待办 / 已知限制

- **ffmpeg webp**：macOS 上 brew 版 ffmpeg 默认不带 libwebp，选 webp 会报错；打包用的 sidecar 自带 webp。
- **seed 兼容性**：是否下发由能力表决定（openai_chat_image / gemini_native 下发，openai_images 不下发）；
  部分中转站无视该字段属正常。
- **批量任务导出**：目前只支持导入，尚未做「把画廊/任务导出成 JSONL」的反向流水线。
- **批量开关与蒙版涂抹的 GUI 手工验证**：底层纯函数与单测已覆盖（提交语义、蒙版几何换算），
  但 UI 接线还没有人工点过一遍（见 `docs/superpowers/plans/2026-10-31-p0-3-p0-5-fixes.md` 里
  仍未勾选的两步）。
