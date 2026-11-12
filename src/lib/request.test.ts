import test from "node:test";
import assert from "node:assert/strict";
import type { Capabilities } from "../types.ts";
import { buildGenerateRequest, type FormInput } from "./request.ts";

const caps: Capabilities = {
  sizes: ["1024x1024", "1536x1024"],
  quality: ["auto", "high"],
  seed: true,
  refs: true,
  mask: true,
  endpoint: "images",
  sizeLabel: "尺寸",
};

function form(overrides: Partial<FormInput> = {}): FormInput {
  return {
    providerId: "p1",
    prompt: "cat",
    n: 2,
    size: "1024x1024",
    quality: "high",
    seed: "42",
    refs: [],
    mask: null,
    ...overrides,
  };
}

test("buildGenerateRequest 带 session_id 与全部参数", () => {
  const req = buildGenerateRequest(form({ sessionId: "s1", seed: "42" }), caps);
  assert.equal(req.session_id, "s1");
  assert.equal(req.seed, 42);
  assert.equal(req.n, 2);
  assert.equal(req.size, "1024x1024");
  assert.equal(req.quality, "high");
});

test("能力表关闭 seed 时不下发", () => {
  const req = buildGenerateRequest(form(), { ...caps, seed: false });
  assert.equal(req.seed, undefined);
});

test("非法 seed 与空 seed 不下发", () => {
  assert.equal(buildGenerateRequest(form({ seed: "abc" }), caps).seed, undefined);
  assert.equal(buildGenerateRequest(form({ seed: "-1" }), caps).seed, undefined);
  assert.equal(buildGenerateRequest(form({ seed: "  " }), caps).seed, undefined);
});

test("size / quality 按能力表回落到表内首个值", () => {
  const req = buildGenerateRequest(form({ size: "999x999", quality: "hd" }), caps);
  assert.equal(req.size, "1024x1024");
  assert.equal(req.quality, "auto");
});

test("不支持 mask 时丢弃，支持时透传", () => {
  const mask = { name: "m.png", mime: "image/png", data: "aGk=" };
  assert.equal(buildGenerateRequest(form({ mask }), { ...caps, mask: false }).mask, undefined);
  assert.deepEqual(buildGenerateRequest(form({ mask }), caps).mask, mask);
});

test("n 被 clamp 到 1..8", () => {
  assert.equal(buildGenerateRequest(form({ n: 0 }), caps).n, 1);
  assert.equal(buildGenerateRequest(form({ n: 99 }), caps).n, 8);
});
