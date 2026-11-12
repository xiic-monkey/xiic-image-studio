import test from "node:test";
import assert from "node:assert/strict";
import { draftFromForm, draftSignature, EMPTY_DRAFT, formFromDraft } from "./draft.ts";

const form = { ...EMPTY_DRAFT, providerId: "p1", prompt: "cat", seed: "7" };

test("draftFromForm 不含 refs / mask / session_id", () => {
  const keys = Object.keys(draftFromForm(form));
  assert.deepEqual(keys.sort(), ["batchMode", "count", "prompt", "providerId", "quality", "seed", "size"]);
  assert.ok(!keys.includes("refs"));
  assert.ok(!keys.includes("session_id"));
});

test("formFromDraft 补齐默认尺寸 / 质量 / 数量", () => {
  const restored = formFromDraft({ prompt: "dog" }, { ...EMPTY_DRAFT, size: "512x512" });
  assert.equal(restored.prompt, "dog");
  assert.equal(restored.size, "512x512");
  assert.equal(restored.quality, "auto");
  assert.equal(restored.count, 1);
  assert.equal(restored.seed, "");
  assert.equal(restored.batchMode, false);
});

test("formFromDraft 处理空草稿", () => {
  assert.deepEqual(formFromDraft(undefined), EMPTY_DRAFT);
});

test("draftSignature 同形稳定、改值即变", () => {
  assert.equal(draftSignature(form), draftSignature({ ...form }));
  assert.notEqual(draftSignature(form), draftSignature({ ...form, prompt: "dog" }));
  assert.notEqual(draftSignature(form), draftSignature({ ...form, batchMode: true }));
});
