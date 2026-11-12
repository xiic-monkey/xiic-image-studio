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
