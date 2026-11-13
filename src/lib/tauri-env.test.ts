import test from "node:test";
import assert from "node:assert/strict";
import { hasTauriBridge, NOT_IN_APP_MESSAGE } from "./tauri-env.ts";

test("没有 window 的环境（node）里判定为不在应用内", () => {
  assert.equal(hasTauriBridge(), false);
});

test("提示语要说清是浏览器预览、以及怎么用应用", () => {
  assert.match(NOT_IN_APP_MESSAGE, /浏览器预览/);
  assert.match(NOT_IN_APP_MESSAGE, /tauri dev/);
});
