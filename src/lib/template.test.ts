import { test } from "node:test";
import assert from "node:assert/strict";
import {
  composeTemplate,
  composeText,
  defaultValues,
  parseRatio,
  pickSizeForRatio,
  templateCategories,
  type PromptTemplate,
} from "./template.ts";

const tpl: PromptTemplate = {
  id: "t",
  title: "示例",
  category: "角色设计",
  summary: "",
  fields: [
    { key: "character", label: "角色形象", required: true },
    { key: "style", label: "画风", default: "二次元" },
    { key: "extra", label: "附加要求" },
  ],
  prompt: "角色：{{character}}。画风：{{style}}。附加：{{extra}}。",
};

test("必填字段没填时列进 missing", () => {
  const r = composeTemplate(tpl, {});
  assert.deepEqual(r.missing, ["角色形象"]);
});

test("必填字段填了就不报缺", () => {
  const r = composeTemplate(tpl, { character: "银发少女" });
  assert.deepEqual(r.missing, []);
  assert.match(r.prompt, /角色：银发少女/);
});

test("可选字段留空时整段被清掉，不留脏标点", () => {
  const r = composeTemplate(tpl, { character: "银发少女", style: "二次元" });
  assert.match(r.prompt, /角色：银发少女/);
  assert.match(r.prompt, /画风：二次元/);
  assert.ok(!r.prompt.includes("附加"), "空的可选字段不该留下痕迹");
  assert.ok(!r.prompt.includes("。."), "不该出现句号叠句号");
});

test("字段默认值在没填时生效", () => {
  const values = defaultValues(tpl);
  assert.equal(values.style, "二次元");
  assert.equal(values.character, "");
  const r = composeTemplate(tpl, { ...values, character: "A" });
  assert.match(r.prompt, /画风：二次元/);
});

test("模板与步骤共用同一批字段值", () => {
  const withStep: PromptTemplate = {
    ...tpl,
    steps: [{ title: "步骤1", prompt: "为 {{character}} 画三视图" }],
  };
  const r = composeTemplate(withStep, { character: "剑士" });
  assert.match(r.prompt, /剑士/);
  assert.match(withStep.steps![0].prompt, /\{\{character\}\}/);
});

test("占位符前后空格容错", () => {
  assert.equal(composeText("a {{ name }} b", { name: "X" }), "a X b");
});

test("未知占位符按空处理，不会把原文留在提示词里", () => {
  assert.equal(composeText("前缀 {{missing}} 后缀", {}), "前缀 后缀");
});

test("比例解析认得像素、比例与星号三种写法", () => {
  assert.equal(parseRatio("1024x1536"), 1024 / 1536);
  assert.equal(parseRatio("1024*768"), 1024 / 768);
  assert.equal(parseRatio("16:9"), 16 / 9);
  assert.equal(parseRatio("auto"), undefined);
});

test("按比例挑尺寸：命中精确值优先", () => {
  assert.equal(pickSizeForRatio(["1:1", "16:9", "9:16"], "16:9"), "16:9");
});

test("按比例挑尺寸：像素表里挑最接近的比例", () => {
  // 16:9 最接近 3:2（1536x1024），不是 1:1
  assert.equal(pickSizeForRatio(["1024x1024", "1536x1024", "1024x1536"], "16:9"), "1536x1024");
});

test("能力表为空或没有可比尺寸时返回 undefined，不硬塞", () => {
  assert.equal(pickSizeForRatio([], "16:9"), undefined);
  assert.equal(pickSizeForRatio(undefined, "16:9"), undefined);
  assert.equal(pickSizeForRatio(["1024x1024"], "auto"), undefined);
});

test("分类列表按首次出现顺序去重", () => {
  const cats = templateCategories([
    { ...tpl, id: "a", category: "角色设计" },
    { ...tpl, id: "b", category: "人物摄影" },
    { ...tpl, id: "c", category: "角色设计" },
  ]);
  assert.deepEqual(cats, ["角色设计", "人物摄影"]);
});
