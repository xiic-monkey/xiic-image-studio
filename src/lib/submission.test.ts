import test from "node:test";
import assert from "node:assert/strict";
import { buildSubmissions, pickPrefillPrompt, remainingText } from "./submission.ts";

test("单条模式：整段多行文本是一个提示词，数量沿用用户选择", () => {
  const text = "一只戴帽子的猫\n写实摄影\n柔和侧光";
  assert.deepEqual(buildSubmissions(text, false, 4), [{ prompt: text, n: 4 }]);
});

test("批量模式：每行一个任务、每行 1 张，空行丢弃", () => {
  const text = "猫\n\n狗  \n  ";
  assert.deepEqual(buildSubmissions(text, true, 4), [
    { prompt: "猫", n: 1 },
    { prompt: "狗", n: 1 },
  ]);
});

test("空文本不提交", () => {
  assert.deepEqual(buildSubmissions("   \n  \n", true, 2), []);
  assert.deepEqual(buildSubmissions("", false, 1), []);
});

test("部分失败：输入框回填未发出的行（含失败那一行）", () => {
  const subs = buildSubmissions("猫\n狗\n鸟", true, 1);
  assert.equal(remainingText(subs, 1), "狗\n鸟");
  assert.equal(remainingText(subs, 3), "");
});

test("prefill 没带 prompt 时保持输入框现状", () => {
  // 工具箱「发到工作台」只塞参考图，不该把用户写到一半的提示词清空
  assert.equal(pickPrefillPrompt("正在写的提示词", undefined), "正在写的提示词");
  assert.equal(pickPrefillPrompt("旧内容", ""), "");
  assert.equal(pickPrefillPrompt("旧内容", "新内容"), "新内容");
});
