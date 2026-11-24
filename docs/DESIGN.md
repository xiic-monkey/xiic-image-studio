# xiic-image-studio 设计文档

> 版本 v0.5 · 2026-11-22 · 状态：MVP + Phase 2 主要功能已实现（含 CLI 无头模式、GUI/CLI/MJ 统一编排层、画廊导出与种子扫描）

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
  protocol: OpenAIImages | OpenAIChatImage | GeminiNative | MidjourneyProxy
         | VolcengineArk | DashScope,
  model, extra_headers, custom_model_name   // 预设协议 + 手动覆盖模型名
}
```

十套协议适配器（对标 mj-studio，覆盖中转站全部主流形态）：

| 协议 | 端点 | 典型模型 |
|---|---|---|
| **OpenAIImages** | `POST {base}/v1/images/generations` / `edits` | gpt-image-1/2、dall-e-3、Flux、seedream |
| **OpenAIChatImage** | `POST {base}/v1/chat/completions`（回复中解析图片 URL/b64） | gpt-4o-image、Grok image、通义万相、nano-banana |
| **GeminiNative** | `POST {base}/v1beta/models/{m}:generateContent` | gemini-2.5-flash-image |
| **MidjourneyProxy** | `/mj/submit/imagine`、`/mj/task/{id}/fetch`（轮询） | midjourney 系（含垫图、U/V 操作） |
| **VolcengineArk** | `POST {base}/api/v3/images/generations`（同步，OpenAI 兼容） | 豆包 Seedream 4/5、即梦；国内直连免梯子 |
| **DashScope** | wan2.x：`POST {base}/api/v1/services/aigc/text2image/image-synthesis` + `GET /api/v1/tasks/{id}`（异步轮询）；qwen-image-3.x：`POST {base}/api/v1/services/aigc/multimodal-generation/generation`（同步，图在 `output.choices[]`） | 通义万相 wan2.x、Qwen-Image 3.0（支持参考图）；国内直连 |
| **Stability** | `POST {base}/v2beta/stable-image/generate/{model}`（multipart，Accept 头切换字节/JSON） | Stable Image Core/Ultra、SD3.5；尺寸是画面比例，seed 是标准参数 |
| **Ideogram** | `POST {base}/v1/{model}/generate`（Api-Key 头，提示词字段叫 text_prompt） | Ideogram 4/3；文字排版最强；模型名在路径里 |
| **FluxBfl** | `POST {base}/v1/{model}` + 轮询 `polling_url`（x-key 头，宽高分开传） | FLUX.2 pro/max/flex/klein；结果 URL 十分钟过期 |
| **Minimax** | `POST {base}/v1/wand/minimax-image/generation`，带参考图改走 `/edits` | MiniMax H3-Image；国内可用，支持多图参考编辑 |

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
6. **单会话工作台**：草稿自动保存（prompt、模型、参数）；任务队列与画廊共用「加入参考图 / 继续生成 / 复制图片 / 在 Finder 打开」右键菜单
7. **提示词库**：模板 CRUD、检索、一键套用

### Phase 2（M4+，已实现）
8. **局部重绘**：Canvas 蒙版涂抹 → images/edits（仅 OpenAI Images 协议精确蒙版，其他协议退化为普通图生图）
9. **图片工具箱**（ffmpeg sidecar）：webp/avif/png 转换、批量压缩、GIF 合成、视频抽帧做参考图、改尺寸
10. **批量任务**：工作台「按行批量」开关（默认关）——开着时一行一个任务、每行 1 张；
    关着时整段多行文本按**一条**提示词提交，`数量`生效。换行本身不再隐含批量，
    模板追加因此能把多段描述合成一条。另有 **JSONL/CSV 文件导入**（"导入批量"按钮）：
    按文件扩展名判格式（.csv / .jsonl / .txt，认不出扩展名时看内容），
    CSV 走标准引号规则（引号内的逗号不算分隔符、`""` 是转义），纯文本整行即一条提示词。
11. **MJ 扩展操作**：U/V 放大变换按钮（走 `pipeline::prepare_task` + `spawn_task`，与 GUI 生成同链路：
    有进度、可取消、受全局并发闸门约束；`out.meta.mj_task_id` 保留链式 U/V）
12. **提示词模板库**：工作台「模板」按钮打开模态模板库，左栏按分类挑模板、右栏填字段并实时预览合成结果。
    内置 49 个模板、7 大类（角色设计 / 人物摄影 / 电商产品 / 插画风格 / 场景分镜 / 设计品牌 / 社媒物料），
    结构按业内标准工作流编排（详见 §6.1）。

### Phase 3（M5+，已实现）
13. **CLI 无头模式**：`xiic-image-studio generate|providers|export`（不占端口、非常驻），对外暴露
    `generate`（含 `--ref` 参考图）、`providers` 与 `export`（导出画廊 JSONL）。
    与 GUI 共用 `pipeline::prepare_task`（唯一建任务行）和 `run_task_headless`，同库、同 adapters、
    同落库规则、同 `images/` 目录；前端靠轮询发现无头进程写入的任务。详见 §10。
14. **画廊导出 JSONL**：GUI「导出 JSONL」按钮与 CLI `export` 子命令共用
    `gallery::query_images` + `export_rows` + `write_jsonl`。导出字段（prompt/size/quality/seed）
    与「导入批量」的 `BatchItem` 对齐，**导出的文件能原样导回再跑一遍**——导入与导出构成闭环，
    不是单向存档。
15. **种子扫描**：工作台「种子扫描」开关。开启后一条提示词按连续种子拆成多个任务
    （每任务 n=1、seed 逐个 +1），用于探索同一提示词在不同 seed 下的差异。
    模型不支持 seed 时**明确拒绝**（提交入口按能力表判定），不静默退化成「同一提示词随机出 N 张」。
16. **配方预设**：提示词 + 尺寸 + 质量 + 种子 + 供应商的组合存成可复用配方。
    画廊右键「存为配方」从一张成功的图直接存参数组合，工作台「配方」面板一键套用。
    与 `prompts` 表分开存：提示词库存文本，配方存的是「文本 + 能让它复现的参数」。

### Phase 4（M6+，已实现）
17. **供应商 × 模型多对多**：供应商=连接（url/key/headers），模型=一等实体（名+协议），
    通过 `provider_models` 自由组合——同一模型可挂多家供应商，同一供应商可挂多协议的多个模型。
    协议归模型（不是供应商），这正是"支持更多模型协议"的结构基础：加新协议只需加枚举+适配器+能力表一行。
    生成请求带 `model_id`，`model::db::resolve` 统一解析（GUI/CLI 共用），capabilities 按所选模型算；
    工作台拆成「供应商 / 模型」两级下拉，供应商页新增模型库与每家供应商的挂模型/设默认/解绑。
    旧库自动迁移、配方也记 `model_id`。详见 §7。

### 参数说明（已对齐实现）
- **seed（随机种子）**：由「协议 × 模型能力表」（`src-tauri/src/capabilities.rs`）决定是否下发。openai_chat_image 走请求体顶层（Chat Completions 标准参数）；gemini_native 走 `generationConfig.seed`；**openai_images 不下发**（Images API 规范里没有该字段，官方与转 OpenAI 的中转站都会 400）。上游不支持时静默忽略，不影响其他参数。
- **n（数量）**：前端「数量」选择器 → `pipeline::execute` 按 `0..n` 并发 `buffer_unordered(FANOUT=4)`
  各生成一张，任务 `total=n`。上限由 `MAX_N=8` 统一裁剪（GUI 与 CLI 共用同一上限）。
- **重试**：仅 `ref_count==0` 且状态为 `failed` / `canceled` / `completed_with_errors` 的任务提供重试入口；
  图生图因参考图未入库，重试会退化，前端据此隐藏入口。重试经 `retryRequest` 复现
  `size` / `quality` / `seed` / `n` 与任务归属（`session_id` 仅作归属信息、不作可见性过滤），
  旧数据无归属时回落到当前草稿会话。
- **能力表（协议 × 模型）**：`for_model(protocol, model)` 推导该组合实际接受的参数
  （尺寸取值 / 质量取值 / 是否支持 seed、参考图、蒙版 / 上游端点形态）。
  适配器据此发参，GUI 与 CLI 提交入口据此裁剪请求，前端据此收敛控件并把不支持项置灰。
  能力来源三层，后者覆盖前者：**协议默认值 → 内置模型规则表（`MODEL_RULES`，按模型名前缀匹配，
  加模型只加一行数据不改控制流）→ 用户对单个模型的 DB 级覆写（`models.capabilities_json`，
  给中转站这类非标准尺寸用；非法 JSON 保存时报错，不静默丢弃）**。
  能力随请求解析结果一路带进 `GenCtx.caps`，适配器不再自己算第二遍——两处各算一次迟早漂移。
  模型名匹配不到时走宽松兜底行（宁可上游报错，也不静默吞掉用户设的参数）。
  Imagen（`imagen-*`）走 `:predict` 端点分支。

## 6. 架构

```
┌─ 前端 React (WebView) ─────────────────────────┐
│  工作台（hooks + workbench 子组件）│ 任务队列 │ 画廊 │ 供应商 │
└──────────────┬──────────────────────────────────┘
               │ Tauri IPC (invoke / event)
