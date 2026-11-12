/** 任务列表轮询用的内容指纹。errors 变化也要触发重渲染，否则失败原因不会刷新。 */
export interface FingerprintedTask {
  id: string;
  status: string;
  done: number;
  total: number;
  images: { length: number };
  errors: { length: number };
  seed?: number | null;
  sessionId?: string | null;
}

export function signature(list: FingerprintedTask[]): string {
  return list
    .map((t) =>
      [
        t.id,
        t.status,
        `${t.done}/${t.total}`,
        t.images.length,
        t.errors.length,
        t.seed ?? "",
        t.sessionId ?? "",
      ].join(":"),
    )
    .join("|");
}
