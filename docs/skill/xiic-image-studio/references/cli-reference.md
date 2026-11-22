# CLI 参考

`xiic-image-studio` 的无头子命令。脚本 `scripts/xiic.mjs` 是它的薄封装，等价于直接调二进制。

## 子命令

### `generate`

```
xiic-image-studio generate --prompt <提示词> [选项]
```

| 参数 | 说明 |
|---|---|
| `--prompt <文本>` | **必填**。画面描述。空字符串会被拒绝。 |
| `--ref <图片路径>` | 参考图，可重复。最多 4 张，单张 ≤16MB，格式 png/jpg/jpeg/webp/gif。 |
| `--provider <名称\|id>` | 不填则用第一个配好 API Key 的。名称大小写不敏感。 |
| `--model <名称\|id>` | 用该供应商下的哪个模型（模型名或 id）。不传用该供应商的默认模型。指定未关联的模型会报错，不会静默换模型。 |
| `-n <1..8>` | 生成张数，默认 1。越界自动收敛到 [1, 8]。也接受 `--n` / `--count`。 |
| `--size <1024x1024>` | 分辨率。**按模型能力表裁剪**：不支持的取值会被静默丢弃。 |
| `--quality <high>` | 质量档。同样按能力表裁剪。 |
| `--seed <整数>` | 随机种子。**按模型能力表裁剪**：openai_images 规范里没有 seed 字段，会被丢弃。 |
| `--json` | 以 JSON 输出，便于脚本消费。 |

也支持 `--flag=value` 内联写法（`--prompt=猫`）。

### `providers`

```
xiic-image-studio providers [--json]
```

列出已配置的 provider：名称、协议、模型、是否已配 Key。`--json` 时输出完整
`ProviderListItem`（含 `capabilities` 能力表与 `model_ids` 关联的模型 id 列表）。

供应商与模型是多对多：供应商只是连接（url/key），模型带协议，一家供应商可挂多个不同协议的模型，
同一模型也能挂多家供应商。`--model` 指定实际用哪个，协议与能力都从模型解析（不从供应商旧列派生）。

### `export`

```
xiic-image-studio export --out <文件.jsonl> [选项]
```

把画廊里的图导出成 JSONL 记录（提示词 + 实际下发的尺寸/质量/种子 + 原图路径）。

| 参数 | 说明 |
|---|---|
| `--out <文件>` | **必填**。导出目标，必须是 `.jsonl` / `.ndjson` 后缀，目录必须已存在。 |
| `--q <关键词>` | 按提示词模糊过滤，与 GUI 搜索同语义。 |
| `--model <模型名>` | 按模型过滤。 |
| `--fav` | 只导出收藏。也接受 `--favorite` / `--favorites`。 |
| `--limit <条数>` | 最多导出多少条，默认 5000。 |

输出：stdout 一行 `已导出 N 张图的记录到 <路径>`。

**导出文件能被 GUI 的「导入批量」原样读回**——字段名（`prompt` / `size` /
`quality` / `seed`）与批量导入格式一致。所以它不是存档，而是「把这批参数
再批量跑一遍」的往返管道。文件里另有 `model` / `path` / `created_at` /
`favorite` 四个附加字段，导入时会被忽略。

### 其他

```
xiic-image-studio --help      # 用法
xiic-image-studio --version   # 版本号
xiic-image-studio             # 不带参数 = 启动图形界面
```

## 输出契约

`--json` 结构：

```json
{
  "task_id": "abc-123",
  "status": "completed",
  "count": 2,
  "images": [
    { "id": "...", "path": "/Users/.../images/<task_id>/00-xxx.png", "thumb_path": "..." }
  ]
}
```

- `status`：`completed` / `completed_with_errors` / `failed` / `canceled`。
- `count` = 成功出图数，可能小于请求的 `-n`。
- `error`：部分失败时的首个错误文本。
- `warning`：图已入库但任务状态没写回库（图能用，只是任务卡片可能状态不对）。
- **没有** `data_url` 字段。CLI 走本地路径，不把图片本体塞进输出。

## 退出码与流

- stdout 只有结果；stderr 只有错误。
- 失败一律非零退出码（1 = 业务错误，127 = 找不到二进制）。

## 参数裁剪规则

`size` / `quality` / `seed` 由 `capabilities::for_model(protocol, model)` 推导，
**不匹配就丢弃，不报错**。这是刻意的：能力表宁可让上游报错，也不静默吞掉用户设的参数。
想要确认某个 provider 支持什么，跑 `providers --json` 看 `capabilities` 字段。

`refs` 是唯一的例外——CLI 入口会**主动报错**而不是丢弃，因为带图和不带图是完全不同的产物。
详见 `reference-images.md`。