┌─ Rust 核心 ──┴──────────────────────────────────┐
│ provider 模块   十套协议适配器 + 模型发现        │
│ generate 模块                                    │
│   ├─ pipeline.rs  ← 唯一编排层（prepare_task /   │
│   │                 execute / spawn_task /       │
│   │                 run_task_headless）           │
│   ├─ adapters.rs  十协议 + MJ action 出图        │
│   └─ mod.rs       run_one_core（唯一出图落库叶子）│
│ storage 模块    rusqlite (任务/会话/图片元数据)  │
│ media 模块      image 缩略图 ｜ ffmpeg sidecar  │
│ crypto 模块     AES-256-GCM 本地密钥             │
│ cli 模块        generate / providers 无头模式   │
└─────────────────────────────────────────────────┘
产出 → <data_dir>/images/<task_id>/  本地落盘

三条入口收敛到同一条链路：
  GUI  generate_submit ─┐
  CLI  generate ─────────┼─> capabilities::sanitize → build_request_json
  MJ   mj_action ───────┘        → pipeline::prepare_task（唯一建任务行）
                                  → pipeline::execute
                                    ├─ RunMode::Gui      emit 进度 / 取消 / 并发闸门 / finish_task
                                    └─ RunMode::Headless 静默 / 无闸门 / finish_task_row
                                  → mod.rs::run_one_core → adapters 出图 → 落盘 + 落库

