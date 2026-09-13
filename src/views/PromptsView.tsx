import { useEffect, useState } from "react";
import { api } from "../ipc";
import { usePrefill } from "../stores/prefill";
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

  const inputCls =
    "w-full rounded-md border border-neutral-300 bg-white px-3 py-2 text-sm focus:border-orange-500 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100";

  return (
    <div className="mx-auto max-w-3xl p-6">
      <div className="mb-5 flex items-center gap-3">
        <h1 className="text-lg font-medium">提示词库</h1>
        <input className={`${inputCls} w-56`} placeholder="搜索…" value={q} onChange={(e) => setQ(e.target.value)} />
        <button className="ml-auto rounded-md bg-orange-600 px-4 py-2 text-sm text-white hover:bg-orange-700"
          onClick={() => setEditing({ title: "", content: "" })}>
          + 新增
        </button>
      </div>

      <div className="space-y-2">
        {filtered.map((p) => (
          <div key={p.id} className="rounded-lg border border-neutral-200 p-3 dark:border-neutral-800">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium">{p.title}</span>
              <div className="ml-auto flex gap-1 text-xs">
                <button className="rounded px-2 py-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                  onClick={() => sendPrefill({ prompt: p.content })}>应用到工作台</button>
                <button className="rounded px-2 py-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                  onClick={() => navigator.clipboard.writeText(p.content)}>复制</button>
                <button className="rounded px-2 py-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                  onClick={() => setEditing({ id: p.id, title: p.title, content: p.content })}>编辑</button>
                <button className="rounded px-2 py-1 text-red-500 hover:bg-red-50 dark:hover:bg-red-950"
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
            <textarea className={`${inputCls} min-h-[160px] resize-y`} placeholder="提示词内容…"
              value={editing.content} onChange={(e) => setEditing({ ...editing, content: e.target.value })} />
            <div className="mt-4 flex justify-end gap-2">
              <button className="rounded-md px-4 py-2 text-sm text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                onClick={() => setEditing(null)}>取消</button>
              <button className="rounded-md bg-orange-600 px-4 py-2 text-sm text-white hover:bg-orange-700" onClick={save}>保存</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
