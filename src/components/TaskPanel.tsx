import { useCallback, useEffect, useRef, useState } from "react";
import { api, onTaskProgress } from "../ipc";
import type { ImageMeta, TaskItem, TaskProgress } from "../types";

export interface TaskView {
  id: string;
  status: string;
  done: number;
  total: number;
  errors: string[];
  images: ImageMeta[];
  createdAt: string;
  prompt: string;
  /** 原始参数，重试时复用（重试不带参考图，refCount > 0 的任务不提供重试） */
  providerId: string;
  size: string | null;
  quality: string | null;
  n: number;
  refCount: number;
}

/** 任务列表保留上限。初次加载是 30 条，运行中 upsert 出来的也必须裁到这个数，
 *  否则长跑时列表只增不减，配套的缩略图缓存也就永远回收不了。 */
const MAX_TASKS = 30;

const STATUS_LABEL: Record<string, { text: string; cls: string }> = {
  running: { text: "生成中", cls: "bg-blue-50 text-blue-700" },
  completed: { text: "完成", cls: "bg-green-50 text-green-700" },
  completed_with_errors: { text: "部分失败", cls: "bg-amber-50 text-amber-700" },
  failed: { text: "失败", cls: "bg-red-50 text-red-700" },
  canceled: { text: "已取消", cls: "bg-neutral-100 text-neutral-500" },
};

/** 出现这些状态说明任务没跑成，给用户一个重试入口 */
const RETRYABLE_STATUS = new Set(["failed", "completed_with_errors", "canceled"]);

/** 任务列表的"变化指纹"：id + 状态 + 进度 + 图片数。
 *  轮询时用它判断列表是否真的变了，没变就不 setState，避免每 3 秒重渲染一次。 */
function signature(list: TaskView[]): string {
  return list.map((t) => `${t.id}:${t.status}:${t.done}/${t.total}:${t.images.length}`).join("|");
}

// 把多行提示词压成一行并截断，作为任务卡上的缩写
function shorten(text: string, n = 80): string {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length > n ? flat.slice(0, n) + "\u2026" : flat;
}

export function useTaskFeed() {
  const [tasks, setTasks] = useState<TaskView[]>([]);
  const b64Cache = useRef<Map<string, string>>(new Map());
  const [imgUrls, setImgUrls] = useState<Record<string, string>>({});

  const loadImage = useCallback(
    async (img: ImageMeta) => {
      const key = img.thumb_path || img.path;
      if (b64Cache.current.has(key)) return;
      b64Cache.current.set(key, "");
      try {
        const url = await api.imageReadB64(key);
        b64Cache.current.set(key, url);
        setImgUrls((prev) => ({ ...prev, [key]: url }));
      } catch {
        b64Cache.current.delete(key);
      }
    },
    [],
  );

  const upsert = useCallback((p: Partial<TaskView> & { id: string; error?: string }) => {
    setTasks((prev) => {
      const idx = prev.findIndex((t) => t.id === p.id);
      const base: TaskView =
        idx === -1
          ? {
              id: p.id,
              status: "running",
              done: 0,
              total: 1,
              errors: [],
              images: [],
              createdAt: new Date().toISOString(),
              prompt: "",
              providerId: "",
              size: null,
              quality: null,
              n: 1,
              refCount: 0,
            }
          : prev[idx];
      // error 来自运行中的 run_one / finish_task 实时事件，并入 errors 数组并去重
      const errors = p.error
        ? base.errors.includes(p.error)
          ? base.errors
          : [p.error, ...base.errors]
        : base.errors;
      const merged: TaskView = { ...base, ...p, errors };
      if (idx === -1) return [merged, ...prev].slice(0, MAX_TASKS);
      const next = [...prev];
      next[idx] = merged;
      return next;
    });
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const refresh = async () => {
      let items: TaskItem[];
      try {
        items = await api.taskList(30);
      } catch {
        return;
      }
      if (cancelled) return;
      const views: TaskView[] = items.map((t) => ({
        id: t.id,
        status: t.status,
        done: t.done,
        total: t.total,
        errors: t.error ? [t.error] : [],
        images: t.images ?? [],
        createdAt: t.created_at,
        prompt: t.prompt ?? "",
        providerId: t.provider_id,
        size: t.size ?? null,
        quality: t.quality ?? null,
        n: t.n ?? 1,
        refCount: t.ref_count ?? 0,
      }));
      // 只在任务集合真的变了才 setState：轮询每 3s 一次，
      // 无脑覆盖会让整个列表（含缩略图）每 3 秒重渲染一次。
      setTasks((prev) => (signature(prev) === signature(views) ? prev : views));
      // 回填历史任务的缩略图：实时进度只推"新生成"的图，不补这一步的话
      // 刷新窗口后老任务卡永远是一片空白（只有 prompt 文字）。
      views.forEach((v) => v.images.forEach((img) => loadImage(img)));
    };

    refresh();
    // 轮询的必要性：xiic-image-mcp（或任何外部进程）直接往同一个 SQLite 写任务，
    // 它发不出 tauri 事件，光靠 onTaskProgress 永远发现不了这些任务——
    // 不轮询的话，MCP 生成的图要重启 app 才能在工作台看到。
    const timer = setInterval(refresh, 3000);

    onTaskProgress((p: TaskProgress) => {
      upsert({
        id: p.taskId,
        status: p.status,
        done: p.done,
        total: p.total,
        error: p.error,
      });
      if (p.image) {
        setTasks((prev) =>
          prev.map((t) =>
            t.id === p.taskId && !t.images.some((i) => i.id === p.image!.id)
              ? { ...t, images: [p.image!, ...t.images] }
              : t,
          ),
        );
        loadImage(p.image);
      }
    }).then((fn) => (unlisten = fn));

    return () => {
      cancelled = true;
      clearInterval(timer);
      unlisten?.();
    };
  }, [upsert, loadImage]);

  // 缓存回收：任务被删除（或因超过 MAX_TASKS 被裁掉）后，它的缩略图 data-url
  // 不该继续留在内存里。b64Cache 是只增不减的 ref，imgUrls 是同样只累积的 state，
  // 长时间运行 / 大量出图会一直涨内存。
  useEffect(() => {
    const alive = new Set<string>();
    for (const t of tasks) for (const img of t.images) alive.add(img.thumb_path || img.path);
    // b64Cache 是普通 ref，同步清理即可，不需要触发渲染
    for (const k of Array.from(b64Cache.current.keys())) {
      if (!alive.has(k)) b64Cache.current.delete(k);
    }
    setImgUrls((prev) => {
      const keys = Object.keys(prev);
      if (keys.every((k) => alive.has(k))) return prev; // 无变化就原样返回，避免多余渲染
      const next: Record<string, string> = {};
      for (const k of keys) if (alive.has(k)) next[k] = prev[k];
      return next;
    });
  }, [tasks]);

  const cancel = useCallback((taskId: string) => api.generateCancel(taskId), []);

  const remove = useCallback(async (taskId: string) => {
    await api.taskDelete(taskId);
    setTasks((prev) => prev.filter((t) => t.id !== taskId));
  }, []);

  return { tasks, imgUrls, upsert, cancel, remove };
}