无头模式与 GUI 是「两个进程、同一张库、同一个 images 目录」：
  xiic-image-studio generate  ←→  SQLite  ←→  GUI（前端轮询发现新任务）
```

### 6.1 提示词模板体系

模板不是「一段固定提示词」，而是**结构 + 槽位**：选模板 → 填字段 → 合成提示词 → 插入工作台。

```
PromptTemplate {
  id / title / category / summary
  fields: [{ key, label, hint, default, required, multiline }]   // 槽位
  prompt: "…{{character}}…{{style}}…"                            // 骨架
  ratio?: "16:9"                                                 // 建议比例（不写死像素）
  steps?: [{ title, prompt, note }]                              // 分步工作流
  needsRef?: boolean                                             // 是否建议配合参考图
}
```

合成规则（`src/lib/template.ts`，纯函数 + 单测）：

- **必填校验**：必填槽位没填不给插入，界面直接标出「还差：角色形象」。
- **空槽整段省略**：可选槽位留空时，「附加：。」这类残渣整句丢掉，提示词里不留脏标点。
- **比例落地**：模板只声明 `ratio`，`pickSizeForRatio` 按当前模型能力表挑最接近的合法尺寸
  （16:9 → 1536x1024 这类），换供应商/模型也不会发出上游不认的 size；MJ 协议没有 size 字段，
  改为在提示词尾部追加 `--ar 16:9`。

内置模板按业内标准工作流编排（参考 PixAI 角色设定指南、PromptMake turnaround 手册、
Apatero 三视图指南、Google 官方生图最佳实践、Midjourney `--cref/--cw` 用法）：

| 分类 | 模板 |
|---|---|
| 角色设计 | 定案立绘 / 三视图 / 短剧设定图 / 四视图 / 单视角 / 表情差分 / 服装设定 / 配色卡 / 道具设定 / 场景延展 / 资产全套工作流 |
| 人物摄影 | 写实特写 / 全身人像 / 杂志大片 / 街头纪实 / 证件照 / 宠物写真 |
| 电商产品 | 白底主图 / 场景带货 / 模特上身 / 细节特写 / 美食摄影 |
| 插画风格 | 二次元立绘 / 扁平插画 / 水彩绘本 / 游戏概念 / 动画电影 / 3D 动画 / 粘土定格 / 盲盒潮玩 / 像素 / 等距微缩 / 铅笔素描 / 赛博朋克 |
| 场景分镜 | 场景概念 / 分镜故事板 / 影视主视觉 / 室内效果图 / 建筑外观 |
| 设计品牌 | Logo / 海报 / UI 界面稿 / 包装设计 / 印花周边 / 专辑封面 |
| 社媒物料 | 小红书封面 / 视频缩略图 / 手机壁纸 / 贴纸包 |

模板面板的版式是「筛选条 + 内容区」：顶栏（标题/关闭）→ 整行搜索 → 整条分类标签
（横向滚动、每个标签带模板数）→ 左列表 / 右详情。搜索跨分类，搜索时分类高亮收回到「全部」；
「全部」视图下左列表按分类分组显示组头，选定分类后组头隐藏。分类标签不再塞进左栏，
模板变多后左栏只承担列表，不重复承担导航。

在线模板库调研（2026-11）：生图领域没有统一的模板开放协议或稳定 API。GitHub 上有
AI-Visual-Prompt-Cookbook（MIT，约 150 个 style.json）、awesome-gpt-image-2（MIT，544 个
逆向案例）、BoltPrompt（8000+ 条、可自部署）等 JSON 数据集，但 schema 各自为政、无法直接对接，
因此当前以内置模板为主，不接远程源。

三视图类模板写死业内三条硬约束：**冻结姿势**（A-pose，无动作、无透视缩短）、
**压平光线**（均匀平光，保证服装结构可读）、**锁定比例**（头身比 + 头顶/下巴/肩/腰/膝同一条水平基准线）；
正侧面写「只露一只眼睛」、背面写「不露脸」，这是三视图最容易崩的两个点。

短剧 / AI 漫剧用另一套通行版式（`char-drama-sheet`）：**面部特写 + 正/侧/背三张全身站立**，
共四格横排——特写格锁五官辨识度，三张全身保形体与服装一致。与动画/游戏的三格 turnaround
不是一回事，因此单独出一条模板，原三视图模板保留给需要纯 turnaround 的场景。

## 7. 数据模型（SQLite）

```sql
providers       (id, name, base_url, protocol, model, custom_headers, created_at)  -- 连接；key 只存库内（AES-GCM）
-- protocol/model 两列保留作迁移来源与回退兜底，代码不再用它们派生生成参数
models          (id, protocol, model_name, label, created_at)  -- 一等模型；UNIQUE(protocol, model_name)
provider_models (provider_id, model_id, is_default, created_at) -- 多对多关联；PK(provider_id, model_id)
                -- provider_id REFERENCES providers(id) ON DELETE CASCADE
                -- model_id REFERENCES models(id) ON DELETE CASCADE
