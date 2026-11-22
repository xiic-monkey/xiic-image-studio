import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  PROTOCOL_LABELS,
  PROTOCOL_PRESET_MODELS,
  PROTOCOL_PRESET_URL,
  PROVIDER_TEMPLATES,
  type Protocol,
} from "./protocols.ts";

const ALL: Protocol[] = Object.keys(PROTOCOL_LABELS) as Protocol[];

describe("协议常量", () => {
  it("每个协议都有标签、预设地址与预设模型", () => {
    for (const p of ALL) {
      assert.ok(PROTOCOL_LABELS[p], `${p} 缺标签`);
      assert.ok(PROTOCOL_PRESET_URL[p], `${p} 缺预设地址`);
      assert.ok(PROTOCOL_PRESET_MODELS[p].length > 0, `${p} 缺预设模型`);
    }
  });

  it("预设地址是 https 且没有尾随斜杠", () => {
    for (const p of ALL) {
      const url = PROTOCOL_PRESET_URL[p];
      assert.ok(url.startsWith("https://"), `${p} 预设地址应为 https：${url}`);
      assert.ok(!url.endsWith("/"), `${p} 预设地址不该以斜杠结尾：${url}`);
    }
  });

  it("协议列表覆盖后端实现的六种协议", () => {
    // 新增后端协议时忘了加前端标签，这里会先炸
    assert.deepEqual([...ALL].sort(), [
      "dashscope",
      "gemini_native",
      "midjourney_proxy",
      "openai_chat_image",
      "openai_images",
      "volcengine_ark",
    ]);
  });
});

describe("供应商模板", () => {
  it("模板 id 唯一", () => {
    const ids = PROVIDER_TEMPLATES.map((t) => t.id);
    assert.equal(new Set(ids).size, ids.length);
  });

  it("每个模板的协议都存在于协议表里，且地址与模型齐全", () => {
    for (const t of PROVIDER_TEMPLATES) {
      assert.ok(PROTOCOL_LABELS[t.protocol], `${t.id} 用了未知协议 ${t.protocol}`);
      assert.ok(t.base_url.startsWith("https://"), `${t.id} 地址应为 https`);
      assert.ok(t.models.length > 0, `${t.id} 至少要有一个常用模型`);
      for (const m of t.models) {
        assert.ok(m.trim().length > 0, `${t.id} 有空模型名`);
      }
    }
  });

  it("模板内的模型名不重复", () => {
    for (const t of PROVIDER_TEMPLATES) {
      const names = t.models.map((m) => m.trim());
      assert.equal(new Set(names).size, names.length, `${t.id} 有重复模型名`);
    }
  });
});
