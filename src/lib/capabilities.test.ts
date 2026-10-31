import test from "node:test";
import assert from "node:assert/strict";
import { isSeedEnabled, normalizeQuality, normalizeSize } from "./capabilities.ts";
import type { Capabilities } from "../types.ts";

const caps = (over: Partial<Capabilities> = {}): Capabilities => ({
  sizes: ["1024x1024", "1536x1024"],
  quality: ["auto", "high"],
  seed: true,
  refs: true,
  mask: true,
  endpoint: "images",
  sizeLabel: "尺寸",
  ...over,
});

test("当前尺寸合法就保持不变", () => {
  assert.equal(normalizeSize(caps(), "1536x1024"), "1536x1024");
});

test("当前尺寸不在能力表内就回落到第一个", () => {
  assert.equal(normalizeSize(caps(), "2048x2048"), "1024x1024");
});

test("不支持尺寸的协议没有可用尺寸", () => {
  assert.equal(normalizeSize(caps({ sizes: [] }), "1024x1024"), undefined);
});

test("质量合法则保持，不合法回落到第一个", () => {
  assert.equal(normalizeQuality(caps(), "high"), "high");
  assert.equal(normalizeQuality(caps(), "standard"), "auto");
});

test("不支持质量时返回 undefined", () => {
  assert.equal(normalizeQuality(caps({ quality: [] }), "auto"), undefined);
});

test("种子是否可用由能力表决定", () => {
  assert.equal(isSeedEnabled(caps()), true);
  assert.equal(isSeedEnabled(caps({ seed: false })), false);
});
