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
  /** 重试要靠这两个字段复原供应商与模型；不参与指纹的话，
   *  轮询拿到的完整数据会被“没变化”的判断丢掉，卡片永远缺它们。 */
  providerId?: string;
  modelId?: string | null;
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
        t.providerId ?? "",
        t.modelId ?? "",
      ].join(":"),
    )
    .join("|");
}
