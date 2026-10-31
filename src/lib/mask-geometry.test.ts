import test from "node:test";
import assert from "node:assert/strict";
import { contentRect, toCanvasPoint } from "./mask-geometry.ts";

// 竖图 1024x1536 在 510px 宽容器里被 max-h-[420px] 夹住：
// 实际内容只有 280x420，左右各 115px 是 letterbox 留白
const portrait = { width: 1024, height: 1536 };
const box = { width: 510, height: 420 };

test("竖图：内容区按高度等比、水平居中", () => {
  assert.deepEqual(contentRect(portrait, box), { x: 115, y: 0, width: 280, height: 420 });
});

test("竖图：内容左上角映射到画布 (0,0)，右下角映射到 (1024,1536)", () => {
  assert.deepEqual(toCanvasPoint({ x: 115, y: 0 }, portrait, box), { x: 0, y: 0 });
  assert.deepEqual(toCanvasPoint({ x: 395, y: 420 }, portrait, box), { x: 1024, y: 1536 });
});

test("竖图：旧算法的偏移点在新算法里落回图像中部", () => {
  // 元素盒正中（x=255）落在内容区里，画布上应是水平中点 512
  const p = toCanvasPoint({ x: 255, y: 210 }, portrait, box);
  assert.equal(p.x, 512);
  assert.equal(p.y, 768);
});

test("指针落在左右留白里时夹到画布边缘，不产生负坐标", () => {
  const p = toCanvasPoint({ x: 0, y: 210 }, portrait, box);
  assert.equal(p.x, 0);
  assert.equal(p.y, 768);
});

test("横图未触发 max-height 时是等比缩放、零偏移", () => {
  const canvas = { width: 2048, height: 1024 };
  const wide = { width: 510, height: 255 };
  assert.deepEqual(contentRect(canvas, wide), { x: 0, y: 0, width: 510, height: 255 });
  assert.deepEqual(toCanvasPoint({ x: 255, y: 127.5 }, canvas, wide), { x: 1024, y: 512 });
});
