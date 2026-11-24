import { useEffect, useState } from "react";
import { api } from "../ipc";
import { usePrefill } from "../stores/prefill";
import {
  btnGhostCls,
  btnPrimaryCls,
  btnRowCls,
  btnRowDangerCls,
  inputCls,
  textareaCls,
} from "../ui/controls";
import type { PromptItem } from "../types";

export default function PromptsView() {
  const [items, setItems] = useState<PromptItem[]>([]);
  const [q, setQ] = useState("");
  const [editing, setEditing] = useState<{ id?: string; title: string; content: string } | null>(null);
  const sendPrefill = usePrefill((s) => s.send);

  const reload = () => api.promptList().then(setItems);
  useEffect(() => {
    reload();
  }, []);

  const filtered = items.filter(
    (p) => !q || p.title.includes(q) || p.content.includes(q) || p.tags.some((t) => t.includes(q)),
  );

  const save = async () => {
    if (!editing || !editing.title.trim() || !editing.content.trim()) return;
    await api.promptSave({ id: editing.id, title: editing.title.trim(), content: editing.content.trim() });
    setEditing(null);
    reload();
  };

  return (
    <div className="mx-auto max-w-3xl p-6">
      <div className="mb-5 flex items-center gap-3">
        <h1 className="shrink-0 text-lg font-medium whitespace-nowrap">提示词库</h1>
        {/* w-56! 必须带 important：inputCls 里的 w-full 在 Tailwind 产物中排在 w-56 之后，
            不加 important 搜索框会撑满整行，把标题和按钮压成最小宽度，中文逐字断行。 */}
        <input className={`${inputCls} w-56!`} placeholder="搜索…" value={q} onChange={(e) => setQ(e.target.value)} />
        <button className={`${btnPrimaryCls} ml-auto shrink-0 whitespace-nowrap`}
          onClick={() => setEditing({ title: "", content: "" })}>
          + 新增
        </button>
      </div>

      <div className="space-y-2">
        {filtered.map((p) => (
          <div key={p.id} className="rounded-lg border border-neutral-200 p-3 dark:border-neutral-800">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium">{p.title}</span>
              <div className="ml-auto flex gap-1">
                <button className={btnRowCls}
                  onClick={() => sendPrefill({ prompt: p.content })}>应用到工作台</button>
                <button className={btnRowCls}
                  onClick={() => navigator.clipboard.writeText(p.content)}>复制</button>
                <button className={btnRowCls}
                  onClick={() => setEditing({ id: p.id, title: p.title, content: p.content })}>编辑</button>
                <button className={btnRowDangerCls}
                  onClick={async () => { if (confirm("删除该提示词？")) { await api.promptDelete(p.id); reload(); } }}>删除</button>
              </div>
            </div>
            <p className="mt-1 line-clamp-2 text-xs text-neutral-500">{p.content}</p>
          </div>
        ))}
        {filtered.length === 0 && <p className="py-16 text-center text-sm text-neutral-400">还没有提示词，点右上角新增</p>}
      </div>

      {editing && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6">
          <div className="w-[520px] rounded-xl bg-white p-6 dark:bg-neutral-900">
            <h2 className="mb-4 text-base font-medium">{editing.id ? "编辑提示词" : "新增提示词"}</h2>
            <input className={`${inputCls} mb-3`} placeholder="标题" value={editing.title}
              onChange={(e) => setEditing({ ...editing, title: e.target.value })} />
            <textarea className={`${textareaCls} min-h-[160px] resize-y`} placeholder="提示词内容…"
              value={editing.content} onChange={(e) => setEditing({ ...editing, content: e.target.value })} />
            <div className="mt-4 flex justify-end gap-2">
              <button className={btnGhostCls}
                onClick={() => setEditing(null)}>取消</button>
              <button className={btnPrimaryCls} onClick={save}>保存</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