sessions        (id, name, provider_id, draft_json, created_at)
tasks           (id, session_id, provider_id, protocol, request_json, status,
                 error, concurrency, total, created_at, finished_at)  -- session_id REFERENCES sessions(id) ON DELETE SET NULL
images          (id, task_id, path, thumb_path, prompt, model, params_json,
                 favorite, width, height, size, created_at)    -- task_id REFERENCES tasks(id) ON DELETE SET NULL（可空）
prompts         (id, title, content, tags)
presets         (id, name, prompt, size, quality, seed, provider_id, model_id, created_at)  -- 生成配方
meta            (key, value)  -- 迁移标记（provider_models_migrated）
```

- **供应商 × 模型 多对多**：供应商只是连接（url/key/headers），模型是一等实体且**协议归模型**；
  同一 `(protocol, model_name)` 全局唯一、可被多家供应商复用，同一供应商也能挂不同协议的模型。
  生成请求带 `model_id`，后端 `model::db::resolve` 解析出 `(protocol, model_name)` 与能力，
  不再从 `provider.protocol/model` 隐式派生。能力仍由 `capabilities::for_model` 动态算，不入库。
- 旧库迁移 `storage::migrate_provider_models`：把每个旧 provider 的 `(protocol, model)` 拆成
  models + provider_models，按 UNIQUE 去重、空模型建兜底行；用 `meta` 表的一次性标记做幂等，
  跑完即使用户解绑了全部关联也不会在下次启动被重新挂回。
- `presets.model_id`：配方除供应商外也记模型，套用时能恢复模型选择。

- 外键是真声明的（不是只开 `PRAGMA foreign_keys`）：`tasks.session_id` → `sessions(id)`、
  `images.task_id` → `tasks(id)`，都是 `ON DELETE SET NULL`。
  删任务/删会话**不删**图片与任务本身——画廊里的图不该因为清任务队列就没了，
  只是不再声称自己属于某个已删的任务。
- `tasks.session_id` **真实写入**：GUI 提交与 MJ U/V（继承父任务）带当前会话 id，
  `pipeline::prepare_task` 把它交给 `create_task` 落库；CLI 与外部 examples 传 `NULL`。
  删会话后该列被 `ON DELETE SET NULL` 清空，任务仍在。`task_list` **不按 session_id 过滤**——
  否则 CLI 任务与历史任务会从工作台消失。`TaskItem` 同时回传 `session_id` / `seed`，
  一键重试据此复现原会话归属与随机种子。
- `request_json` 由 `build_request_json` 唯一构造，记 7 个键：`prompt/n/size/quality/seed/ref_count/model_id`；
  `model_id` 供一键重试复原到具体模型。MJ U/V 用 `build_mj_action_request_json`
  （`action` + `mj_task_id`，无 prompt → 前端不给重试入口）。
  `task_list` 的解析抽成 `parse_request_json`，对旧数据与坏 JSON 做兼容。
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
│   ├── provider/{client,commands,db,mod,types}.rs   # 十协议连接 + 模型发现 + 测试
│   ├── model/{commands,db,mod,types}.rs            # 模型实体 + 供应商×模型关联（多对多）
│   ├── generate/{adapters,client,commands,mod,types,pipeline}.rs
│   │       # pipeline = 唯一编排层；mod = 底层原语；adapters = 十协议 + MJ action
│   ├── cli.rs                     # generate / export / providers 无头模式
│   ├── media.rs                   # ffmpeg sidecar + 缩略图
│   ├── crypto.rs                  # AES-256-GCM 本地密钥
│   ├── capabilities.rs            # 协议 × 模型能力表（尺寸/质量/seed/参考图/蒙版/端点），唯一事实源
│   │                              # 协议默认 + MODEL_RULES 规则表 + 用户覆写三层
│   ├── {storage,sessions,gallery,prompts,presets,error}.rs
├── src/lib/protocols.ts           # 协议常量 + 供应商模板（后端加协议要同步这里）
├── src/
│   ├── views/{Workbench,Gallery,Providers,Prompts,Tools}View.tsx
│   ├── views/workbench/{RefImages,ParamRow,SubmitBar,TemplatePanel,PresetPanel,
│   │       MaskEditorLayer,styles}.tsx/.ts  # 工作台展示层
│   ├── hooks/{useTaskFeed,useDraft,useSubmissions,useAddAsRef}.ts  # 轮询/草稿/提交/加参考图副作用
│   ├── components/{TaskPanel,MaskEditor,ProviderForm,Select,TrafficLights}.tsx
│   ├── ui/controls.ts               # 全局控件规格：单行控件统一 30px，字号分常规/紧凑两档
│   ├── ui/ContextMenu.tsx           # 画廊与任务队列共用的右键菜单
│   ├── stores/{prefill,providers,models}.ts
│   ├── lib/{submission,request,draft,retry,task-signature,mask-geometry,
│   │       capabilities,batch-parse,ref-image,template}.ts + 同名 .test.ts
│   │       # 纯函数：提交语义 / 请求组装 / 草稿指纹 / 重试 / 列表指纹 / 蒙版几何 /
│   │       # 能力收敛 / 批量解析 / 参考图转换 / 模板合成，node --test
│   ├── templates/                   # 内置提示词模板库（按分类分文件 + index 汇总）
│   │   ├── {index,character,portrait,commerce,illustration,scene,design}.ts
│   └── ipc.ts / types.ts
├── src-tauri/examples/{gen_icon,migrate_db}.rs  # 不依赖 GUI 的出图自检 / 库结构迁移
└── scripts/fetch-ffmpeg.sh        # 打包前拉 ffmpeg sidecar（GitHub Release tag ffmpeg-v1）
```

