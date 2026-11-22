import test from "node:test";
import assert from "node:assert/strict";
import {
  buildScanSubmissions,
  buildSubmissions,
  parseSeedBase,
  pickPrefillPrompt,
  remainingText,
  MAX_SCAN,
} from "./submission.ts";

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

test("扫描：一条提示词按连续种子拆成多个任务，每任务 1 张", () => {
  const subs = buildScanSubmissions("一只猫", 100, 3);
  assert.equal(subs.length, 3);
  assert.deepEqual(subs, [
    { prompt: "一只猫", n: 1, seed: 100 },
    { prompt: "一只猫", n: 1, seed: 101 },
    { prompt: "一只猫", n: 1, seed: 102 },
  ]);
});

test("扫描：提示词为空不产出任务", () => {
  assert.deepEqual(buildScanSubmissions("   \n ", 0, 4), []);
});

test("扫描：张数被夹在 1..MAX_SCAN，非法值按 1 张算", () => {
  assert.equal(buildScanSubmissions("猫", 0, 999).length, MAX_SCAN);
  assert.equal(buildScanSubmissions("猫", 0, 0).length, 1);
  assert.equal(buildScanSubmissions("猫", 0, -3).length, 1);
});

test("扫描：起始种子留空或非法时从 0 开始（必须可复现）", () => {
  assert.equal(parseSeedBase(""), 0);
  assert.equal(parseSeedBase("   "), 0);
  assert.equal(parseSeedBase("abc"), 0);
  assert.equal(parseSeedBase("-5"), 0);
  assert.equal(parseSeedBase("42"), 42);
  assert.equal(parseSeedBase(" 42 "), 42);
});

test("普通提交不带 seed 字段（沿用表单里的种子）", () => {
  const subs = buildSubmissions("猫", false, 2);
  assert.equal(subs[0].seed, undefined);
});
