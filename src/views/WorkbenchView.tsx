import { useCallback, useEffect, useState } from "react";
import Select from "../components/Select";
import TaskPanel from "../components/TaskPanel";
import { useDraft } from "../hooks/useDraft";
import { useSessions } from "../hooks/useSessions";
import { useSubmissions } from "../hooks/useSubmissions";
import { useTaskFeed } from "../hooks/useTaskFeed";
import { normalizeQuality, normalizeSize } from "../lib/capabilities";
import { filesToRefs, MAX_REFS } from "../lib/ref-image";
import { pickPrefillPrompt } from "../lib/submission";
import { usePrefill } from "../stores/prefill";
import { useProviders } from "../stores/providers";
import type { TemplateItem } from "../templates";
import { PROTOCOL_LABELS, type RefImage } from "../types";
import MaskEditorLayer from "./workbench/MaskEditorLayer";
import ParamRow from "./workbench/ParamRow";
import RefImages from "./workbench/RefImages";
import SessionTabs from "./workbench/SessionTabs";
import { inputCls, labelCls, selCls } from "./workbench/styles";
import SubmitBar from "./workbench/SubmitBar";
import TemplatePanel from "./workbench/TemplatePanel";

/** 组合层：状态与副作用在 hooks，展示在各 workbench 子组件，这里只负责接线。 */
export default function WorkbenchView() {
  const [notice, setNotice] = useState<string | null>(null);
  const { providers, load } = useProviders();
  const { tasks, imgUrls, upsert, cancel, remove } = useTaskFeed();
  const { sessions, sessionId, selectSession, newSession, deleteSession, renameSession } = useSessions(setNotice);
  const { form, setForm, patch } = useDraft(sessionId, sessions);

  const [refs, setRefs] = useState<RefImage[]>([]);
  const [mask, setMask] = useState<RefImage | null>(null);
  const [maskEditing, setMaskEditing] = useState(false);
  const [tplOpen, setTplOpen] = useState(false);
  const [dragOver, setDragOver] = useState(false);

  const { submit, importBatch, retry, submitting } = useSubmissions({
    providers,
    providerId: form.providerId,
    form,
    refs,
    mask,
    sessionId,
    upsert,
    patch,
    clearRefs: () => { setRefs([]); setMask(null); },
    setNotice,
  });

  const provider = providers.find((p) => p.id === form.providerId);
  const caps = provider?.capabilities;
  const prefill = usePrefill((s) => s.prefill);

  useEffect(() => {
    void load();
  }, [load]);

  // 用函数式更新读 prev：会话切换那一轮的 form 还是上个会话的，
  // 拿闭包里的旧值覆盖会把新会话草稿冲掉。
  useEffect(() => {
    if (providers.length === 0) return;
    setForm((prev) => (prev.providerId ? prev : { ...prev, providerId: providers[0].id }));
  }, [providers, form.providerId, setForm]);

  // 切换供应商/模型后旧的尺寸/质量可能不在新能力表内。只在展示层回落，
  // 不改草稿：写回表单会在切会话那一轮把新会话的草稿当成旧会话的来覆盖；
  // 提交时 buildGenerateRequest 仍会按能力表归一化，上游拿到的值始终是合法的。
  const displaySize = caps ? normalizeSize(caps, form.size) ?? form.size : form.size;
  const displayQuality = caps ? normalizeQuality(caps, form.quality) ?? form.quality : form.quality;

  // ---- 画廊/提示词回填 ----
  useEffect(() => {
    if (!prefill) return;
    setForm((prev) => ({ ...prev, prompt: pickPrefillPrompt(prev.prompt, prefill.prompt) }));
    if (prefill.size) patch({ size: prefill.size });
    if (prefill.quality) patch({ quality: prefill.quality });
    if (prefill.count) patch({ count: prefill.count });
    if (prefill.seed) patch({ seed: prefill.seed });
    if (prefill.refs?.length) {
      setRefs((prev) => [...prefill.refs!, ...prev].slice(0, MAX_REFS));
    }
  }, [prefill, setForm, patch]);

  const addFiles = useCallback(async (files: FileList | File[]) => {
    const newRefs = await filesToRefs(files);
    setRefs((prev) => [...prev, ...newRefs].slice(0, MAX_REFS));
  }, []);

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
    setRefs((prev) => prev.filter((_, j) => j !== i));
    if (i === 0) setMask(null);
  };

  const pickTemplate = (t: TemplateItem) => {
    setForm((prev) => ({ ...prev, prompt: prev.prompt ? `${prev.prompt}\n${t.prompt}` : t.prompt }));
    if (t.size) patch({ size: t.size });
    if (t.quality) patch({ quality: t.quality });
    setTplOpen(false);
  };

  return (
    <div className="flex h-full">
      {/* 左：参数区 */}
      <div
        className="w-[380px] shrink-0 space-y-4 overflow-y-auto border-r border-neutral-200 px-5 pb-5 pt-3.5 dark:border-neutral-800"
        onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(e) => { e.preventDefault(); setDragOver(false); void addFiles(e.dataTransfer.files); }}
      >
        <h1 className="text-base font-medium leading-6">工作台</h1>

        <SessionTabs
          sessions={sessions}
          sessionId={sessionId}
          onSelect={selectSession}
          onNew={newSession}
          onDelete={deleteSession}
          onRename={renameSession}
        />

        <div>
          <label className={labelCls}>供应商 / 模型</label>
          <Select
            className={selCls}
            value={form.providerId}
            onChange={(v) => patch({ providerId: v })}
            options={
              providers.length === 0
                ? [{ value: "", label: "（先去供应商页添加）" }]
                : providers.map((p) => ({
                    value: p.id,
                    label: `${p.name} · ${p.model || PROTOCOL_LABELS[p.protocol]}`,
                  }))
            }
          />
        </div>

        <div>
          <div className="flex items-center justify-between">
            <label className={labelCls}>提示词</label>
            <button
              type="button"
              role="switch"
              aria-checked={form.batchMode}
              onClick={() => setForm((prev) => ({ ...prev, batchMode: !prev.batchMode }))}
              className={`rounded-md px-2 py-0.5 text-[11px] transition-colors ${
                form.batchMode
                  ? "bg-orange-100 font-medium text-orange-700 dark:bg-orange-950 dark:text-orange-400"
                  : "text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
              }`}
            >{`按行批量 ${form.batchMode ? "开" : "关"}`}</button>
          </div>
          <textarea
            className={`${inputCls} min-h-[120px] resize-y`}
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
          onSize={(v) => patch({ size: v })}
          onQuality={(v) => patch({ quality: v })}
          onSeed={(v) => patch({ seed: v })}
          onCount={(v) => patch({ count: v })}
        />

        {notice && (
          <div className="rounded-md bg-blue-50 px-3 py-2 text-xs text-blue-700 dark:bg-blue-950 dark:text-blue-400">
            {notice}
          </div>
        )}

        <SubmitBar
          submitting={submitting}
          tplOpen={tplOpen}
          onToggleTemplates={() => setTplOpen((o) => !o)}
          onImportBatch={() => void importBatch()}
          onSubmit={() => void submit()}
        />

        {tplOpen && <TemplatePanel onPick={pickTemplate} onClose={() => setTplOpen(false)} />}
      </div>

      {/* 右：任务面板 */}
      <div className="min-w-0 flex-1 overflow-y-auto">
        <div className="sticky top-0 z-10 border-b border-neutral-200 bg-white/90 px-5 py-3 text-sm font-medium backdrop-blur dark:border-neutral-800 dark:bg-neutral-950/90">
          任务队列
        </div>
        <TaskPanel tasks={tasks} imgUrls={imgUrls} onCancel={cancel} onDelete={remove} onRetry={retry} />
      </div>

      {maskEditing && (
        <MaskEditorLayer
          image={refs[0]}
          onDone={(m) => { setMask(m); setMaskEditing(false); }}
          onClose={() => setMaskEditing(false)}
        />
      )}
    </div>
  );
}
