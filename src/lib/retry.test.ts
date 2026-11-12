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
  sessionId: "s1",
};

test("running 状态不可重试", () => {
  assert.equal(canRetry({ ...task, status: "running" }), false);
});

test("图生图任务（refCount>0）不可重试", () => {
  assert.equal(canRetry({ ...task, refCount: 1 }), false);
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

test("旧任务无归属时回落到当前会话", () => {
  assert.equal(retryRequest({ ...task, sessionId: null }, "current-session").session_id, "current-session");
});
