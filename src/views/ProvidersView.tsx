import { useEffect, useState } from "react";
import ProviderForm from "../components/ProviderForm";
import Select from "../components/Select";
import { api } from "../ipc";
import { errorMessage } from "../lib/error";
import { useModels } from "../stores/models";
import { useProviders } from "../stores/providers";
import { confirmDelete } from "../ui/dialogs";
import {
  btnCls,
  btnGhostCls,
  btnPrimaryCls,
  btnRowCls,
  btnRowDangerCls,
  inputCls,
  labelCls,
  textareaCls,
} from "../ui/controls";
import {
  PROTOCOL_LABELS,
  type ModelDraft,
  type ModelItem,
  type Protocol,
  type ProviderListItem,
} from "../types";

export default function ProvidersView() {
  const { providers, loading, error, load, remove } = useProviders();
  const { models, load: loadModels } = useModels();
  const [editing, setEditing] = useState<ProviderListItem | null | undefined>(undefined);
  const [notice, setNotice] = useState<string | null>(null);

  // 新增模型的草稿
  const [modelName, setModelName] = useState("");
  const [modelProtocol, setModelProtocol] = useState<Protocol>("openai_images");
  const [modelLabel, setModelLabel] = useState("");
  const [discoverProviderId, setDiscoverProviderId] = useState("");
  const [discovered, setDiscovered] = useState<string[]>([]);
  // 正在编辑能力覆写的模型
  const [capsEditingId, setCapsEditingId] = useState<string | null>(null);
  const [capsDraft, setCapsDraft] = useState("");

  useEffect(() => {
    void load();
    void loadModels();
  }, [load, loadModels]);

  const modelsFor = (p: ProviderListItem): ModelItem[] =>
    (p.model_ids ?? [])
      .map((id) => models.find((m) => m.id === id))
      .filter((m): m is ModelItem => !!m);

  const allModelsForProvider = (p: ProviderListItem): ModelItem[] => {
    const attached = new Set(p.model_ids ?? []);
    return models.filter((m) => !attached.has(m.id));
  };

  const addModel = async () => {
    const name = modelName.trim();
    if (!name) {
      setNotice("模型名不能为空");
      return;
    }
    try {
      await api.modelAdd({
        protocol: modelProtocol,
        model_name: name,
        label: modelLabel.trim(),
      } satisfies ModelDraft);
      setModelName("");
      setModelLabel("");
      await loadModels();
      setNotice(`已添加模型「${name}」`);
    } catch (e) {
      setNotice(errorMessage(e));
    }
  };

  const discover = async () => {
    const p = providers.find((x) => x.id === discoverProviderId);
    if (!p) {
      setNotice("先选一个供应商，再用它的配置去发现模型");
      return;
    }
    try {
      const res = await api.providerDiscover({
        id: p.id,
        base_url: p.base_url,
        protocol: p.protocol,
        model: p.model,
        api_key: "",
        custom_headers: p.custom_headers,
      });
      setDiscovered(res.models);
      setNotice(res.message || `发现 ${res.models.length} 个模型`);
    } catch (e) {
      setNotice(errorMessage(e));
    }
  };

  const batchAddDiscovered = async () => {
    const p = providers.find((x) => x.id === discoverProviderId);
    if (!p) return;
    try {
      for (const name of discovered) {
        const id = await api.modelAdd({
          protocol: p.protocol,
          model_name: name,
          label: "",
        });
        await api.providerModelAdd(p.id, id);
      }
      setDiscovered([]);
      await Promise.all([loadModels(), load()]);
      setNotice(`已批量添加并关联 ${discovered.length} 个模型`);
    } catch (e) {
      setNotice(errorMessage(e));
    }
  };

  const attach = async (providerId: string, modelId: string) => {
    try {
      await api.providerModelAdd(providerId, modelId);
      await Promise.all([loadModels(), load()]);
    } catch (e) {
      setNotice(errorMessage(e));
    }
  };

  const detach = async (providerId: string, modelId: string) => {
    try {
      await api.providerModelRemove(providerId, modelId);
      await Promise.all([loadModels(), load()]);
    } catch (e) {
      setNotice(errorMessage(e));
    }
  };

  const setDefault = async (providerId: string, modelId: string) => {
    try {
      await api.providerModelSetDefault(providerId, modelId);
      await load();
      setNotice("已设为默认模型");
    } catch (e) {
      setNotice(errorMessage(e));
    }
  };

  /** 保存模型级能力覆写：复用 model_add（同协议+同名即同一条模型），只改覆写与显示名。 */
  const saveCaps = async (m: ModelItem) => {
    const raw = capsDraft.trim();
    if (raw) {
      try {
        JSON.parse(raw);
      } catch {
        setNotice("能力覆写必须是合法 JSON");
        return;
      }
    }
    try {
      await api.modelAdd({
        protocol: m.protocol,
        model_name: m.model_name,
        label: m.label,
        capabilities_override: raw,
      } satisfies ModelDraft);
      setCapsEditingId(null);
      setCapsDraft("");
      await loadModels();
      setNotice(raw ? "已更新能力覆写" : "已清除能力覆写");
    } catch (e) {
      setNotice(errorMessage(e));
    }
  };

  const deleteModel = async (m: ModelItem) => {
    if (!(await confirmDelete(`删除模型「${m.label || m.model_name}」？会从所有供应商解绑。`))) return;
    try {
      await api.modelDelete(m.id);
      await Promise.all([loadModels(), load()]);
    } catch (e) {
      setNotice(errorMessage(e));
    }
  };

  return (
    <div className="mx-auto max-w-3xl space-y-8 p-8">
      {notice && (
        <div className="rounded-md bg-blue-50 px-3 py-2 text-sm text-blue-700 dark:bg-blue-950 dark:text-blue-400">
          {notice}
          <button className="ml-2 text-xs underline" onClick={() => setNotice(null)}>关闭</button>
        </div>
      )}

      {/* 模型库：全局模型，可被多家供应商复用 */}
      <section>
        <div className="mb-4">
          <h1 className="text-lg font-medium text-neutral-900 dark:text-neutral-100">模型库</h1>
          <p className="mt-1 text-sm text-neutral-500">
            模型可挂到多家供应商；协议归模型，同一家供应商也能挂不同协议的模型。
          </p>
        </div>

        <div className="mb-4 rounded-xl border border-neutral-200 p-4 dark:border-neutral-800">
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className={labelCls}>模型名（上游实际用的 ID）</label>
              <input
                className={inputCls}
                value={modelName}
                onChange={(e) => setModelName(e.target.value)}
                placeholder="gpt-image-1"
                list="discovered-model-names"
              />
            </div>
            <div>
              <label className={labelCls}>协议</label>
              <Select
                className={inputCls}
                value={modelProtocol}
                onChange={(v) => setModelProtocol(v as Protocol)}
                options={(Object.keys(PROTOCOL_LABELS) as Protocol[]).map((p) => ({
                  value: p,
                  label: PROTOCOL_LABELS[p],
                }))}
              />
            </div>
          </div>
          <div className="mt-3">
            <label className={labelCls}>显示名（可选）</label>
            <input className={inputCls} value={modelLabel} onChange={(e) => setModelLabel(e.target.value)} placeholder="如：GPT Image 1" />
          </div>
          <datalist id="discovered-model-names">
            {discovered.map((m) => <option key={m} value={m} />)}
          </datalist>
          <div className="mt-3 flex justify-end gap-2">
            <button
              className={btnCls}
              onClick={() => void discover()}
            >从供应商发现模型</button>
            <button
              className={btnPrimaryCls}
              onClick={() => void addModel()}
              disabled={!modelName.trim()}
            >添加模型</button>
          </div>

          {providers.length > 0 && (
            <div className="mt-3 border-t border-neutral-200 pt-3 dark:border-neutral-800">
              <label className={labelCls}>用哪家供应商的配置去发现模型</label>
              <Select
                className={inputCls}
                value={discoverProviderId}
                onChange={(v) => { setDiscoverProviderId(v); setDiscovered([]); }}
                options={[
                  { value: "", label: "（选择供应商）" },
                  ...providers.map((p) => ({ value: p.id, label: p.name })),
                ]}
              />
              {discovered.length > 0 && (
                <div className="mt-2 rounded-md bg-neutral-50 p-2 dark:bg-neutral-900">
                  <div className="mb-2 flex items-center justify-between text-xs text-neutral-500">
                    <span>发现 {discovered.length} 个模型</span>
                    <button className="text-orange-600 hover:underline" onClick={() => void batchAddDiscovered()}>
                      全部添加并挂到该供应商
                    </button>
                  </div>
                  <div className="flex flex-wrap gap-1">
                    {discovered.slice(0, 40).map((m) => (
                      <button
                        key={m}
                        onClick={() => setModelName(m)}
                        className="rounded-full bg-neutral-200 px-2 py-0.5 text-xs text-neutral-700 hover:bg-orange-100 dark:bg-neutral-800 dark:text-neutral-200"
                      >{m}</button>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>

        {models.length === 0 ? (
          <p className="text-sm text-neutral-400">模型库为空，添加一个模型或先从供应商发现。</p>
        ) : (
          <div className="space-y-2">
            {models.map((m) => (
              <div key={m.id} className="flex items-center justify-between rounded-lg border border-neutral-200 px-4 py-2 dark:border-neutral-800">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-neutral-900 dark:text-neutral-100">{m.label || m.model_name}</span>
                    <span className="rounded-full bg-neutral-100 px-2 py-0.5 text-xs text-neutral-500 dark:bg-neutral-800">
                      {PROTOCOL_LABELS[m.protocol]?.split("（")[0] ?? m.protocol}
                    </span>
                    {m.provider_count > 0 && (
                      <span className="text-xs text-neutral-400">已挂 {m.provider_count} 家</span>
                    )}
                  </div>
                  <p className="mt-0.5 truncate text-xs text-neutral-400">{m.model_name}</p>
                </div>
                <div className="flex shrink-0 gap-1">
                  <button
                    className={btnRowCls}
                    onClick={() => {
                      setCapsEditingId(capsEditingId === m.id ? null : m.id);
                      setCapsDraft(m.capabilities_override ?? "");
                    }}
                  >能力</button>
                  <button
                    className={btnRowDangerCls}
                    onClick={() => void deleteModel(m)}
                  >删除</button>
                </div>
              </div>
            ))}
          </div>
        )}

        {capsEditingId && (
          <div className="rounded-xl border border-orange-200 bg-orange-50/40 p-4 dark:border-orange-900 dark:bg-orange-950/20">
            <label className={labelCls}>
              能力覆写（JSON，可选）——中转站这类非标准模型用它改尺寸/质量取值
            </label>
            <textarea
              className={`${textareaCls} h-28 font-mono text-xs`}
              value={capsDraft}
              onChange={(e) => setCapsDraft(e.target.value)}
              placeholder={'{"sizes":["1024x1024","768x1344"],"seed":true}'}
            />
            <p className="mt-1 text-xs text-neutral-500">
              可用字段：sizes / quality / seed / refs / mask / endpoint / sizeLabel。留空=用内置推导值。
            </p>
            <div className="mt-2 flex justify-end gap-2">
              <button
                className={btnGhostCls}
                onClick={() => setCapsEditingId(null)}
              >取消</button>
              <button
                className={btnPrimaryCls}
                onClick={() => {
                  const m = models.find((x) => x.id === capsEditingId);
                  if (m) void saveCaps(m);
                }}
              >保存</button>
            </div>
          </div>
        )}
      </section>

      {/* 供应商：连接配置 + 每家挂哪些模型 */}
      <section>
        <div className="mb-4 flex items-center justify-between">
          <div>
            <h2 className="text-lg font-medium text-neutral-900 dark:text-neutral-100">供应商</h2>
            <p className="mt-1 text-sm text-neutral-500">接入官方或任意中转站，Key 存本地数据库，不上传。</p>
          </div>
          <button
            className={btnPrimaryCls}
            onClick={() => setEditing(null)}
          >+ 新增供应商</button>
        </div>

        {error && <div className="mb-3 rounded-md bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-950 dark:text-red-400">{error}</div>}
        {loading && <p className="text-sm text-neutral-400">加载中…</p>}
        {!loading && providers.length === 0 && (
          <div className="rounded-xl border border-dashed border-neutral-300 p-10 text-center dark:border-neutral-700">
            <p className="text-sm text-neutral-500">还没有供应商配置</p>
            <p className="mt-1 text-xs text-neutral-400">
              支持 {Object.keys(PROTOCOL_LABELS).length} 种协议（OpenAI Images / Chat 生图 / Gemini / MJ Proxy / 火山方舟 / 百炼 / Stability / Ideogram / FLUX / MiniMax），选模板即可一键配置
            </p>
          </div>
        )}

        <div className="space-y-3">
          {providers.map((p) => {
            const attached = modelsFor(p);
            const available = allModelsForProvider(p);
            const defaultId = attached[0]?.id;
            return (
              <div key={p.id} className="rounded-xl border border-neutral-200 p-4 dark:border-neutral-800">
                <div className="flex items-center justify-between">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-neutral-900 dark:text-neutral-100">{p.name}</span>
                      <span className={`text-xs ${p.has_key ? "text-green-600" : "text-amber-600"}`}>
                        {p.has_key ? "Key 已存" : "无 Key"}
                      </span>
                    </div>
                    <p className="mt-1 truncate text-xs text-neutral-400">{p.base_url}</p>
                  </div>
                  <div className="flex shrink-0 gap-1">
                    <button className={btnRowCls}
                      onClick={() => setEditing(p)}>编辑</button>
                    <button className={btnRowDangerCls}
                      onClick={async () => { if (await confirmDelete(`删除供应商「${p.name}」？`)) remove(p.id); }}>删除</button>
                  </div>
                </div>

                <div className="mt-3 border-t border-neutral-100 pt-3 dark:border-neutral-800">
                  <label className={labelCls}>挂在这个供应商下的模型</label>
                  {attached.length === 0 ? (
                    <p className="text-xs text-neutral-400">还没挂模型 —— 生成会失败，请从下方选一个挂上。</p>
                  ) : (
                    <div className="space-y-1.5">
                      {attached.map((m) => (
                        <div key={m.id} className="flex items-center justify-between rounded-md bg-neutral-50 px-3 py-1.5 dark:bg-neutral-900">
                          <div className="flex min-w-0 items-center gap-2">
                            <span className="truncate text-sm text-neutral-700 dark:text-neutral-200">{m.label || m.model_name}</span>
                            {m.id === defaultId && (
                              <span className="shrink-0 rounded-full bg-orange-100 px-2 py-0.5 text-[11px] text-orange-700 dark:bg-orange-950 dark:text-orange-400">默认</span>
                            )}
                          </div>
                          <div className="flex shrink-0 gap-2 text-xs">
                            {m.id !== defaultId && (
                              <button className="text-orange-600 hover:underline" onClick={() => void setDefault(p.id, m.id)}>设为默认</button>
                            )}
                            <button className="text-red-500 hover:underline" onClick={() => void detach(p.id, m.id)}>解绑</button>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                  {available.length > 0 && (
                    <div className="mt-2 flex items-center gap-2">
                      <Select
                        className={inputCls}
                        value=""
                        onChange={(v) => { if (v) void attach(p.id, v); }}
                        options={[
                          { value: "", label: "+ 挂一个模型…" },
                          ...available.map((m) => ({ value: m.id, label: m.label || m.model_name })),
                        ]}
                      />
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </section>

      {editing !== undefined && (
        <ProviderForm editing={editing} onClose={() => setEditing(undefined)} />
      )}
    </div>
  );
}
