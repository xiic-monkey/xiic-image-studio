import { test } from "node:test";
import assert from "node:assert/strict";
import { appendRefs, dataUrlToRef, MAX_REFS } from "./ref-image.ts";

test("标准 data URL 拆成 mime + 裸 base64", () => {
  assert.deepEqual(dataUrlToRef("data:image/webp;base64,AAAA", "a.webp"), {
    name: "a.webp",
    mime: "image/webp",
    data: "AAAA",
  });
});

test("逗号在 base64 里不会出现，只认第一个逗号", () => {
  assert.equal(dataUrlToRef("data:image/png;base64,AA,BB", "x").data, "AA,BB");
});

test("没有 data: 前缀时按 png 兜底", () => {
  assert.deepEqual(dataUrlToRef("AAAA", "x"), { name: "x", mime: "image/png", data: "AAAA" });
});

test("data: 但缺 mime 时也兜底 png", () => {
  assert.equal(dataUrlToRef("data:;base64,AAAA", "x").mime, "image/png");
});

test("参考图上限为 4", () => {
  assert.equal(MAX_REFS, 4);
});

const mk = (name: string) => dataUrlToRef(`data:image/png;base64,${name}`, name);

test("appendRefs 追加到末尾，顺序不变", () => {
  const r = appendRefs([mk("a")], [mk("b")]);
  assert.equal(r.added, 1);
  assert.deepEqual(r.refs.map((x) => x.name), ["a", "b"]);
});

test("appendRefs 只收到上限为止，多的不静默塞进来", () => {
  const r = appendRefs([mk("1"), mk("2"), mk("3")], [mk("4"), mk("5")]);
  assert.equal(r.added, 1);
  assert.deepEqual(r.refs.map((x) => x.name), ["1", "2", "3", "4"]);
});

test("满了再加返回 0，原内容不变", () => {
  const full = [mk("1"), mk("2"), mk("3"), mk("4")];
  const r = appendRefs(full, [mk("5")]);
  assert.equal(r.added, 0);
  assert.deepEqual(r.refs, full);
});
