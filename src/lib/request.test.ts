import test from "node:test";
import assert from "node:assert/strict";
import type { Capabilities } from "../types.ts";
import { buildGenerateRequest, type FormInput } from "./request.ts";
import { buildScanSubmissions } from "./submission.ts";

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

test("扫描任务：每个种子各自组装成一条请求", () => {
  // 扫描把「一条提示词 + 连续种子」拆成多个任务，请求组装层看到的
  // 就是普通请求，只是 seed 不同、n 恒为 1。
  const subs = buildScanSubmissions("猫", 100, 3);
  const reqs = subs.map((s) =>
    buildGenerateRequest(form({ prompt: s.prompt, n: s.n, seed: String(s.seed) }), caps),
  );
  assert.deepEqual(reqs.map((r) => r.seed), [100, 101, 102]);
  assert.deepEqual(reqs.map((r) => r.n), [1, 1, 1]);
  assert.ok(reqs.every((r) => r.prompt === "猫"));
});

test("扫描在不支持 seed 的模型上不会悄悄变成重复出图", () => {
  // 前端提交入口会先按能力表拒绝扫描；这里锁住请求层的行为：
  // seed 被丢弃，n 仍是 1 —— 不会退化成「同一提示词随机出 N 张」。
  const subs = buildScanSubmissions("猫", 100, 3);
  const req = buildGenerateRequest(
    form({ prompt: subs[0].prompt, n: subs[0].n, seed: String(subs[0].seed) }),
    { ...caps, seed: false },
  );
  assert.equal(req.seed, undefined);
  assert.equal(req.n, 1);
});
