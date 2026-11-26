import test from "node:test";
import assert from "node:assert/strict";
import { canRetry, retryRequest, type RetryableTask } from "./retry.ts";

const task: RetryableTask = {
  status: "failed",
  prompt: "cat",
  providerId: "p1",
  n: 3,
  size: "1024x1024",
  quality: "high",
  seed: 42,
  refCount: 0,
  refPaths: [],
  sessionId: "s1",
};

test("running 状态不可重试", () => {
  assert.equal(canRetry({ ...task, status: "running" }), false);
});

test("旧图生图任务（没有参考图快照）不可重试", () => {
  assert.equal(canRetry({ ...task, refCount: 1 }), false);
  assert.equal(canRetry({ ...task, refCount: 1, refPaths: [] }), false);
});

test("有参考图快照的图生图任务可重试", () => {
  assert.equal(canRetry({ ...task, refCount: 1, refPaths: [".refs/a/0.png"] }), true);
});

test("空 prompt 或无供应商不可重试", () => {
  assert.equal(canRetry({ ...task, prompt: "  " }), false);
  assert.equal(canRetry({ ...task, providerId: "" }), false);
});

test("失败任务可重试", () => {
  assert.equal(canRetry(task), true);
});

test("已完成的任务不挂重试入口", () => {
  assert.equal(canRetry({ ...task, status: "completed" }), false);
});

test("retryRequest 保留 seed / size / quality / n / sessionId", () => {
  const req = retryRequest(task, "current-session");
  assert.equal(req.seed, 42);
  assert.equal(req.size, "1024x1024");
  assert.equal(req.quality, "high");
  assert.equal(req.n, 3);
  assert.equal(req.session_id, "s1");
  assert.deepEqual(req.refs, []);
});

test("retryRequest 带上参考图快照复现图生图", () => {
  const refs = [
    { name: "ref-1", mime: "image/png", data: "AAAA" },
    { name: "ref-2", mime: "image/jpeg", data: "BBBB" },
  ];
  const req = retryRequest({ ...task, refCount: 2, refPaths: ["a", "b"] }, "current-session", undefined, refs);
  assert.deepEqual(req.refs, refs);
  assert.equal(req.refs.length, 2, "重试必须带回全部参考图，不能退化成文生图");
});

test("旧任务无归属时回落到当前会话", () => {
  assert.equal(retryRequest({ ...task, sessionId: null }, "current-session").session_id, "current-session");
});

test("retryRequest 复原原任务的 model_id", () => {
  const req = retryRequest({ ...task, modelId: "m-gpt" }, "current-session");
  assert.equal(req.model_id, "m-gpt", "重试必须回到原模型，不能跟着当前选择漂移");
  // 旧任务没有 model_id 时不带字段，由后端回退到默认模型
  assert.equal(retryRequest(task, "current-session").model_id, undefined);
});
