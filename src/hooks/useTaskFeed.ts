import { useCallback, useEffect, useRef, useState } from "react";
import { api, onTaskProgress } from "../ipc";
import { signature } from "../lib/task-signature.ts";
import type { ImageMeta, TaskProgress, TaskUpsert, TaskView } from "../types";

/** 任务列表保留上限。初次加载是 30 条，运行中 upsert 出来的也必须裁到这个数，
 *  否则长跑时列表只增不减，配套的缩略图缓存也就永远回收不了。 */
const MAX_TASKS = 30;

export function useTaskFeed() {
  const [tasks, setTasks] = useState<TaskView[]>([]);
  const b64Cache = useRef<Map<string, string>>(new Map());
  const [imgUrls, setImgUrls] = useState<Record<string, string>>({});
  // 卸载后所有在途的读图 / 轮询 / 事件回调都必须停手，否则会对已卸载组件 setState，
  // 且 tauri 的监听器会一直挂着（切 tab 往返一次就多一份）。
  const cancelled = useRef(false);

  const loadImage = useCallback(async (img: ImageMeta) => {
    const key = img.thumb_path || img.path;
    if (b64Cache.current.has(key)) return;
    b64Cache.current.set(key, "");
    try {
      const url = await api.imageReadB64(key);
      if (cancelled.current) {
        b64Cache.current.delete(key);
        return;
      }
      b64Cache.current.set(key, url);
      setImgUrls((prev) => ({ ...prev, [key]: url }));
    } catch {
      b64Cache.current.delete(key);
    }
  }, []);

  const upsert = useCallback((p: TaskUpsert) => {
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
              seed: null,
              sessionId: null,
            }
          : prev[idx];
      // error 来自运行中的进度事件，并入 errors 数组并去重
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
    cancelled.current = false;
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const refresh = async () => {
      let items: Awaited<ReturnType<typeof api.taskList>>;
      try {
        items = await api.taskList(30);
      } catch {
        return;
      }
      if (cancelled.current) return;
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
        seed: t.seed ?? null,
        sessionId: t.session_id ?? null,
      }));
      // 只在任务集合真的变了才 setState：轮询每 3s 一次，
      // 无脑覆盖会让整个列表（含缩略图）每 3 秒重渲染一次。
      setTasks((prev) => (signature(prev) === signature(views) ? prev : views));
      // 回填历史任务的缩略图：实时进度只推"新生成"的图，不补这一步的话
      // 刷新窗口后老任务卡永远是一片空白（只有 prompt 文字）。
      views.forEach((v) => v.images.forEach((img) => void loadImage(img)));
    };

    void refresh();
    // 轮询的必要性：外部进程（--mcp 无头模式）直接往同一个 SQLite 写任务，
    // 它发不出 tauri 事件，光靠 onTaskProgress 永远发现不了这些任务。
    const timer = setInterval(refresh, 3000);

    void onTaskProgress((p: TaskProgress) => {
      if (cancelled.current) return;
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
        void loadImage(p.image);
      }
    })
      .then((fn) => {
        // 注册是异步的：cleanup 先跑时 unlisten 还没赋值，必须在这里立刻退订，
        // 否则监听泄漏且回调会继续写已卸载组件的状态。
        if (disposed || cancelled.current) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch(() => {
        // 监听注册失败不应打断轮询
      });

    return () => {
      cancelled.current = true;
      disposed = true;
      clearInterval(timer);
      unlisten?.();
    };
  }, [upsert, loadImage]);

  // 缓存回收：任务被删除（或因超过 MAX_TASKS 被裁掉）后，它的缩略图 data-url
  // 不该继续留在内存里。b64Cache 是只增不减的 ref，imgUrls 是同样只累积的 state。
  useEffect(() => {
    const alive = new Set<string>();
    for (const t of tasks) for (const img of t.images) alive.add(img.thumb_path || img.path);
    for (const k of Array.from(b64Cache.current.keys())) {
      if (!alive.has(k)) b64Cache.current.delete(k);
    }
    setImgUrls((prev) => {
      const keys = Object.keys(prev);
      if (keys.every((k) => alive.has(k))) return prev;
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
