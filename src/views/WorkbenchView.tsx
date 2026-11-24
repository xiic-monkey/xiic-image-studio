import { useCallback, useEffect, useState } from "react";
import Select from "../components/Select";
import TaskPanel from "../components/TaskPanel";
import { useAddAsRef } from "../hooks/useAddAsRef";
import { useCopyImage } from "../hooks/useCopyImage";
import { useDraft } from "../hooks/useDraft";
import { useSubmissions } from "../hooks/useSubmissions";
import { useTaskFeed } from "../hooks/useTaskFeed";
import { api } from "../ipc";
import { normalizeQuality, normalizeSize } from "../lib/capabilities";
import { errorMessage } from "../lib/error";
import { filesToRefs } from "../lib/ref-image";
import { pickPrefillPrompt } from "../lib/submission";
import { pickSizeForRatio } from "../lib/template";
import { useAssets } from "../stores/assets";
import { useModels } from "../stores/models";
import { usePrefill } from "../stores/prefill";
import { useProviders } from "../stores/providers";
import { useToast } from "../stores/toast";
import { PROTOCOL_LABELS, type ImageMeta, type PresetItem, type RefImage, type TaskView } from "../types";
import MaskEditorLayer from "./workbench/MaskEditorLayer";
import ParamRow from "./workbench/ParamRow";
import PresetPanel from "./workbench/PresetPanel";
import RefImages from "./workbench/RefImages";
import { labelCls, selCls, textareaCls } from "./workbench/styles";
import SubmitBar from "./workbench/SubmitBar";
import TemplatePanel, { type TemplateApplyPayload } from "./workbench/TemplatePanel";

