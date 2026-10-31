/**
 * 蒙版编辑器的坐标换算。
 *
 * 画布 CSS 是 `w-full max-h-[420px] object-contain`：元素盒（getBoundingClientRect）
 * 与真实图像内容并不总是一样大——被 max-height 夹住时图像居中、两侧留白。
 * 指针坐标必须先减掉这段留白再换算到画布内部分辨率，否则涂出来的蒙版整体偏移，
 * 竖图尤其明显（1024x1536 会横偏 115px）。
 */

export interface Point {
  x: number;
  y: number;
}

export interface Size {
  width: number;
  height: number;
}

export interface Box extends Point, Size {}

/** object-contain 后图像内容在元素盒里的实际矩形。 */
export function contentRect(canvas: Size, box: Size): Box {
  const scale = Math.min(box.width / canvas.width, box.height / canvas.height);
  const width = canvas.width * scale;
  const height = canvas.height * scale;
  return { x: (box.width - width) / 2, y: (box.height - height) / 2, width, height };
}

/**
 * 指针（已减去元素盒左上角）→ 画布像素坐标。
 * 落在留白里时夹到画布边缘：涂抹笔触可以超出图像显示范围，但画布坐标不能为负，
 * 否则 destination-out 会作用在画布外、导出的蒙版与底图错位。
 */
export function toCanvasPoint(pointer: Point, canvas: Size, box: Size): Point {
  const c = contentRect(canvas, box);
  const x = ((pointer.x - c.x) / c.width) * canvas.width;
  const y = ((pointer.y - c.y) / c.height) * canvas.height;
  return {
    x: Math.min(Math.max(x, 0), canvas.width),
    y: Math.min(Math.max(y, 0), canvas.height),
  };
}