## 9. 里程碑

| 里程碑 | 内容 | 验收 |
|---|---|---|
| **M1 骨架** | Tauri 工程 + SQLite + 布局 + Provider 管理（四协议 + 测试 + 发现） | 能配通一个中转站 |
| **M2 生图核心** | 文生图/图生图 + 任务队列 + 本地落盘 | 中转站真实出图 |
| **M3 画廊与工作流** | 画廊/搜索/收藏/参数回填 + 继续生成/加入参考图 + 提示词库 | 完整日常可用 |
| **M4 进阶** | 蒙版重绘 + ffmpeg 工具箱 + 批量任务 + MJ U/V | 功能齐（批量为显式开关；文件导入按扩展名判格式，CSV 支持引号字段） |
| **M5 无头 CLI** | `generate` / `providers` 子命令，对外 `--prompt` / `--ref` / `--json` | 外部 agent 能驱动生图（含以图生图）、结果进同一画廊 |
| **M6 统一编排层** | `pipeline.rs` 收编 GUI / CLI / MJ 三条链路；接通 `session_id`；拆工作台、修轮询与草稿竞态；工程化收尾（fmt / clippy / CI / CSP） | 三条入口共用一套建任务与执行语义，`cargo test --lib` / `npm test` / `npm run build` 全绿 |

