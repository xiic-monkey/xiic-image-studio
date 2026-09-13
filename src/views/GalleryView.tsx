import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../ipc";
import { usePrefill } from "../stores/prefill";
import type { GalleryItem } from "../types";

export default function GalleryView() {
  const [items, setItems] = useState<GalleryItem[]>([]);
  const [q, setQ] = useState("");
  const [favOnly, setFavOnly] = useState(false);
  const [loading, setLoading] = useState(false);
  const [preview, setPreview] = useState<number>(-1);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const urlCache = useRef<Map<string, string>>(new Map());
  const sendPrefill = usePrefill((s) => s.send);

  const setNoticeToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 4000);
  };

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await api.imageList({ q, favoriteOnly: favOnly, limit: 120 }));
    } finally {
      setLoading(false);
    }
  }, [q, favOnly]);

  useEffect(() => {
    const t = setTimeout(reload, q ? 300 : 0);
    return () => clearTimeout(t);
  }, [reload]);

  // 静默轮询：xiic-image-mcp（外部进程）直接往同一个库写图，发不出前端事件，
  // 不轮询的话 MCP 生成的图不会出现在这里。只在 id/收藏状态变化时更新，
  // 既不会打断当前浏览，也不会触发 loading 闪烁。
  useEffect(() => {
    const timer = setInterval(async () => {
      try {
        const next = await api.imageList({ q, favoriteOnly: favOnly, limit: 120 });
        setItems((prev) =>
          prev.length === next.length &&
          prev.every((it, i) => it.id === next[i].id && it.favorite === next[i].favorite)
            ? prev
            : next,
        );
      } catch {
        // 轮询失败无所谓，下次再来
      }
    }, 5000);
    return () => clearInterval(timer);
  }, [q, favOnly]);

  const thumbUrl = useCallback(
    async (path: string) => {
      if (urlCache.current.has(path)) return urlCache.current.get(path)!;
      const url = await api.imageReadB64(path);
      urlCache.current.set(path, url);
      return url;
    },
    [],
  );

  useEffect(() => {
    let alive = true;
    (async () => {
      for (const it of items) {
        if (!alive) return;
        const key = it.thumb_path || it.path;
        if (!urlCache.current.has(key)) {
          const url = await thumbUrl(key);
          setItems((prev) => [...prev]);
          urlCache.current.set(key, url);
        }
      }
    })();
    return () => {
      alive = false;
    };
  }, [items, thumbUrl]);

  const openPreview = useCallback(
    async (idx: number) => {
      setPreview(idx);
      setPreviewUrl(null);
      const it = items[idx];
      if (it) setPreviewUrl(await thumbUrl(it.path));
    },
    [items, thumbUrl],
  );

  useEffect(() => {
    if (preview < 0) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setPreview(-1);
      if (e.key === "ArrowLeft" && preview > 0) openPreview(preview - 1);
      if (e.key === "ArrowRight" && preview < items.length - 1) openPreview(preview + 1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [preview, items.length, openPreview]);

  const toggleFav = async (it: GalleryItem) => {
    await api.imageSetFavorite(it.id, !it.favorite);
    setItems((prev) => prev.map((x) => (x.id === it.id ? { ...x, favorite: !x.favorite } : x)));
  };

  const remove = async (it: GalleryItem) => {
    if (!confirm("删除这张图？文件也会一并删除。")) return;
    await api.imageDelete(it.id);
    setItems((prev) => prev.filter((x) => x.id !== it.id));
    setPreview(-1);
  };

  const refilling = async (it: GalleryItem) => {
    sendPrefill({
      prompt: it.prompt,
      size: it.params?.size,
      quality: it.params?.quality,
    });
  };

  const inputCls =
    "rounded-md border border-neutral-300 bg-white px-3 py-2 text-sm focus:border-orange-500 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100";

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-3 border-b border-neutral-200 px-6 py-3 dark:border-neutral-800">
        <h1 className="text-lg font-medium">画廊</h1>
        <input className={`${inputCls} w-64`} placeholder="搜索提示词…" value={q} onChange={(e) => setQ(e.target.value)} />
        <button
          className={`rounded-md px-3 py-2 text-sm ${favOnly ? "bg-orange-600 text-white" : "border border-neutral-300 text-neutral-600 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-300"}`}
          onClick={() => setFavOnly(!favOnly)}
        >
          ★ 收藏
        </button>
        <span className="ml-auto text-xs text-neutral-400">{items.length} 张</span>
      </div>

      <div className="flex-1 overflow-y-auto p-5">
        {items.length === 0 && !loading && (
          <p className="py-16 text-center text-sm text-neutral-400">空空如也，先去工作台生成几张</p>
        )}
        <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-3">
          {items.map((it, i) => {
            const url = urlCache.current.get(it.thumb_path || it.path);
            return (
              <div key={it.id} className="group relative overflow-hidden rounded-lg border border-neutral-200 dark:border-neutral-800">
                <button className="block w-full cursor-zoom-in" onClick={() => openPreview(i)}>
                  {url ? (
                    <img src={url} className="aspect-square w-full object-cover transition-transform group-hover:scale-[1.03]" alt="" />
                  ) : (
                    <div className="aspect-square w-full animate-pulse bg-neutral-100 dark:bg-neutral-800" />
                  )}
                </button>
                <button
                  className={`absolute right-1.5 top-1.5 text-lg leading-none drop-shadow ${it.favorite ? "text-amber-400" : "text-white/70 hover:text-amber-300"}`}
                  onClick={() => toggleFav(it)}
                >
                  {it.favorite ? "★" : "☆"}
                </button>
                <div className="truncate px-2 py-1.5 text-[11px] text-neutral-500">{it.prompt}</div>
              </div>
            );
          })}
        </div>
      </div>

      {/* 预览 */}
      {toast && (
        <div className="fixed bottom-6 left-1/2 z-[60] -translate-x-1/2 rounded-md bg-neutral-800 px-4 py-2 text-xs text-white shadow-lg">
          {toast}
        </div>
      )}
      {preview >= 0 && items[preview] && (
        <div className="fixed inset-0 z-50 flex flex-col bg-black/85 p-4" onClick={() => setPreview(-1)}>
          <div className="flex items-center justify-between px-2 pb-2 text-sm text-white/90" onClick={(e) => e.stopPropagation()}>
            <div className="flex items-center gap-2">
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => preview > 0 && openPreview(preview - 1)}>←</button>
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => preview < items.length - 1 && openPreview(preview + 1)}>→</button>
              <span className="text-xs text-white/60">{preview + 1} / {items.length}</span>
            </div>
            <div className="flex items-center gap-2">
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => toggleFav(items[preview])}>
                {items[preview].favorite ? "★ 已收藏" : "☆ 收藏"}
              </button>
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => refilling(items[preview])}>回填参数</button>
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => api.imageReveal(items[preview].id)}>在 Finder 显示</button>
              <button className="rounded bg-red-500/80 px-3 py-1.5 hover:bg-red-500" onClick={() => remove(items[preview])}>删除</button>
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => setPreview(-1)}>×</button>
            </div>
          </div>
          <div className="flex min-h-0 flex-1 items-center justify-center" onClick={(e) => e.stopPropagation()}>
            {previewUrl ? (
              <img src={previewUrl} className="max-h-full max-w-full rounded-lg object-contain" alt="" />
            ) : (
              <div className="h-16 w-16 animate-pulse rounded bg-white/10" />
            )}
          </div>
          <div className="mx-auto max-w-3xl px-2 pt-2 text-center text-xs text-white/70" onClick={(e) => e.stopPropagation()}>
            <p className="line-clamp-2">{items[preview].prompt}</p>
            <p className="mt-1 text-white/40">
              {items[preview].model} · {items[preview].width}×{items[preview].height} · {items[preview].params?.size ?? ""} {items[preview].params?.quality ?? ""}
            </p>
            {items[preview].params?.mj_task_id && (
              <div className="mt-2 flex flex-wrap items-center justify-center gap-1" onClick={(e) => e.stopPropagation()}>
                <span className="text-white/50">MJ 变换：</span>
                {["U1", "U2", "U3", "U4", "V1", "V2", "V3", "V4"].map((cmd) => (
                  <button key={cmd}
                    className="rounded bg-white/10 px-2 py-1 text-[11px] text-white hover:bg-white/25"
                    onClick={async () => {
                      try {
                        await api.mjAction(items[preview].id, cmd);
                        setNoticeToast(`已提交 ${cmd}，完成后自动出现在画廊`);
                      } catch (err) {
                        setNoticeToast(String(err));
                      }
                    }}>{cmd}</button>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
