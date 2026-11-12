import test from "node:test";
import assert from "node:assert/strict";
import { signature, type FingerprintedTask } from "./task-signature.ts";

const task: FingerprintedTask = {
  id: "t1",
  status: "running",
  done: 1,
  total: 2,
  images: { length: 1 },
  errors: { length: 0 },
  seed: 7,
  sessionId: "s1",
};

test("相同列表指纹稳定", () => {
  assert.equal(signature([task]), signature([{ ...task }]));
});

test("errors 长度变化会改变指纹", () => {
  assert.notEqual(signature([task]), signature([{ ...task, errors: { length: 1 } }]));
});

test("seed 与 sessionId 变化会改变指纹", () => {
  assert.notEqual(signature([task]), signature([{ ...task, seed: 8 }]));
  assert.notEqual(signature([task]), signature([{ ...task, sessionId: null }]));
});

test("图片数量与进度变化会改变指纹", () => {
  assert.notEqual(signature([task]), signature([{ ...task, images: { length: 2 } }]));
  assert.notEqual(signature([task]), signature([{ ...task, done: 2 }]));
});
