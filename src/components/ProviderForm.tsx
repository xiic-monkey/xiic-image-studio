import { useState } from "react";
import Select from "../components/Select";
import { api } from "../ipc";
import { errorMessage } from "../lib/error";
import { useProviders } from "../stores/providers";
import {
  btnCls,
  btnCompactCls,
  btnGhostCls,
  btnPrimaryCls,
  inputCls,
  labelCls,
} from "../ui/controls";
import {
  PROTOCOL_LABELS,
  PROTOCOL_PRESET_MODELS,
  PROTOCOL_PRESET_URL,
  PROVIDER_TEMPLATES,
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
  // 模板：选一个就把名称/baseUrl/协议/常用模型填好，Key 仍需自己填
  const [templateId, setTemplateId] = useState("");
  const [templateModels, setTemplateModels] = useState<string[]>([]);

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

  /** 套用模板：填好名称/baseUrl/协议，并列出该模板的常用模型供勾选。 */
  const applyTemplate = (id: string) => {
    setTemplateId(id);
    const t = PROVIDER_TEMPLATES.find((x) => x.id === id);
    if (!t) return;
    setName(t.name);
    setBaseUrl(t.base_url);
    setProtocol(t.protocol);
    setTemplateModels(t.models);
    setModel(t.models[0] ?? "");
    setNotice(t.note ? `已套用模板：${t.note}` : "已套用模板");
  };

  const runTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      setTestResult(await api.providerTest(draft()));
    } catch (e) {
      setTestResult({ ok: false, status: null, message: errorMessage(e), latency_ms: null });
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
      setNotice(errorMessage(e));
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
      const id = editing?.id ?? crypto.randomUUID();
      await api.providerSave({
        id,
        name: name.trim(),
        base_url: baseUrl.trim(),
        protocol,
        model: model.trim(),
        custom_headers: Object.fromEntries(
          headers.filter((h) => h.k.trim() && h.v.trim()).map((h) => [h.k.trim(), h.v.trim()]),
        ),
        api_key: apiKey.trim() ? apiKey.trim() : null,
      });
      // 模板勾选的模型：建好模型实体并挂到这家供应商下（模型是全局实体，多家可共用）
      const wanted = templateId ? templateModels : [];
      for (const modelName of wanted) {
        const mid = await api.modelAdd({ protocol, model_name: modelName, label: "" });
        await api.providerModelAdd(id, mid);
      }
      await reload();
      onClose();
    } catch (e) {
      setNotice(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6">
      <div className="max-h-full w-[560px] overflow-y-auto rounded-xl bg-white p-6 shadow-xl dark:bg-neutral-900">
        <h2 className="mb-4 text-base font-medium text-neutral-900 dark:text-neutral-100">
          {editing ? "编辑供应商" : "新增供应商"}
        </h2>

        <div className="space-y-4">
          {!editing && (
            <div className="rounded-lg border border-neutral-200 p-3 dark:border-neutral-800">
              <label className={labelCls}>从模板快速开始</label>
              <Select
                className={inputCls}
                value={templateId}
                onChange={(v) => applyTemplate(v)}
                options={[
                  { value: "", label: "（不用模板，手动填）" },
                  ...PROVIDER_TEMPLATES.map((t) => ({ value: t.id, label: t.name })),
                ]}
              />
              {templateId && templateModels.length > 0 && (
                <div className="mt-2 space-y-1">
                  <p className="text-xs text-neutral-500">
                    保存时一并建好模型并挂到这家供应商下（模型是全局实体，别家也能挂）：
                  </p>
                  {templateModels.map((m) => (
                    <label
                      key={m}
                      className="flex items-center gap-2 text-xs text-neutral-600 dark:text-neutral-300"
                    >
                      <input
                        type="checkbox"
                        checked={templateModels.includes(m)}
                        onChange={(e) =>
                          setTemplateModels(
                            e.target.checked
                              ? [...templateModels, m]
                              : templateModels.filter((x) => x !== m),
                          )
                        }
                      />
                      {m}
                    </label>
                  ))}
                </div>
              )}
            </div>
          )}

          <div>
            <label className={labelCls}>名称</label>
            <input className={inputCls} value={name} onChange={(e) => setName(e.target.value)} placeholder="如：OpenAI 官方" />
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
            <label className={labelCls}>Base URL（官方地址或自建代理均可）</label>
            <div className="flex gap-2">
              <input className={inputCls} value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="https://..." />
              <button
                className={`${btnCompactCls} shrink-0`}
                onClick={() => setBaseUrl(PROTOCOL_PRESET_URL[protocol])}
              >预设</button>
            </div>
          </div>

          <div>
            <label className={labelCls}>API Key{editing?.has_key ? "（已保存，留空则不修改）" : ""}</label>
            <input className={inputCls} type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-..." />
          </div>

          <div>
            <label className={labelCls}>
              模型（可留空；填了会顺带挂到这个供应商下，完整管理在「模型库」）
            </label>
            <div className="flex gap-2">
              <input className={inputCls} value={model} onChange={(e) => setModel(e.target.value)} placeholder="gpt-image-2.5" list="discovered-models" />
              <button
                className={`${btnCompactCls} shrink-0`}
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
              <label className={labelCls}>自定义 Header（供应商要求额外鉴权头时用）</label>
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
              className={btnCls}
              onClick={runTest} disabled={testing}>
              {testing ? "测试中…" : "连通测试"}
            </button>
            <div className="flex gap-2">
              <button className={btnGhostCls} onClick={onClose}>取消</button>
              <button className={btnPrimaryCls}
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