## 10. CLI 无头模式（已实现）

- 形态：`xiic-image-studio generate` / `xiic-image-studio export` / `xiic-image-studio providers`，
  无 HTTP / SSE / socket / daemon、零端口非常驻，跑完即退。
- 入口最简：直连 `src-tauri/target/release/xiic-image-studio`（skill 的 `scripts/xiic.js`
  按 `XIIC_IMAGE_STUDIO_BIN` → PATH → release 目录 → `/Applications/...app` 顺序找它）。
  `cargo clean` 后需 `cargo build --release` 恢复。
- 模型选择：`--model <模型名|id>` 指定用该供应商下的哪个模型，不传用默认模型。
  GUI 与 CLI 都走 `model::db::resolve` 解析出 `(protocol, model_name)` 与能力，
  协议/模型不再从供应商旧列派生；`request_json.model_id` 供重试复原。
  指定未关联的模型直接报错，绝不静默改用别的模型。
- 能力必须是 studio 自己的：`cli.rs::generate` 与 GUI 的 `generate_submit` 走同一套
  `model::db::resolve` → `capabilities::sanitize` → `build_request_json` → `pipeline::prepare_task`（唯一建任务行）；
  执行侧 GUI 走 `spawn_task` / `execute(RunMode::Gui)`，无头走 `run_task_headless` /
  `execute(RunMode::Headless)`，两边最终都落到 `mod.rs::run_one_core`（同库、同 adapters、
  同落库规则、同 images 目录）。**不在外部 crate 复制 DB schema / 解密 / adapters**，
  也**不开 unix socket / HTTP 端口**做 bridge。
- `RunMode::Headless` 在类型上拿不到 `AppHandle`，误 emit 进度直接编译失败。
  CLI 不 emit 进度（前端 3s 轮询发现）、不参与 GUI 进程内的 Semaphore（跨进程本就不共享）、
  也没有取消入口——这三条是**有意保留的差异**，不是没对齐。
- 输出契约：stdout 只出结果（默认人类可读，`--json` 出结构化），错误一律 stderr + 非零退出码，
  便于脚本与管道消费。`--json` 结构为 `task_id/status/count/images[{id,path,thumb_path}]`。
- 参考图（以图生图）：`--ref <path>` 可重复，最多 4 张（与前端 `MAX_REFS` 一致），
  单张上限 16MB，格式限 png/jpg/jpeg/webp/gif。**CLI 入口按能力表自行拦截**：
  `caps.refs=false` 的模型（如 dall-e-3、Imagen）带图直接报错，不静默丢弃；
  后端 `sanitize()` 也会兜底裁掉 refs，避免手写请求把图发到不支持的端点。
- 无头进程与 GUI 是两个进程、写同一张 SQLite：`~/Library/Application Support/com.xiic.image-studio/image-studio.db`
  （无头侧靠 `dirs::data_dir()` + `com.xiic.image-studio` 定位，须与 tauri `app_data_dir()` 一致）。
- 写库三条硬约束：① 图片必须落在 `<data_dir>/images/<task_id>/`（否则 `image_read_b64` 越界校验拒绝）；
  ② 必须写 `tasks`（status running→completed/failed）与 `images`（含 thumb_path），工作台才可见；
  ③ `busy_timeout(10s)` 由 `storage::init()` 统一设置（GUI 与无头共用同一条约束；WAL 只解读写冲突，不解写写冲突）。
