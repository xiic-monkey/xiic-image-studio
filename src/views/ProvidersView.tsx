import { useEffect, useState } from "react";
import ProviderForm from "../components/ProviderForm";
import { useProviders } from "../stores/providers";
import { PROTOCOL_LABELS, type Protocol, type ProviderListItem } from "../types";

export default function ProvidersView() {
  const { providers, loading, error, load, remove } = useProviders();
  const [editing, setEditing] = useState<ProviderListItem | null | undefined>(undefined);

  useEffect(() => { load(); }, [load]);

  return (
    <div className="mx-auto max-w-3xl p-8">
      <div className="mb-6 flex items-center justify-between">
        <div>
          <h1 className="text-lg font-medium text-neutral-900 dark:text-neutral-100">供应商</h1>
          <p className="mt-1 text-sm text-neutral-500">接入官方或任意中转站，Key 存本地数据库，不上传。</p>
        </div>
        <button
          className="rounded-md bg-orange-600 px-4 py-2 text-sm text-white hover:bg-orange-700"
          onClick={() => setEditing(null)}
        >+ 新增供应商</button>
      </div>

      {error && <div className="rounded-md bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-950 dark:text-red-400">{error}</div>}

      {loading && <p className="text-sm text-neutral-400">加载中…</p>}

      {!loading && providers.length === 0 && (
        <div className="rounded-xl border border-dashed border-neutral-300 p-10 text-center dark:border-neutral-700">
          <p className="text-sm text-neutral-500">还没有供应商配置</p>
          <p className="mt-1 text-xs text-neutral-400">支持 OpenAI Images / Chat 生图 / Gemini 原生 / MJ Proxy 四种协议</p>
        </div>
      )}

      <div className="space-y-3">
        {providers.map((p) => (
          <div key={p.id} className="flex items-center justify-between rounded-xl border border-neutral-200 p-4 dark:border-neutral-800">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <span className="font-medium text-neutral-900 dark:text-neutral-100">{p.name}</span>
                <span className="rounded-full bg-neutral-100 px-2 py-0.5 text-xs text-neutral-500 dark:bg-neutral-800">
                  {PROTOCOL_LABELS[p.protocol as Protocol]?.split("（")[0] ?? p.protocol}
                </span>
                <span className={`text-xs ${p.has_key ? "text-green-600" : "text-amber-600"}`}>
                  {p.has_key ? "Key 已存" : "无 Key"}
                </span>
              </div>
              <p className="mt-1 truncate text-xs text-neutral-400">{p.base_url} · {p.model || "未设模型"}</p>
            </div>
            <div className="flex shrink-0 gap-1">
              <button className="rounded-md px-3 py-1.5 text-sm text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"
                onClick={() => setEditing(p)}>编辑</button>
              <button className="rounded-md px-3 py-1.5 text-sm text-red-500 hover:bg-red-50 dark:hover:bg-red-950"
                onClick={() => { if (confirm(`删除供应商「${p.name}」？`)) remove(p.id); }}>删除</button>
            </div>
          </div>
        ))}
      </div>

      {editing !== undefined && (
        <ProviderForm editing={editing} onClose={() => setEditing(undefined)} />
      )}
    </div>
  );
}
