import { useCallback, useEffect, useRef, useState, type MouseEvent } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { useAddAsRef } from "../hooks/useAddAsRef";
import { useCopyImage } from "../hooks/useCopyImage";
import { api } from "../ipc";
import { errorMessage } from "../lib/error";
import { useNav } from "../stores/nav";
import { usePrefill } from "../stores/prefill";
import { useToast } from "../stores/toast";
import ContextMenu from "../ui/ContextMenu";
import PromptDialog from "../ui/PromptDialog";
import { confirmDelete } from "../ui/dialogs";
import { btnCls, btnToggleCls, inputCls } from "../ui/controls";
import type { GalleryItem, ModelItem } from "../types";

export default function GalleryView() {
  const [items, setItems] = useState<GalleryItem[]>([]);
  const [q, setQ] = useState("");
  const [favOnly, setFavOnly] = useState(false);
  const [modelFilter, setModelFilter] = useState("");
  const [models, setModels] = useState<ModelItem[]>([]);
  const [selectMode, setSelectMode] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [compare, setCompare] = useState<GalleryItem[] | null>(null);
  const [compareUrls, setCompareUrls] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  // 预览记图片 id 而不是下标：轮询每 5 秒整批替换 items，下标会在增删后指向另一张图。
  const [preview, setPreview] = useState<string | null>(null);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [ctx, setCtx] = useState<{ x: number; y: number; id: string } | null>(null);
  const [presetTarget, setPresetTarget] = useState<GalleryItem | null>(null);
  const urlCache = useRef<Map<string, string>>(new Map());
  // 原图单独一份有上限的缓存：预览/对比只看手边几张，没必要把每张原图都永久
  // 留在内存里（一张大图的 data URL 就是几十 MB）。
  const fullCache = useRef<Map<string, string>>(new Map());
  const FULL_CACHE_LIMIT = 6;
  const sendPrefill = usePrefill((s) => s.send);
  const setTab = useNav((s) => s.setTab);

  const setNoticeToast = useToast((s) => s.show);
  const addAsRefs = useAddAsRef(setNoticeToast);
  const copyImage = useCopyImage(setNoticeToast);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await api.imageList({ q, favoriteOnly: favOnly, model: modelFilter || undefined, limit: 120 }));
    } finally {
      setLoading(false);
    }
  }, [q, favOnly, modelFilter]);

  useEffect(() => {
    api.modelList().then(setModels).catch(() => {});
  }, []);

  useEffect(() => {
    const t = setTimeout(reload, q ? 300 : 0);
    return () => clearTimeout(t);
  }, [reload]);

  // 静默轮询：外部进程（无头 CLI）直接往同一个库写图，发不出前端事件，
  // 不轮询的话 CLI 生成的图不会出现在这里。只在 id/收藏状态变化时更新，
  // 既不会打断当前浏览，也不会触发 loading 闪烁。
  useEffect(() => {
    const timer = setInterval(async () => {
      try {
        const next = await api.imageList({ q, favoriteOnly: favOnly, model: modelFilter || undefined, limit: 120 });
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
  }, [q, favOnly, modelFilter]);

  const thumbUrl = useCallback(
    async (path: string) => {
      if (urlCache.current.has(path)) return urlCache.current.get(path)!;
      const url = await api.imageReadB64(path);
      urlCache.current.set(path, url);
      return url;
    },
    [],
  );

  // 原图走 LRU：命中移到队尾，超上限淘汰最久未用的那张
  const fullUrl = useCallback(
    async (path: string) => {
      const hit = fullCache.current.get(path);
      if (hit) {
        fullCache.current.delete(path);
        fullCache.current.set(path, hit);
        return hit;
      }
      const url = await api.imageReadB64(path);
      fullCache.current.set(path, url);
      while (fullCache.current.size > FULL_CACHE_LIMIT) {
        const oldest = fullCache.current.keys().next().value;
        if (oldest === undefined) break;
        fullCache.current.delete(oldest);
      }
      return url;
    },
    [FULL_CACHE_LIMIT],
  );

  const previewSeq = useRef(0);

  useEffect(() => {
    let alive = true;
    // 串行 for-await 会让 120 张图变成 120 次往返，首屏明显慢。
    // 改成小批并发：既并行起来，又不把上百个 IPC 一次全压过去。
    const pending = items
      .map((it) => it.thumb_path || it.path)
      .filter((key) => !urlCache.current.has(key));
    void (async () => {
      for (let i = 0; i < pending.length; i += 8) {
        if (!alive) return;
        const batch = pending.slice(i, i + 8);
        const loaded = await Promise.all(
          batch.map(async (key) => {
            try {
              return [key, await thumbUrl(key)] as const;
            } catch {
              return null;
            }
          }),
        );
        if (!alive) return;
        for (const entry of loaded) {
          if (entry) urlCache.current.set(entry[0], entry[1]);
        }
        setItems((prev) => [...prev]);
      }
    })();
    return () => {
      alive = false;
    };
  }, [items, thumbUrl]);

  const openPreview = useCallback(
    async (it: GalleryItem) => {
      setPreview(it.id);
      const seq = ++previewSeq.current;
      // 先用已缓存的缩略图秒开，再后台读原图替换；seq 防止快速切换时旧图盖掉新图
      setPreviewUrl(urlCache.current.get(it.thumb_path || it.path) ?? null);
      try {
        const full = await fullUrl(it.path);
        if (seq === previewSeq.current) setPreviewUrl(full);
      } catch {
        // 原图读失败就保留缩略图
      }
    },
    [fullUrl],
  );

  // 预览标题/按钮都以 id 反查当前项：轮询换掉 items 后仍指向同一张图，图没了就自动关闭
  const previewIdx = preview ? items.findIndex((it) => it.id === preview) : -1;
  const previewItem = previewIdx >= 0 ? items[previewIdx] : null;

  // 预览层：点容器自身的空白处关闭；按钮/图片/MJ 区域点在子元素上，不误关。
  const closeOnBackdrop = (e: MouseEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget) return;
    if (compare) setCompare(null);
    else setPreview(null);
  };

  useEffect(() => {
    if (!previewItem) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setPreview(null);
      if (e.key === "ArrowLeft" && previewIdx > 0) openPreview(items[previewIdx - 1]);
      if (e.key === "ArrowRight" && previewIdx < items.length - 1) openPreview(items[previewIdx + 1]);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [previewItem, previewIdx, items, openPreview]);

  const toggleFav = async (it: GalleryItem) => {
    await api.imageSetFavorite(it.id, !it.favorite);
    setItems((prev) => prev.map((x) => (x.id === it.id ? { ...x, favorite: !x.favorite } : x)));
  };

  const remove = async (it: GalleryItem) => {
    if (!(await confirmDelete("删除这张图？文件也会一并删除。"))) return;
    await api.imageDelete(it.id);
    setItems((prev) => prev.filter((x) => x.id !== it.id));
    setPreview(null);
  };

  const refilling = (it: GalleryItem) => {
    sendPrefill({
      prompt: it.prompt,
      size: it.params?.size,
      quality: it.params?.quality,
    });
    // prefill 由工作台挂载时的 effect 消费；不切过去就看不到回填结果
    setTab("workbench");
  };

  const addAsRef = async (it: GalleryItem) => {
    const { added, duplicates } = await addAsRefs([{ path: it.path, id: it.id }]);
    // 不在这里提示成功：切到工作台会立刻卸载本视图，提示只会闪一下。
    // 参考图缩略图出现在工作台，本身就是结果。
    if (added > 0 || duplicates > 0) setTab("workbench");
  };

  // 继续生成：继承这张图的提示词/参数，并把原图作为参考图，跳回工作台再出一版。
  // 先加参考图、成功后再回填提示词——满 4 张时只提示，不改动用户当前的提示词。
  const continueGenerating = async (it: GalleryItem) => {
    const { added, duplicates } = await addAsRefs([{ path: it.path, id: it.id }]);
    // 图已在参考里（重复）也要回填参数；只有真满额才不动用户的提示词
    if (added === 0 && duplicates === 0) return;
    sendPrefill({
      prompt: it.prompt,
      size: it.params?.size,
      quality: it.params?.quality,
    });
    setTab("workbench");
  };

  // 导出当前筛选条件下的图记录为 JSONL。字段与「导入批量」一致，
  // 导出的文件能原样导回去再跑一遍。
  const exportJsonl = async () => {
    try {
      const path = await save({
        defaultPath: "xiic-gallery.jsonl",
        filters: [{ name: "JSONL", extensions: ["jsonl"] }],
      });
      if (!path) return;
      const n = await api.galleryExportJsonl(path, { q, favoriteOnly: favOnly, model: modelFilter || undefined });
      setNoticeToast(`已导出 ${n} 张图的记录`, "success");
    } catch (err) {
      setNoticeToast(errorMessage(err), "error");
    }
  };

  // 打包导出：图片本体 + 记录 + 数据库快照 + 供应商配置。Key 不导出，换机器需重填。
  const exportBundle = async () => {
    try {
      const path = await save({
        defaultPath: "xiic-gallery-bundle.zip",
        filters: [{ name: "ZIP", extensions: ["zip"] }],
      });
      if (!path) return;
      const r = await api.galleryExportBundle(path, { q, favoriteOnly: favOnly, model: modelFilter || undefined });
      setNoticeToast(`已打包 ${r.images} 张图 + ${r.providers} 家供应商（不含 API Key）`, "success");
    } catch (err) {
      setNoticeToast(errorMessage(err), "error");
    }
  };

  // ---- 批量操作：收藏 / 取消收藏 / 删除（淘汰）/ 并排对比 ----
  const toggleSelect = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const chosenItems = () => items.filter((it) => selected.has(it.id));

  const batchFav = async (favorite: boolean) => {
    const chosen = chosenItems();
    if (chosen.length === 0) return;
    for (const it of chosen) {
      try {
        await api.imageSetFavorite(it.id, favorite);
      } catch {
        // 单张失败不影响其余
      }
    }
    setNoticeToast(`已${favorite ? "收藏" : "取消收藏"} ${chosen.length} 张`, "success");
    setSelected(new Set());
    await reload();
  };

  const batchDelete = async () => {
    const chosen = chosenItems();
    if (chosen.length === 0) return;
    if (!(await confirmDelete(`删除选中的 ${chosen.length} 张图？文件也会一并删除。`))) return;
    let n = 0;
    for (const it of chosen) {
      try {
        await api.imageDelete(it.id);
        n += 1;
      } catch {
        // 单张失败不影响其余
      }
    }
    setNoticeToast(`已删除 ${n} 张`, "success");
    setSelected(new Set());
    await reload();
  };

  const openCompare = async () => {
    const chosen = chosenItems();
    if (chosen.length < 2) {
      setNoticeToast("至少选 2 张才能对比", "warning");
      return;
    }
    setPreview(null);
    setCompare(chosen);
    setCompareUrls({});
    const urls: Record<string, string> = {};
    await Promise.all(
      chosen.map(async (it) => {
        try {
          urls[it.id] = await thumbUrl(it.thumb_path || it.path);
        } catch {
          // 读不到的图留空
        }
      }),
    );
    setCompareUrls(urls);
  };

  // 把这张图的提示词与参数存成配方，之后可在工作台一键套用。
  // 命名走应用内弹窗，不用浏览器原生 prompt（暗色主题下是白框）。
  const saveAsPreset = (it: GalleryItem) => setPresetTarget(it);

  const submitPreset = async (name: string) => {
    const it = presetTarget;
    setPresetTarget(null);
    if (!it) return;
    try {
      await api.presetSave({
        name: name.trim(),
        prompt: it.prompt,
        size: it.params?.size ?? null,
        quality: it.params?.quality ?? null,
        seed: it.params?.seed ?? null,
        provider_id: it.provider_id ?? null,
        model_id: it.model_id ?? null,
      });
      setNoticeToast(`已存为配方「${name}」`, "success");
    } catch (err) {
      setNoticeToast(errorMessage(err), "error");
    }
  };

  // 右键菜单点外部关闭；画廊滚动时也要关，避免菜单悬在错误的位置
  useEffect(() => {
    if (!ctx) return;
    const close = () => setCtx(null);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("click", close);
    window.addEventListener("resize", close);
    window.addEventListener("keydown", onKey);
    const scroller = document.getElementById("gallery-scroll");
    scroller?.addEventListener("scroll", close, true);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("resize", close);
      window.removeEventListener("keydown", onKey);
      scroller?.removeEventListener("scroll", close, true);
    };
  }, [ctx]);

  // 右键菜单五个入口共用同一份查找，不必每项各查一遍
  const ctxItem = ctx ? items.find((it) => it.id === ctx.id) : undefined;

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-3 border-b border-neutral-200 px-6 py-3 dark:border-neutral-800">
        {selectMode ? (
          <>
            <button className={btnCls} onClick={() => { setSelectMode(false); setSelected(new Set()); }}>退出批量</button>
            <span className="text-xs text-neutral-500">已选 {selected.size} 张</span>
            <button className={btnCls} onClick={() => setSelected(new Set(items.map((it) => it.id)))}>全选</button>
            <button className={btnCls} onClick={() => void openCompare()}>并排对比</button>
            <button className={btnCls} onClick={() => void batchFav(true)}>批量收藏</button>
            <button className={btnCls} onClick={() => void batchFav(false)}>取消收藏</button>
            <button className={btnCls} onClick={() => void batchDelete()}>批量删除</button>
          </>
        ) : (
          <>
        <h1 className="text-lg font-medium">画廊</h1>
        <input className={`${inputCls} w-64!`} placeholder="搜索提示词…" value={q} onChange={(e) => setQ(e.target.value)} />
        <select
          className={`${inputCls} w-40!`}
          value={modelFilter}
          onChange={(e) => setModelFilter(e.target.value)}
          title="按生成模型筛选"
        >
          <option value="">全部模型</option>
          {models.map((m) => (
            <option key={m.id} value={m.model_name}>{m.label || m.model_name}</option>
          ))}
        </select>
        <button
          className={btnToggleCls(favOnly)}
          onClick={() => setFavOnly(!favOnly)}
        >
          ★ 收藏
        </button>
        <button
          className={btnCls}
          onClick={() => void exportJsonl()}
          title="把当前筛选出的图导出为 JSONL，格式与「导入批量」一致，可原样导回"
        >
          导出 JSONL
        </button>
        <button
          className={btnCls}
          onClick={() => void exportBundle()}
          title="打包当前筛选出的图片本体、JSONL 记录、数据库快照和供应商配置（不含 API Key，换机器需重填）"
        >
          打包导出
        </button>
        <button className={btnCls} onClick={() => { setSelectMode(true); setSelected(new Set()); }}>批量</button>
          </>
        )}
        <span className="ml-auto text-xs text-neutral-400">{items.length} 张</span>
      </div>

      <div id="gallery-scroll" className="flex-1 overflow-y-auto p-5">
        {items.length === 0 && !loading && (
          <p className="py-16 text-center text-sm text-neutral-400">空空如也，先去工作台生成几张</p>
        )}
        <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-3">
          {items.map((it) => {
            const url = urlCache.current.get(it.thumb_path || it.path);
            return (
              <div
                key={it.id}
                className={`group relative overflow-hidden rounded-lg border ${selected.has(it.id) ? "border-orange-500" : "border-neutral-200 dark:border-neutral-800"}`}
                onContextMenu={selectMode ? undefined : (e) => {
                  e.preventDefault();
                  setCtx({ x: e.clientX, y: e.clientY, id: it.id });
                }}
                onClick={selectMode ? () => toggleSelect(it.id) : undefined}
              >
                {selectMode && (
                  <div className={`absolute left-1.5 top-1.5 z-10 flex h-5 w-5 items-center justify-center rounded border text-xs ${selected.has(it.id) ? "border-orange-600 bg-orange-600 text-white" : "border-white bg-black/30 text-transparent"}`}>
                    ✓
                  </div>
                )}
                <button className={`block w-full ${selectMode ? "" : "cursor-zoom-in"}`} onClick={selectMode ? undefined : () => openPreview(it)}>
                  {url ? (
                    <img src={url} className="aspect-square w-full object-cover transition-transform group-hover:scale-[1.03]" alt="" />
                  ) : (
                    <div className="aspect-square w-full animate-pulse bg-neutral-100 dark:bg-neutral-800" />
                  )}
                </button>
                <button
                  className={`absolute right-1.5 top-1.5 text-lg leading-none drop-shadow ${it.favorite ? "text-amber-400" : "text-white/70 hover:text-amber-300"}`}
                  onClick={selectMode ? undefined : () => toggleFav(it)}
                >
                  {it.favorite ? "★" : "☆"}
                </button>
                <div className="truncate px-2 py-1.5 text-[11px] text-neutral-500">{it.prompt}</div>
              </div>
            );
          })}
        </div>
      </div>

      {/* 右键菜单 */}
      {ctx && (
        <ContextMenu
          x={ctx.x}
          y={ctx.y}
          items={[
            {
              label: "加入参考图",
              onClick: () => {
                const item = ctxItem;
                setCtx(null);
                if (item) void addAsRef(item);
              },
            },
            {
              label: "继续生成",
              onClick: () => {
                const item = ctxItem;
                setCtx(null);
                if (item) void continueGenerating(item);
              },
            },
            {
              label: "存为配方",
              onClick: () => {
                const item = ctxItem;
                setCtx(null);
                if (item) saveAsPreset(item);
              },
            },
            {
              label: "复制图片",
              onClick: () => {
                const item = ctxItem;
                setCtx(null);
                if (item) void copyImage(item.path);
              },
            },
            {
              label: "在 Finder 打开",
              onClick: () => {
                const item = ctxItem;
                setCtx(null);
                if (item) void api.imageReveal(item.id);
              },
            },
          ]}
        />
      )}

      <PromptDialog
        open={presetTarget !== null}
        title="存为配方"
        placeholder="配方名"
        defaultValue={presetTarget?.prompt.slice(0, 20) || "未命名配方"}
        confirmLabel="保存"
        onSubmit={(name) => void submitPreset(name)}
        onCancel={() => setPresetTarget(null)}
      />

      {/* 预览 */}
      {previewItem && (
        <div className="fixed inset-0 z-50 flex flex-col bg-black/85 p-4" onClick={closeOnBackdrop}>
          <div className="flex items-center justify-between px-2 pb-2 text-sm text-white/90" onClick={closeOnBackdrop}>
            <div className="flex items-center gap-2">
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => previewIdx > 0 && openPreview(items[previewIdx - 1])}>←</button>
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => previewIdx < items.length - 1 && openPreview(items[previewIdx + 1])}>→</button>
              <span className="text-xs text-white/60">{previewIdx + 1} / {items.length}</span>
            </div>
            <div className="flex items-center gap-2">
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => toggleFav(previewItem)}>
                {previewItem.favorite ? "★ 已收藏" : "☆ 收藏"}
              </button>
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => refilling(previewItem)}>回填参数</button>
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => saveAsPreset(previewItem)}>存为配方</button>
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => void addAsRef(previewItem)}>加入参考图</button>
              <button className="rounded bg-orange-600 px-3 py-1.5 hover:bg-orange-500" onClick={() => void continueGenerating(previewItem)}>继续生成</button>
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => api.imageReveal(previewItem.id)}>在 Finder 显示</button>
              <button className="rounded bg-red-500/80 px-3 py-1.5 hover:bg-red-500" onClick={() => remove(previewItem)}>删除</button>
              <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => setPreview(null)}>×</button>
            </div>
          </div>
          <div className="flex min-h-0 flex-1 items-center justify-center" onClick={closeOnBackdrop}>
            {previewUrl ? (
              <img src={previewUrl} className="max-h-full max-w-full rounded-lg object-contain" alt="" onClick={(e) => e.stopPropagation()} />
            ) : (
              <div className="h-16 w-16 animate-pulse rounded bg-white/10" />
            )}
          </div>
          <div className="mx-auto max-w-3xl px-2 pt-2 text-center text-xs text-white/70" onClick={closeOnBackdrop}>
            <p className="line-clamp-2">{previewItem.prompt}</p>
            <p className="mt-1 text-white/40">
              {previewItem.model} · {previewItem.width}×{previewItem.height} · {previewItem.params?.size ?? ""} {previewItem.params?.quality ?? ""}
            </p>
            {previewItem.params?.mj_task_id && (
              <div className="mt-2 flex flex-wrap items-center justify-center gap-1" onClick={(e) => e.stopPropagation()}>
                <span className="text-white/50">MJ 变换：</span>
                {["U1", "U2", "U3", "U4", "V1", "V2", "V3", "V4"].map((cmd) => (
                  <button key={cmd}
                    className="rounded bg-white/10 px-2 py-1 text-[11px] text-white hover:bg-white/25"
                    onClick={async () => {
                      try {
                        await api.mjAction(previewItem.id, cmd);
                        setNoticeToast(`已提交 ${cmd}，完成后自动出现在画廊`, "success");
                      } catch (err) {
                        setNoticeToast(errorMessage(err), "error");
                      }
                    }}>{cmd}</button>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {/* 并排对比 */}
      {compare && (
        <div className="fixed inset-0 z-50 flex flex-col bg-black/90 p-6" onClick={closeOnBackdrop}>
          <div className="flex items-center justify-between pb-3 text-sm text-white/90" onClick={closeOnBackdrop}>
            <span>并排对比 · {compare.length} 张</span>
            <button className="rounded bg-white/10 px-3 py-1.5 hover:bg-white/20" onClick={() => setCompare(null)}>关闭</button>
          </div>
          <div className="grid min-h-0 flex-1 grid-cols-[repeat(auto-fit,minmax(240px,1fr))] gap-3 overflow-auto">
            {compare.map((it) => (
              <div key={it.id} className="flex min-h-0 flex-col rounded-lg bg-white/5 p-2" onClick={(e) => e.stopPropagation()}>
                <div className="flex min-h-0 flex-1 items-center justify-center overflow-hidden">
                  {compareUrls[it.id] ? (
                    <img src={compareUrls[it.id]} className="max-h-full max-w-full object-contain" alt="" />
                  ) : (
                    <div className="h-16 w-16 animate-pulse rounded bg-white/10" />
                  )}
                </div>
                <p className="mt-2 line-clamp-2 text-[11px] text-white/70">{it.prompt}</p>
                <p className="text-[10px] text-white/40">{it.model} · {it.params?.size ?? ""}</p>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
