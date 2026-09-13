import { useState } from "react";
import Select from "../components/Select";
import { api } from "../ipc";
import { useProviders } from "../stores/providers";
import {
  PROTOCOL_LABELS,
  PROTOCOL_PRESET_MODELS,
  PROTOCOL_PRESET_URL,
  type Protocol,
  type ProviderListItem,
  type TestResult,
} from "../types";

interface Props {
  editing: ProviderListItem | null;
  onClose: () => void;
}

interface HeaderRow {
  k: string;
  v: string;
}

export default function ProviderForm({ editing, onClose }: Props) {
  const reload = useProviders((s) => s.load);
  const [name, setName] = useState(editing?.name ?? "");
  const [protocol, setProtocol] = useState<Protocol>(editing?.protocol ?? "openai_images");
  const [baseUrl, setBaseUrl] = useState(editing?.base_url ?? "");
  const [model, setModel] = useState(editing?.model ?? "");
  const [apiKey, setApiKey] = useState("");
  const [headers, setHeaders] = useState<HeaderRow[]>(
    Object.entries(editing?.custom_headers ?? {}).map(([k, v]) => ({ k, v })),
  );
  const [models, setModels] = useState<string[]>([]);
  const [testing, setTesting] = useState(false);
  const [discovering, setDiscovering] = useState(false);
  const [testResult, setTestResult] = useState<TestResult | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const draft = () => ({
    id: editing?.id ?? null,
    base_url: baseUrl.trim(),
    protocol,
    model,
    api_key: apiKey,
    custom_headers: Object.fromEntries(
      headers.filter((h) => h.k.trim() && h.v.trim()).map((h) => [h.k.trim(), h.v.trim()]),
    ),
  });

  const switchProtocol = (p: Protocol) => {
    setProtocol(p);
    if (!editing) {
      setBaseUrl(PROTOCOL_PRESET_URL[p]);
      setModel(PROTOCOL_PRESET_MODELS[p][0] ?? "");
    }
  };

  const runTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      setTestResult(await api.providerTest(draft()));
    } catch (e) {
      setTestResult({ ok: false, status: null, message: String(e), latency_ms: null });
    } finally {
      setTesting(false);
    }
  };

  const runDiscover = async () => {
    setDiscovering(true);
    setNotice(null);
    try {
      const res = await api.providerDiscover(draft());
      setModels(res.models);
      setNotice(res.message);
    } catch (e) {
      setNotice(String(e));
    } finally {
      setDiscovering(false);
    }
  };

  const save = async () => {
    if (!name.trim() || !baseUrl.trim()) {
      setNotice("名称和 baseUrl 必填");
      return;
    }
    setSaving(true);
    try {
      await api.providerSave({
        id: editing?.id ?? crypto.randomUUID(),
        name: name.trim(),
        base_url: baseUrl.trim(),
        protocol,
        model: model.trim(),
        custom_headers: Object.fromEntries(
          headers.filter((h) => h.k.trim() && h.v.trim()).map((h) => [h.k.trim(), h.v.trim()]),
        ),
        api_key: apiKey.trim() ? apiKey.trim() : null,
      });
      await reload();
      onClose();
    } catch (e) {
      setNotice(String(e));
    } finally {
      setSaving(false);
    }
  };

  const inputCls =
    "w-full rounded-md border border-neutral-300 bg-white px-3 py-2 text-sm focus:border-orange-500 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100";
  const labelCls = "mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400";

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6">
      <div className="max-h-full w-[560px] overflow-y-auto rounded-xl bg-white p-6 shadow-xl dark:bg-neutral-900">
        <h2 className="mb-4 text-base font-medium text-neutral-900 dark:text-neutral-100">
          {editing ? "编辑供应商" : "新增供应商"}
        </h2>

        <div className="space-y-4">
          <div>
            <label className={labelCls}>名称</label>
            <input className={inputCls} value={name} onChange={(e) => setName(e.target.value)} placeholder="如：某中转站-生图" />
          </div>

          <div>
            <label className={labelCls}>协议</label>
            <Select
              className={inputCls}
              value={protocol}
              onChange={(v) => switchProtocol(v as Protocol)}
              options={(Object.keys(PROTOCOL_LABELS) as Protocol[]).map((p) => ({
                value: p,
                label: PROTOCOL_LABELS[p],
              }))}
            />
          </div>

          <div>
            <label className={labelCls}>Base URL（支持官方或任意中转）</label>
            <div className="flex gap-2">
              <input className={inputCls} value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="https://..." />
              <button
                className="shrink-0 rounded-md border border-neutral-300 px-3 text-xs text-neutral-600 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-300 dark:hover:bg-neutral-800"
                onClick={() => setBaseUrl(PROTOCOL_PRESET_URL[protocol])}
              >预设</button>
            </div>
          </div>

          <div>
            <label className={labelCls}>API Key{editing?.has_key ? "（已保存，留空则不修改）" : ""}</label>
            <input className={inputCls} type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-..." />
          </div>

          <div>
            <label className={labelCls}>模型</label>
            <div className="flex gap-2">
              <input className={inputCls} value={model} onChange={(e) => setModel(e.target.value)} placeholder="gpt-image-2.5" list="discovered-models" />
              <button
                className="shrink-0 rounded-md border border-neutral-300 px-3 text-xs text-neutral-600 hover:bg-neutral-100 disabled:opacity-50 dark:border-neutral-700 dark:text-neutral-300"
                onClick={runDiscover}
                disabled={discovering}
              >{discovering ? "发现中…" : "发现模型"}</button>
            </div>
            <datalist id="discovered-models">
              {models.map((m) => <option key={m} value={m} />)}
            </datalist>
            {models.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-1">
                {models.slice(0, 20).map((m) => (
                  <button key={m} onClick={() => setModel(m)}
                    className="rounded-full bg-neutral-100 px-2 py-0.5 text-xs text-neutral-600 hover:bg-orange-100 hover:text-orange-700 dark:bg-neutral-800 dark:text-neutral-300">
                    {m}
                  </button>
                ))}
              </div>
            )}
          </div>

          <div>
            <div className="flex items-center justify-between">
              <label className={labelCls}>自定义 Header（中转站特殊鉴权用）</label>
              <button className="text-xs text-orange-600 hover:underline" onClick={() => setHeaders([...headers, { k: "", v: "" }])}>
                + 添加
              </button>
            </div>
            {headers.map((h, i) => (
              <div key={i} className="mb-1.5 flex gap-1.5">
                <input className={inputCls} value={h.k} placeholder="Header 名"
                  onChange={(e) => setHeaders(headers.map((x, j) => (j === i ? { ...x, k: e.target.value } : x)))} />
                <input className={inputCls} value={h.v} placeholder="值"
                  onChange={(e) => setHeaders(headers.map((x, j) => (j === i ? { ...x, v: e.target.value } : x)))} />
                <button className="shrink-0 px-2 text-neutral-400 hover:text-red-500"
                  onClick={() => setHeaders(headers.filter((_, j) => j !== i))}>×</button>
              </div>
            ))}
          </div>

          {testResult && (
            <div className={`rounded-md px-3 py-2 text-xs ${testResult.ok ? "bg-green-50 text-green-700 dark:bg-green-950 dark:text-green-400" : "bg-red-50 text-red-700 dark:bg-red-950 dark:text-red-400"}`}>
              {testResult.ok ? "✓ " : "✗ "}{testResult.message}
              {testResult.latency_ms != null && `（${testResult.latency_ms}ms）`}
            </div>
          )}
          {notice && <div className="rounded-md bg-blue-50 px-3 py-2 text-xs text-blue-700 dark:bg-blue-950 dark:text-blue-400">{notice}</div>}

          <div className="flex justify-between pt-2">
            <button
              className="rounded-md border border-neutral-300 px-4 py-2 text-sm text-neutral-700 hover:bg-neutral-100 disabled:opacity-50 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800"
              onClick={runTest} disabled={testing}>
              {testing ? "测试中…" : "连通测试"}
            </button>
            <div className="flex gap-2">
              <button className="rounded-md px-4 py-2 text-sm text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800" onClick={onClose}>取消</button>
              <button className="rounded-md bg-orange-600 px-4 py-2 text-sm text-white hover:bg-orange-700 disabled:opacity-50"
                onClick={save} disabled={saving}>
                {saving ? "保存中…" : "保存"}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