- 前端对无头写入的任务**靠轮询发现**（`useTaskFeed` 3s / Gallery 5s）。指纹由
  `lib/task-signature.ts` 统一计算，含 `errors.length` / `seed` / `sessionId`，
  没变就不 setState；监听注册是异步的，组件卸载时若 `unlisten` 还没返回，
  画廊里的图不该因为清任务队列就消失，删任务只清空归属。
- **对外形态**：仓库内的 `docs/skill/xiic-image-studio/` 提供一份 agent 可读的 skill
  （`SKILL.md` + `references/` + `scripts/xiic.js`），按渐进式披露组织：
  SKILL.md 只留触发条件与最简流程，参数细节下沉到 references/，脚本负责定位二进制并调 CLI。
  该 skill 与具体 agent 无关，可被任何 agent 装载；仓库内不安装到任何 skills 目录。
- **MCP 已移除**：早期的 `--mcp` stdio 模式（`mcp.rs` / `mcp_protocol.rs`）已删除，
  二进制不再识别该参数（传入会明确报错提示改用 CLI 子命令）。
  迁移：`--mcp` → `generate` 子命令，工具 `generate_image` → `generate --prompt`，
  工具 `list_providers` → `providers`。

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
- **共享编排层收编三条入口**：GUI `generate_submit`、CLI `generate`、MJ `mj_action`
  过去各自做参数裁剪、建任务行、并发与收尾，规则会逐条漂移（MJ U/V 甚至绕过进度、
  取消与全局并发闸门）。现在统一到 `pipeline.rs`：`prepare_task` 是唯一建任务行，
  `execute` 是唯一编排体，`run_one_core` 是唯一出图落库叶子。`ImageProducer`
  把「按协议出图」与「MJ action 出图」收成同一个枚举，重试判定也跟着走同一条规则。
- **接通 `session_id` 但不过滤**：`tasks.session_id` 以前恒为 NULL（死列）。现在 GUI /
  MJ 写真实会话、CLI 写 NULL，删会话靠外键置 NULL；`task_list` 仍返回全部任务，
  否则 CLI 任务与历史任务会从工作台消失。归属信息不是「可见性开关」；前端收敛为单会话后，
  sessions 表继续作为唯一草稿槽，历史会话记录留在库里以保住老任务的归属。
- **CLI 的静默进度与跨进程闸门是有意差异**：无头进程没有 `AppHandle`，也和 GUI
  不共享进程内 Semaphore，强行对齐只会引入假象。能对齐的（参数裁剪、建任务、出图、
  落盘、落库、状态收尾）已经全部对齐。
- **草稿用内容指纹而不是单布尔开关**：`skipSave` 那种一次性布尔会被任何一次无关
  重渲染消费掉。现在 `useDraft` 以「已落库内容指纹」为基线做 debounce 保存，
  组件卸载时也 flush，丢不掉最后 800ms 的编辑。单会话下只加载一次草稿；
  `ready` 门控提交与画廊回填，避免 session_id 落 NULL 或异步灌草稿冲掉回填的提示词。
- **前端只在展示层回落能力，不改草稿**：切换供应商后尺寸/质量可能不在新能力表内，
  把回落值写回表单会在加载草稿那一轮覆盖已保存的值；提交时 `buildGenerateRequest`
  统一归一化即可，上游拿到的值始终合法。
- **控件高度全局统一 30px**：以前每个页面各写一份 `inputCls`，靠 `py-1.5` / `py-2` 撑高度，
  结果模型库 36px、工具箱 28px、工作台 30px 混着来，跨页并排看明显不协调。
  现在统一收口到 `src/ui/controls.ts`：单行控件（输入框 / 下拉 / 按钮）一律 `h-[30px]`，
  高度不再随字号变化；字号只分两档（常规 `text-sm`、密集区 `text-xs`），
  多行控件只统一边框与内边距。页面不要再自己拼 `inputCls`，要改就改那一处。

## 12. 待办 / 已知限制

- **供应商 × 模型多对多**：已做。模型是一等实体、协议归模型，关联表自由组合；
  生成请求带 `model_id`，GUI/CLI 共用 `model::db::resolve`；旧库自动迁移。
  已在此基础上新增两种协议（VolcengineArk / DashScope），即"加协议 = 加枚举 + 适配器 + 能力表一行"。
