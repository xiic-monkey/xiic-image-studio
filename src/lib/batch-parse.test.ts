import test from "node:test";
import assert from "node:assert/strict";
import { detectFormat, parseBatch, parseCsvLine, parseCsvRecords } from "./batch-parse.ts";

test("CSV：引号里的逗号不算分隔符，提示词不被切断", () => {
  const items = parseBatch('"一只猫, 写实摄影",1024x1024,hd', "batch.csv");
  assert.equal(items.length, 1);
  assert.equal(items[0].prompt, "一只猫, 写实摄影");
  assert.equal(items[0].size, "1024x1024");
  assert.equal(items[0].quality, "hd");
});

test("CSV：双引号写成两个双引号是转义", () => {
  const items = parseBatch('"他说""好""",1024x1024', "batch.csv");
  assert.equal(items[0].prompt, '他说"好"');
});

test("CSV：表头行跳过，其余按 prompt,size,quality,seed,n 解析", () => {
  const items = parseBatch("prompt,size,quality,seed,n\n猫,1024x1024,hd,42,2\n狗,,,", "batch.csv");
  assert.equal(items.length, 2);
  assert.deepEqual(items[0], { prompt: "猫", size: "1024x1024", quality: "hd", seed: "42", n: "2" });
  assert.equal(items[1].prompt, "狗");
  assert.equal(items[1].size, undefined);
});

test("CSV：引号内的换行属于字段内容，不切记录", () => {
  const records = parseCsvRecords('"第一行\n第二行",1024x1024\n"另一条"');
  assert.equal(records.length, 2);
  assert.equal(records[0][0], "第一行\n第二行");
  assert.equal(records[1][0], "另一条");
});

test("纯文本：整行是一条提示词，提示词里的逗号不被切断", () => {
  const items = parseBatch("一只猫, 写实摄影\n窗外有树, 黄昏", "list.txt");
  assert.deepEqual(items, [
    { prompt: "一只猫, 写实摄影" },
    { prompt: "窗外有树, 黄昏" },
  ]);
});

test("JSONL：每行一个 JSON 对象，取 prompt/size/quality/seed/n", () => {
  const items = parseBatch(
    '{"prompt":"猫","size":"1024x1024","quality":"hd","seed":7,"n":2}\n{"p":"狗"}',
    "batch.jsonl",
  );
  assert.equal(items.length, 2);
  assert.equal(items[0].prompt, "猫");
  assert.equal(items[0].seed, "7");
  assert.equal(items[0].n, "2");
  assert.equal(items[1].prompt, "狗");
});

test("JSONL：不是 JSON 的行当裸提示词", () => {
  const items = parseBatch("裸提示词一行", "batch.jsonl");
  assert.deepEqual(items, [{ prompt: "裸提示词一行" }]);
});

test("格式判定：优先看扩展名，认不出就看内容", () => {
  assert.equal(detectFormat("x", "a.csv"), "csv");
  assert.equal(detectFormat("x", "a.jsonl"), "jsonl");
  assert.equal(detectFormat("x", "a.txt"), "text");
  assert.equal(detectFormat('{"prompt":"猫"}'), "jsonl");
  assert.equal(detectFormat("prompt,size\n猫,1024x1024"), "csv");
  assert.equal(detectFormat("猫,狗"), "text");
});

test("CSV 行切分：普通字段与引号字段混排", () => {
  assert.deepEqual(parseCsvLine('a,"b,c",d'), ["a", "b,c", "d"]);
  assert.deepEqual(parseCsvLine("a,b,c"), ["a", "b", "c"]);
});

test("空行与空提示词被丢掉", () => {
  assert.deepEqual(parseBatch("\n\n   \n", "list.txt"), []);
  assert.deepEqual(parseBatch('"",1024x1024', "batch.csv"), []);
});
