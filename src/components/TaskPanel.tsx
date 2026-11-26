import { useEffect, useState } from "react";
import { canRetry } from "../lib/retry.ts";
import ContextMenu, { type ContextMenuItem } from "../ui/ContextMenu";
import type { ImageMeta, TaskView } from "../types";

const STATUS_LABEL: Record<string, { text: string; cls: string }> = {
  running: { text: "生成中", cls: "bg-blue-50 text-blue-700" },
  completed: { text: "完成", cls: "bg-green-50 text-green-700" },
  completed_with_errors: { text: "部分失败", cls: "bg-amber-50 text-amber-700" },
  failed: { text: "失败", cls: "bg-red-50 text-red-700" },
  canceled: { text: "已取消", cls: "bg-neutral-100 text-neutral-500" },
};

// 把多行提示词压成一行并截断，作为任务卡上的缩写
function shorten(text: string, n = 80): string {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length > n ? flat.slice(0, n) + "\u2026" : flat;
}

export default function TaskPanel({
  tasks,
  imgUrls,
  onCancel,
  onDelete,
  onRetry,
  onAddRefs,
  onContinue,
  onCopy,
  onReveal,
}: {
  tasks: TaskView[];
  imgUrls: Record<string, string>;
  onCancel: (id: string) => void;
  onDelete: (id: string) => void;
  onRetry: (t: TaskView) => void;
  onAddRefs: (imgs: ImageMeta[]) => void;
  onContinue: (t: TaskView) => void;
  onCopy: (img: ImageMeta) => void;
  onReveal: (img: ImageMeta) => void;
}) {
  // 右键目标：整张任务卡（fromImage=false，只给继续生成）或某个缩略图
  const [ctx, setCtx] = useState<{
    x: number;
    y: number;
    task: TaskView;
    imgs: ImageMeta[];
    fromImage: boolean;
  } | null>(null);

  // 与画廊一致：点外部、缩放、Escape 都关菜单
  useEffect(() => {
    if (!ctx) return;
    const close = () => setCtx(null);
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") close(); };
    window.addEventListener("click", close);
    window.addEventListener("resize", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("resize", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [ctx]);

  // 图片才有「加入参考图 / 复制图片 / 在 Finder 打开」，任务卡只有「继续生成」
  const buildItems = (c: NonNullable<typeof ctx>): ContextMenuItem[] => [
    ...(c.fromImage
      ? [{ label: "加入参考图", onClick: () => { setCtx(null); onAddRefs(c.imgs); } }]
      : []),
    { label: "继续生成", onClick: () => { setCtx(null); onContinue(c.task); } },
    ...(c.fromImage
      ? [
          { label: "复制图片", onClick: () => { setCtx(null); onCopy(c.imgs[0]); } },
          { label: "在 Finder 打开", onClick: () => { setCtx(null); onReveal(c.imgs[0]); } },
        ]
      : []),
  ];

  if (tasks.length === 0) {
    return <p className="px-4 py-6 text-center text-xs text-neutral-400">还没有任务，先生成一张试试</p>;
  }
  return (
    <>
      <div className="space-y-2 p-3">
        {tasks.map((t) => {
          const st = STATUS_LABEL[t.status] ?? { text: t.status, cls: "bg-neutral-100 text-neutral-500" };
          const active = t.status === "running" || t.status === "pending";
          return (
            <div
              key={t.id}
              className="rounded-lg border border-neutral-200 p-2.5 dark:border-neutral-800"
              onContextMenu={(e) => {
                e.preventDefault();
                setCtx({ x: e.clientX, y: e.clientY, task: t, imgs: t.images, fromImage: false });
              }}
            >
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
                {/* 图生图只要存了参考图快照（refPaths 非空）也能原样重试；
                    canRetry 会挡住旧任务里没快照的那批，避免退化成文生图 */}
                {!active && canRetry(t) && (
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
                      <img
                        key={img.id}
                        src={url}
                        className="h-16 w-16 rounded object-cover"
                        alt=""
                        onContextMenu={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          setCtx({ x: e.clientX, y: e.clientY, task: t, imgs: [img], fromImage: true });
                        }}
                      />
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

      {ctx && <ContextMenu x={ctx.x} y={ctx.y} items={buildItems(ctx)} />}
    </>
  );
}