- **供应商模板**：前端内置 12 个常用供应商模板（OpenAI / OpenRouter / SiliconFlow / Gemini /
  火山方舟 / 阿里百炼 / 百炼 OpenAI 兼容模式 / Stability / Ideogram / BFL FLUX / MiniMax / Recraft），
  一键填好名称、baseUrl、协议与常用模型，Key 仍需自填。
- **模型级能力覆写**：模型库每行可展开「能力」，用 JSON 覆写该模型的尺寸/质量/seed 等取值，
  给非标准中转站用。覆写优先于内置规则表；非法 JSON 保存时直接报错。
- **模型版本策略**：同一模型家族只登记**最新两版**（如 gpt-image-2.6 / 2.5、Seedream 5.5 / 5.0、
  Wan 2.7 / 2.6、Qwen-Image 3.0 / 2.1）。能力表按模型名前缀匹配，同协议下新版本自动继承，
  不用为每个版本单独写规则；更老的版本除复现历史任务外没有保留价值。
- **走兼容模式而非专用协议的模型**：Recraft 与 Qwen-Image 都有 OpenAI 兼容端点，直接用
  `openai_images` 协议 + 一条能力规则即可，不必各写一个适配器——协议按**请求形状**划分，
  不是按厂商划分。
- **ffmpeg webp**：macOS 上 brew 版 ffmpeg 默认不带 libwebp，选 webp 会报错；打包用的 sidecar 自带 webp。
- **seed 兼容性**：是否下发由能力表决定（openai_chat_image / gemini_native 下发，openai_images 不下发）；
  部分中转站无视该字段属正常。新增的 volcengine_ark / dashscope 同样不下发 seed。
- **批量任务导出**：已做。GUI「导出 JSONL」与 CLI `export` 共用
  `gallery::query_images` / `export_rows` / `write_jsonl`，导出字段与「导入批量」对齐，
  导出的文件能原样导回再跑一遍。
- **种子扫描的可用面**：只有能力表 `seed=true` 的协议（openai_chat_image / gemini_native）
  能用；openai_images 规范里没有 seed 字段，能力表 `seed=false`，扫描会被明确拒绝。
  也就是说在 gpt-image / dall-e 这类模型上扫描不可用——这是上游限制，不是实现缺口。
- **画廊并排对比 / 批量收藏淘汰**：未做。画廊目前只有收藏与搜索两种筛选。
- **批量导出图片本体 / 工作区备份**：未做。`export` 只导出记录（含原图绝对路径），
  不打包图片文件，也不导出数据库与供应商配置。
- **批量开关与蒙版涂抹的 GUI 手工验证**：底层纯函数与单测已覆盖（提交语义、蒙版几何换算），
  但 UI 接线还没有人工点过一遍（见 `docs/superpowers/plans/2026-10-31-p0-3-p0-5-fixes.md` 里
  仍未勾选的两步）。
- **两处历史不一致已收紧**：`sanitize()` 现在也裁 `refs`（GUI 手写请求不再能把图发到
  `refs=false` 的端点；CLI 仍先显式报错再走裁剪）；`tools_file_b64` 改为复用
  `generate::read_file_b64`，带 `canonicalize()` + 目录越界校验，只认 `app_data/images/tools`
  内的工具箱输出，不再能读任意路径。
- **画廊行解析容忍 NULL**：`width`/`height`/`size`/`thumb_path` 四列可空，
  原实现按 `i64` / `String` 直接取，NULL 会让整行报错、再被 `filter_map(|r| r.ok())`
  静默丢掉——缺尺寸的历史图会在画廊里凭空消失。现在缺值按 0 / 空串处理。
- **前端无 ESLint / Prettier 配置**：目前靠 TypeScript strict + `noUnusedLocals` +
  `node --test` 兜底；`react-hooks/exhaustive-deps` 一类规则还没纳入检查。
- **未做运行时冒烟**：GUI（提交→DB `session_id`、删会话置 NULL、重试保留 seed、MJ U/V 进度、
  dev 控制台 CSP）与 CLI（`cargo build --release` 后出图、GUI 3s 轮询发现、DB 为 NULL）
  两轮人工验收仍需执行。
