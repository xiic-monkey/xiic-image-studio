# xiic-image-studio 设计文档

> 版本 v0.1 · 2026-08-29 · 状态：待评审

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

### Phase 2（M4+）
8. **局部重绘**：Canvas 蒙版涂抹 → images/edits
9. **图片工具箱**（ffmpeg）：webp/avif/png 转换、批量压缩、GIF 合成、视频抽帧做参考图、改尺寸
10. **批量任务**：JSONL/CSV 导入批量生图
11. **MJ 扩展操作**：U/V 放大变换按钮
12. 预设模板库（电商/头像/插画等场景模板）

## 6. 架构

```
┌─ 前端 React (WebView) ─────────────────────────┐
│  会话工作台 │ 任务队列面板 │ 画廊 │ 设置/供应商 │
└──────────────┬──────────────────────────────────┘
               │ Tauri IPC (invoke / event)
┌─ Rust 核心 ──┴──────────────────────────────────┐
│ provider 模块   四套协议适配器 + 模型发现        │
│ queue 模块      tokio 并发队列 / 重试 / 取消     │
│ storage 模块    rusqlite (任务/会话/图片元数据)  │
│ media 模块      image 缩略图 ｜ ffmpeg sidecar  │
│ secrets 模块    keyring 系统钥匙串              │
└─────────────────────────────────────────────────┘
产出 → workspace/images/{YYYY-MM-DD}/{task_id}/  本地落盘
```

## 7. 数据模型（SQLite）

```sql
providers   (id, name, base_url, protocol, model, custom_headers, created_at)  -- key 只存钥匙串引用
sessions    (id, name, provider_id, draft_json, created_at)
tasks       (id, session_id, provider_id, protocol, request_json, status,
             error, concurrency, created_at, finished_at)
images      (id, task_id, path, thumb_path, prompt, model, params_json,
             favorite, width, height, size, created_at)
prompts     (id, title, content, tags)
```

## 8. 目录结构

```
xiic-image-studio/
├── src-tauri/src/
│   ├── provider/{openai_images.rs, chat_image.rs, gemini.rs, mj_proxy.rs, mod.rs}
│   ├── queue.rs        # tokio 任务队列
│   ├── storage.rs      # rusqlite
│   ├── media/{image.rs, ffmpeg.rs}
│   └── secrets.rs
├── src/
│   ├── views/{Workbench,Gallery,Providers,PromptLib,Settings}
│   ├── components/{TaskPanel,ImagePreview,MaskEditor,ProviderForm}
│   └── stores/
└── src-tauri/binaries/ffmpeg   # sidecar
```

## 9. 里程碑

| 里程碑 | 内容 | 验收 |
|---|---|---|
| **M1 骨架** | Tauri 工程 + SQLite + 布局 + Provider 管理（四协议 + 测试 + 发现） | 能配通一个中转站 |
| **M2 生图核心** | 文生图/图生图 + 任务队列 + 本地落盘 | 中转站真实出图 |
| **M3 画廊与工作流** | 画廊/搜索/收藏/参数回填 + 多会话 + 提示词库 | 完整日常可用 |
| **M4 进阶** | 蒙版重绘 + ffmpeg 工具箱 + 批量任务 + MJ U/V | 功能齐 |

## 10. 关键决策记录

- **Tauri 2 而非 Electron**：体积/内存优势 10 倍级，且团队有成熟经验
- **协议适配器模式**：新增中转形态 = 加一个 adapter，不动 UI
- **Key 本地加密存储（非 Keychain）**：AES-256-GCM + 机器绑定(IOPlatformUUID)做混淆级保护，不进 SQLite 明文；dev 裸 binary 签名会变，用 Keychain 会反复弹授权框
- **图片落盘 + 元数据进库**：库坏了好恢复，目录可以直接当相册用
- **ffmpeg 做 sidecar 而非编译进 Rust**：升级独立、体积可控