export default function TaskPanel({
  tasks,
  imgUrls,
  onCancel,
  onDelete,
  onRetry,
}: {
  tasks: TaskView[];
  imgUrls: Record<string, string>;
  onCancel: (id: string) => void;
  onDelete: (id: string) => void;
  onRetry: (t: TaskView) => void;
}) {
  if (tasks.length === 0) {
    return <p className="px-4 py-6 text-center text-xs text-neutral-400">还没有任务，先生成一张试试</p>;
  }
  return (
    <div className="space-y-2 p-3">
      {tasks.map((t) => {
        const st = STATUS_LABEL[t.status] ?? { text: t.status, cls: "bg-neutral-100 text-neutral-500" };
        const active = t.status === "running" || t.status === "pending";
        return (
          <div key={t.id} className="rounded-lg border border-neutral-200 p-2.5 dark:border-neutral-800">
            <div className="flex items-center justify-between gap-2">
              <span className={`rounded px-1.5 py-0.5 text-[11px] ${st.cls}`}>{st.text}</span>
              <span className="text-[11px] text-neutral-400">
                {t.done}/{t.total}
              </span>
              {active && (
                <button className="text-[11px] text-red-500 hover:underline" onClick={() => onCancel(t.id)}>
                  取消
                </button>
              )}
              {/* refCount > 0 是图生图：我们没存参考图本体，重试会退化成文生图，
                  宁可不给入口，也不要给一个结果不一样的"重试" */}
              {!active && t.refCount === 0 && RETRYABLE_STATUS.has(t.status) && (
                <button className="text-[11px] text-orange-600 hover:underline" onClick={() => onRetry(t)}>
                  重试
                </button>
              )}
              {!active && (
                <button
                  className="text-[11px] text-neutral-400 hover:text-red-500 hover:underline"
                  onClick={() => onDelete(t.id)}
                >
                  删除
                </button>
              )}
            </div>
            {t.prompt && (
              <p className="mt-1 break-words text-[11px] leading-snug text-neutral-500 line-clamp-2 dark:text-neutral-400">
                {shorten(t.prompt)}
              </p>
            )}
            {t.errors.length > 0 && (
              <p className="mt-1 max-h-24 overflow-y-auto whitespace-pre-wrap break-words text-[11px] leading-snug text-red-500">
                {t.errors[0]}
              </p>
            )}
            {t.images.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-1.5">
                {t.images.map((img) => {
                  const key = img.thumb_path || img.path;
                  const url = imgUrls[key];
                  return url ? (
                    <img key={img.id} src={url} className="h-16 w-16 rounded object-cover" alt="" />
                  ) : (
                    <div key={img.id} className="h-16 w-16 animate-pulse rounded bg-neutral-100 dark:bg-neutral-800" />
                  );
                })}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
