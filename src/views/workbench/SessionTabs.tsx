import { useEffect, useRef, useState } from "react";
import type { SessionItem } from "../../types";
import SessionContextMenu from "./SessionContextMenu";

interface Props {
  sessions: SessionItem[];
  sessionId: string;
  onSelect: (id: string) => void;
  onNew: () => void;
  onDelete: (id: string) => void;
  onRename: (id: string, name: string) => void;
}

/** 会话 tab 行：tabs 横向滚动，+ 固定在行尾；sticky 吸顶，随左栏滚动保持可见。 */
export default function SessionTabs({ sessions, sessionId, onSelect, onNew, onDelete, onRename }: Props) {
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameVal, setRenameVal] = useState("");
  const renamingRef = useRef<string | null>(null);
  const tabsRef = useRef<HTMLDivElement>(null);
  const prevCount = useRef(sessions.length);

  // 新 tab 在末尾（列表按创建时间升序），渲染提交后把 tab 条滚到最右
  useEffect(() => {
    if (sessions.length > prevCount.current) {
      requestAnimationFrame(() => requestAnimationFrame(() => {
        const el = tabsRef.current;
        if (el) el.scrollLeft = el.scrollWidth;
      }));
    }
    prevCount.current = sessions.length;
  }, [sessions.length]);

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") setMenu(null); };
    window.addEventListener("mousedown", close);
    window.addEventListener("resize", close);
    document.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("resize", close);
      document.removeEventListener("keydown", onKey);
    };
  }, [menu]);

  const startRename = (id: string) => {
    renamingRef.current = id;
    setRenameVal(sessions.find((s) => s.id === id)?.name ?? "");
    setRenamingId(id);
  };

  // Enter 与 blur 会连着触发两次；ref 守卫保证只写一次库
  const commitRename = () => {
    const id = renamingRef.current;
    if (!id) return;
    renamingRef.current = null;
    setRenamingId(null);
    const name = renameVal.trim();
    if (name) void onRename(id, name);
  };

  const cancelRename = () => {
    renamingRef.current = null;
    setRenamingId(null);
  };

  return (
    <>
      <div className="sticky top-0 z-10 flex items-center gap-1 border-b border-neutral-200 bg-white pb-2 pt-1 dark:border-neutral-800 dark:bg-neutral-950">
        <div ref={tabsRef} className="no-scrollbar -mx-1 flex min-w-0 flex-1 items-center gap-1 overflow-x-auto px-1 py-0.5">
          {sessions.map((s) => {
            const active = s.id === sessionId;
            const isRenaming = renamingId === s.id;
            return (
              <div
                key={s.id}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setMenu({
                    x: Math.min(e.clientX, window.innerWidth - 140),
                    y: Math.min(e.clientY, window.innerHeight - 90),
                    id: s.id,
                  });
                }}
                onDoubleClick={() => { if (!isRenaming) startRename(s.id); }}
                className={`group flex shrink-0 items-center gap-1 rounded-md border px-2.5 py-1 text-xs transition-colors ${
                  active
                    ? "border-orange-300 bg-orange-50 font-medium text-orange-700 dark:border-orange-700 dark:bg-orange-950 dark:text-orange-400"
                    : "border-neutral-200 text-neutral-600 hover:bg-neutral-100 dark:border-neutral-800 dark:text-neutral-400 dark:hover:bg-neutral-800"
                }`}
              >
                {isRenaming ? (
                  <input
                    autoFocus
                    value={renameVal}
                    onChange={(e) => setRenameVal(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") commitRename();
                      else if (e.key === "Escape") cancelRename();
                    }}
                    onBlur={commitRename}
                    className="w-[100px] rounded border border-orange-400 bg-white px-1 py-0.5 text-xs outline-none dark:bg-neutral-900"
                  />
                ) : (
                  <>
                    <button
                      className="max-w-[110px] truncate"
                      title={s.name}
                      onClick={() => { if (!active) onSelect(s.id); }}
                    >{s.name}</button>
                    {sessions.length > 1 && (
                      <button
                        title="删除该会话"
                        aria-label={`删除会话 ${s.name}`}
                        className={`-mr-0.5 rounded text-[13px] leading-none hover:text-red-500 ${
                          active ? "opacity-60 hover:opacity-100" : "opacity-0 group-hover:opacity-60"
                        }`}
                        onClick={(e) => { e.stopPropagation(); void onDelete(s.id); }}
                      >×</button>
                    )}
                  </>
                )}
              </div>
            );
          })}
        </div>
        <button
          title="新建会话"
          className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-neutral-200 text-sm leading-none text-neutral-500 hover:border-orange-300 hover:bg-orange-50 hover:text-orange-600 dark:border-neutral-800 dark:hover:border-orange-700 dark:hover:bg-orange-950"
          onClick={() => void onNew()}
        >+</button>
      </div>

      {menu && (
        <SessionContextMenu
          x={menu.x}
          y={menu.y}
          onRename={() => { startRename(menu.id); setMenu(null); }}
          onDelete={() => { const id = menu.id; setMenu(null); void onDelete(id); }}
        />
      )}
    </>
  );
}
