import { test } from "node:test";
import assert from "node:assert/strict";
import { errorMessage } from "./error.ts";

test("Error 实例取 message，不带 Error: 前缀", () => {
  assert.equal(errorMessage(new Error("连接失败")), "连接失败");
});

test("字符串错误原样返回", () => {
  assert.equal(errorMessage("命令不存在"), "命令不存在");
});

test("非 Error 兜底成字符串", () => {
  assert.equal(errorMessage(42), "42");
  assert.equal(errorMessage(null), "null");
});
