import type { RefImage } from "../types";

/** 参考图控件最多收 4 张，转换在提交链路之外完成（FileReader 不可测但可复用）。 */
export const MAX_REFS = 4;

export function fileToRef(file: File): Promise<RefImage> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = String(reader.result);
      const [head, data] = dataUrl.split(",");
      const mime = head.slice(5).split(";")[0] || "image/png";
      resolve({ name: file.name, mime, data });
    };
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

export async function filesToRefs(files: FileList | File[]): Promise<RefImage[]> {
  const imgs = Array.from(files).filter((f) => f.type.startsWith("image/"));
  return Promise.all(imgs.slice(0, MAX_REFS).map(fileToRef));
}

/**
 * 把后端 image_read_b64 / ffmpeg 工具返回的 data URL 转成 RefImage。
 * data 字段不带前缀，mime 单独存，和 fileToRef 的输出保持一致。
 */
export function dataUrlToRef(dataUrl: string, name: string, id?: string): RefImage {
  const comma = dataUrl.indexOf(",");
  const head = comma >= 0 ? dataUrl.slice(0, comma) : "";
  const data = comma >= 0 ? dataUrl.slice(comma + 1) : dataUrl;
  const mime = head.startsWith("data:")
    ? head.slice(5).split(";")[0] || "image/png"
    : "image/png";
  // 不传 id 时不写出该字段，避免给现有调用方多一个 undefined 键
  return id ? { name, mime, data, id } : { name, mime, data };
}

/**
 * 追加参考图并去重：有来源 id 时按 id 比较（O(1)），否则按完整 base64 比较
 * （同一文件读出的内容一致，不需要算 hash）。只收得下的那部分，
 * added 让调用方能提示「满了」，duplicates 让调用方能提示「重复」。
 */
export function appendRefs(
  current: RefImage[],
  incoming: RefImage[],
): { refs: RefImage[]; added: number; duplicates: number } {
  const seen = new Set(current.map((r) => r.id ?? r.data));
  let duplicates = 0;
  const unique: RefImage[] = [];
  for (const ref of incoming) {
    const key = ref.id ?? ref.data;
    if (seen.has(key)) {
      duplicates += 1;
      continue;
    }
    seen.add(key);
    unique.push(ref);
  }
  const room = Math.max(0, MAX_REFS - current.length);
  const added = Math.min(room, unique.length);
  return { refs: [...current, ...unique.slice(0, added)], added, duplicates };
}