/** 组合层：状态与副作用在 hooks，展示在各 workbench 子组件，这里只负责接线。 */
export default function WorkbenchView() {
  const showToast = useToast((s) => s.show);
  // useSubmissions 里的 setNotice(null) 是清旧提示；toast 自动消失，空值直接忽略
  const setNotice = (msg: string | null) => { if (msg) showToast(msg); };
  const { providers, load } = useProviders();
  const { models, load: loadModels } = useModels();
  const { tasks, imgUrls, upsert, cancel, remove } = useTaskFeed();
  const { form, setForm, patch, sessionId, ready } = useDraft();

  const { refs, addRefs, removeRef: removeAssetRef, clearRefs: clearAssetRefs } = useAssets();
  const [mask, setMask] = useState<RefImage | null>(null);
  const [maskEditing, setMaskEditing] = useState(false);
  const [tplOpen, setTplOpen] = useState(false);
  const [presetOpen, setPresetOpen] = useState(false);
  const [dragOver, setDragOver] = useState(false);

  const provider = providers.find((p) => p.id === form.providerId);
  // 当前供应商挂的模型（按关联），模型下拉只列这些；能力表取自选中模型而非 provider 旧列。
  // modelList 返回全局模型（linked=false），归属只看 provider.model_ids，不要再用 linked 过滤。
  const providerModels = models.filter((m) => provider?.model_ids?.includes(m.id));
  const selectedModel = providerModels.find((m) => m.id === form.modelId) ?? providerModels[0];
  const caps = selectedModel?.capabilities ?? provider?.capabilities;

  const { submit, importBatch, retry, submitting } = useSubmissions({
    providers,
    providerId: form.providerId,
    caps,
    form,
    refs,
    mask,
    sessionId,
    ready,
    upsert,
    patch,
    clearRefs: () => { clearAssetRefs(); setMask(null); },
    setNotice,
  });
  const prefill = usePrefill((s) => s.prefill);
  const sendPrefill = usePrefill((s) => s.send);
  const clearPrefill = usePrefill((s) => s.clear);

  const addRefsForTask = useAddAsRef(setNotice);
  const addTaskImagesAsRef = (imgs: ImageMeta[]) => {
    void addRefsForTask(imgs.map((i) => ({ path: i.path, id: i.id })));
  };
  const copyImage = useCopyImage(setNotice);
  const revealTaskImage = (img: ImageMeta) => {
    void api.imageReveal(img.id).catch((e) => setNotice(errorMessage(e)));
  };
  // 与画廊一致：先加参考图，满 4 张时不动提示词；没图片的任务仍可只回填参数继续生成
  const continueTask = async (t: TaskView) => {
    if (t.images.length > 0) {
      const { added, duplicates } = await addRefsForTask(t.images.map((i) => ({ path: i.path, id: i.id })));
      // 图已在参考里（重复）也要回填参数；只有真满额才不动用户的提示词
      if (added === 0 && duplicates === 0) return;
    }
    sendPrefill({ prompt: t.prompt, size: t.size ?? undefined, quality: t.quality ?? undefined });
  };

  useEffect(() => {
    void load();
    void loadModels();
  }, [load, loadModels]);

  // 用函数式更新读 prev：会话切换那一轮的 form 还是上个会话的，
  // 拿闭包里的旧值覆盖会把新会话草稿冲掉。
  useEffect(() => {
    if (providers.length === 0) return;
    setForm((prev) => (prev.providerId ? prev : { ...prev, providerId: providers[0].id }));
  }, [providers, form.providerId, setForm]);

  // 切换供应商后，若当前 modelId 不属于新供应商则清空，让 selectedModel 回落到其默认模型。
  useEffect(() => {
    if (!form.providerId || providerModels.length === 0) return;
    if (!providerModels.some((m) => m.id === form.modelId)) {
      patch({ modelId: providerModels[0].id });
    }
  }, [form.providerId, form.modelId, providerModels, patch]);

  // 切换供应商/模型后旧的尺寸/质量可能不在新能力表内。只在展示层回落，
  // 不改草稿：写回表单会在切会话那一轮把新会话的草稿当成旧会话的来覆盖；
  // 提交时 buildGenerateRequest 仍会按能力表归一化，上游拿到的值始终是合法的。
  const displaySize = caps ? normalizeSize(caps, form.size) ?? form.size : form.size;
  const displayQuality = caps ? normalizeQuality(caps, form.quality) ?? form.quality : form.quality;

  // ---- 画廊/提示词回填 ----
  useEffect(() => {
    // 等草稿加载完再回填：画廊「继续生成」切回工作台时，异步灌草稿会覆盖刚回填的 prompt
    if (!prefill || !ready) return;
    setForm((prev) => ({ ...prev, prompt: pickPrefillPrompt(prev.prompt, prefill.prompt) }));
    if (prefill.size) patch({ size: prefill.size });
    if (prefill.quality) patch({ quality: prefill.quality });
    if (prefill.count) patch({ count: prefill.count });
    if (prefill.seed) patch({ seed: prefill.seed });
    // 用完即弃：工作台随 tab 卸载，不清掉的话下次挂载会再把旧回填灌一遍，
    // 把用户在此期间改过的提示词覆盖回去。
    clearPrefill();
  }, [prefill, ready, setForm, patch, clearPrefill]);

  const addFiles = useCallback(async (files: FileList | File[]) => {
    addRefs(await filesToRefs(files));
  }, [addRefs]);

  // 粘贴图片
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const files = Array.from(e.clipboardData?.files ?? []);
      if (files.length > 0) void addFiles(files);
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, [addFiles]);

  const removeRef = (i: number) => {
    removeAssetRef(i);
    if (i === 0) setMask(null);
  };

  // 套用模板：面板已经把字段拼成提示词，这里只负责落到提示词框与参数上。
  // 尺寸不直接照抄模板——按当前模型的能力表挑最接近的比例对应值，
  // 换供应商/模型也不会发出上游不认的尺寸。
  const applyTemplate = (payload: TemplateApplyPayload) => {
    let prompt = payload.prompt;
    // MJ 的比例只能走提示词参数（协议不收 size 字段）。
    if (payload.ratio && selectedModel?.protocol === "midjourney_proxy") {
      prompt = `${prompt} --ar ${payload.ratio}`;
    }
    setForm((prev) => ({
      ...prev,
      prompt: payload.replace ? prompt : prev.prompt ? `${prev.prompt}\n${prompt}` : prompt,
    }));
    const size = payload.ratio ? pickSizeForRatio(caps?.sizes, payload.ratio) : undefined;
    if (size) patch({ size });
    setTplOpen(false);
    setNotice(
      `已插入模板「${payload.title}」${size ? `，尺寸 ${size}` : ""}`,
    );
  };

  // 套用配方：提示词整条替换，参数一并带上。
  // 配方的尺寸/质量可能不在当前供应商的能力表内——提交时 buildGenerateRequest
  // 会按能力表归一化，这里只管把值填回去，不必提前拦。
  const applyPreset = (p: PresetItem) => {
    setForm((prev) => ({
      ...prev,
      prompt: p.prompt,
      size: p.size ?? prev.size,
      quality: p.quality ?? prev.quality,
      seed: p.seed != null ? String(p.seed) : prev.seed,
      providerId: p.provider_id ?? prev.providerId,
      modelId: p.model_id ?? "",
    }));
    setPresetOpen(false);
    setNotice(`已套用配方「${p.name}」`);
  };

  return (
    <>
      <div className="flex h-full">
        {/* 左：参数区 */}
      <div
        className="w-[380px] shrink-0 space-y-4 overflow-y-auto border-r border-neutral-200 px-5 pb-5 pt-3.5 dark:border-neutral-800"
        onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(e) => { e.preventDefault(); setDragOver(false); void addFiles(e.dataTransfer.files); }}
      >
        <h1 className="text-base font-medium leading-6">工作台</h1>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className={labelCls}>供应商</label>
            <Select
              className={selCls}
              value={form.providerId}
              onChange={(v) => patch({ providerId: v, modelId: "" })}
              options={
                providers.length === 0
                  ? [{ value: "", label: "（先去供应商页添加）" }]
                  : providers.map((p) => ({ value: p.id, label: p.name }))
              }
            />
          </div>
          <div>
            <label className={labelCls}>模型</label>
            <Select
              className={selCls}
              value={selectedModel?.id ?? ""}
              onChange={(v) => patch({ modelId: v })}
              options={
                providerModels.length === 0
                  ? [{ value: "", label: "（先在供应商页挂模型）" }]
                  : providerModels.map((m) => ({
                      value: m.id,
                      label: m.label || m.model_name || PROTOCOL_LABELS[m.protocol],
                    }))
              }
            />
          </div>
        </div>

        <div>
          <div className="flex items-center justify-between">
            <label className={labelCls}>提示词</label>
            <button
              type="button"
              role="switch"
              aria-checked={form.batchMode}
              disabled={form.scanMode}
              title={form.scanMode ? "扫描模式已占用：一次提示词按种子拆任务" : undefined}
              onClick={() => setForm((prev) => ({ ...prev, batchMode: !prev.batchMode }))}
              className={`rounded-md px-2 py-0.5 text-[11px] transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
                form.batchMode
                  ? "bg-orange-100 font-medium text-orange-700 dark:bg-orange-950 dark:text-orange-400"
                  : "text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
              }`}
            >{`按行批量 ${form.batchMode ? "开" : "关"}`}</button>
          </div>
          <textarea
            className={`${textareaCls} min-h-[120px] resize-y`}
            value={form.prompt}
            onChange={(e) => patch({ prompt: e.target.value })}
            placeholder={form.batchMode ? "一行一个提示词，每行提交一个任务" : "描述你想要的画面…（换行不会拆成多个任务）"}
          />
          {form.batchMode && (
            <p className="mt-1 text-[11px] text-neutral-500">批量模式：每行一个任务、每行 1 张，「数量」不生效。</p>
          )}
        </div>

        <RefImages
          refs={refs}
          mask={mask}
          maskEnabled={!!caps?.mask}
          dragOver={dragOver}
          onAddFiles={addFiles}
          onRemove={removeRef}
          onClearMask={() => setMask(null)}
          onEditMask={() => setMaskEditing(true)}
        />

        <ParamRow
          caps={caps}
          size={displaySize}
          quality={displayQuality}
          seed={form.seed}
          count={form.count}
          scanMode={form.scanMode}
          scanCount={form.scanCount}
          onSize={(v) => patch({ size: v })}
          onQuality={(v) => patch({ quality: v })}
          onSeed={(v) => patch({ seed: v })}
          onCount={(v) => patch({ count: v })}
          onScanMode={(v) => setForm((prev) => ({ ...prev, scanMode: v, batchMode: v ? false : prev.batchMode }))}
          onScanCount={(v) => patch({ scanCount: v })}
        />

        <SubmitBar
          submitting={submitting}
          tplOpen={tplOpen}
          presetOpen={presetOpen}
          onToggleTemplates={() => setTplOpen((o) => !o)}
          onTogglePresets={() => setPresetOpen((o) => !o)}
          onImportBatch={() => void importBatch()}
          onSubmit={() => void submit()}
        />

        {presetOpen && (
          <PresetPanel
            onApply={applyPreset}
            current={{
              prompt: form.prompt,
              size: form.size,
              quality: form.quality,
              seed: form.seed,
              providerId: form.providerId,
              modelId: form.modelId,
            }}
            onNotice={setNotice}
          />
        )}
      </div>

      {/* 右：任务面板 */}
      <div className="min-w-0 flex-1 overflow-y-auto">
        <div className="sticky top-0 z-10 border-b border-neutral-200 bg-white/90 px-5 py-3 text-sm font-medium backdrop-blur dark:border-neutral-800 dark:bg-neutral-950/90">
          任务队列
        </div>
        <TaskPanel
          tasks={tasks}
          imgUrls={imgUrls}
          onCancel={cancel}
          onDelete={remove}
          onRetry={retry}
          onAddRefs={addTaskImagesAsRef}
          onContinue={(t) => void continueTask(t)}
          onCopy={(img) => void copyImage(img.path)}
          onReveal={revealTaskImage}
        />
      </div>

      {maskEditing && (
        <MaskEditorLayer
          image={refs[0]}
          onDone={(m) => { setMask(m); setMaskEditing(false); }}
          onClose={() => setMaskEditing(false)}
        />
      )}
      </div>

      {/* 模板面板是整屏模态，挂在最外层避免被参数区的 overflow 裁掉 */}
      {tplOpen && (
        <TemplatePanel
          onApply={applyTemplate}
          onClose={() => setTplOpen(false)}
          onNotice={setNotice}
        />
      )}
    </>
  );
}