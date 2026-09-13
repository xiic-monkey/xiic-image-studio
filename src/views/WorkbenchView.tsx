import { useCallback, useEffect, useRef, useState } from "react";
import MaskEditor from "../components/MaskEditor";
import Select from "../components/Select";
import TaskPanel, { useTaskFeed, type TaskView } from "../components/TaskPanel";
import { api } from "../ipc";
import { usePrefill } from "../stores/prefill";
import { useProviders } from "../stores/providers";
import { PROTOCOL_LABELS, type RefImage, type SessionItem } from "../types";

const SIZE_PRESETS = ["1024x1024", "1536x1024", "1024x1536", "2048x2048"];

function fileToRef(file: File): Promise<RefImage> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = String(reader.result);
      const [head, data] = dataUrl.split(",");
      const mime = head.slice(5).split(";")[0] || "image/png";
      resolve({ name: file.name, mime, data });
    };
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

export default function WorkbenchView() {
  const { providers, load } = useProviders();
  const { tasks, imgUrls, upsert, cancel, remove } = useTaskFeed();

  const [providerId, setProviderId] = useState("");
  const [prompt, setPrompt] = useState("");
  const [size, setSize] = useState("1024x1024");
  const [quality, setQuality] = useState("auto");
  const [count, setCount] = useState(1);
  const [refs, setRefs] = useState<RefImage[]>([]);
  const [mask, setMask] = useState<RefImage | null>(null);
  const [maskEditing, setMaskEditing] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameVal, setRenameVal] = useState("");
  const areaRef = useRef<HTMLDivElement>(null);
  const tabsRef = useRef<HTMLDivElement>(null);

  // ---- 多会话 ----
  const [sessions, setSessions] = useState<SessionItem[]>([]);
  const [sessionId, setSessionId] = useState("");
  const skipSave = useRef(true);

  const refreshSessions = useCallback(async (preferId?: string) => {
    const list = await api.sessionList();
    setSessions(list);
    // preferId 缺省时选第一个（挂载 / 删除当前会话后的场景，不依赖闭包里的旧 sessionId）
    const target = preferId ?? list[0]?.id;
    if (target && list.some((s) => s.id === target)) {
      setSessionId(target);
      const s = list.find((x) => x.id === target)!;
      skipSave.current = true;
      setProviderId((prev) => s.draft.providerId ?? prev);
      setPrompt(s.draft.prompt ?? "");
      setSize(s.draft.size ?? "1024x1024");
      setQuality(s.draft.quality ?? "auto");
      setCount(s.draft.count ?? 1);
    } else if (list.length === 0) {
      const id = await api.sessionSave({ name: "默认会话" });
      setSessions(await api.sessionList());
      setSessionId(id);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    refreshSessions();
  }, [refreshSessions]);

  // 草稿自动保存（debounce 800ms；refs 不入草稿）
  useEffect(() => {
    if (!sessionId) return;
    if (skipSave.current) {
      skipSave.current = false;
      return;
    }
    const t = setTimeout(() => {
      api.sessionSave({
        id: sessionId,
        draft: { providerId, prompt, size, quality, count },
      });
    }, 800);
    return () => clearTimeout(t);
  }, [sessionId, providerId, prompt, size, quality, count]);

  // ---- 画廊/提示词回填 ----
  const prefill = usePrefill((s) => s.prefill);
  useEffect(() => {
    if (!prefill) return;
    setPrompt(prefill.prompt ?? "");
    if (prefill.size) setSize(prefill.size);
    if (prefill.quality) setQuality(prefill.quality);
    if (prefill.count) setCount(prefill.count);
    if (prefill.refs?.length) setRefs((prev) => [...prefill.refs!, ...prev].slice(0, 4));
  }, [prefill]);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    if (!providerId && providers.length > 0) setProviderId(providers[0].id);
  }, [providers, providerId]);

  const provider = providers.find((p) => p.id === providerId);
  const showQuality = provider?.protocol === "openai_images";
  const supportsMask = provider?.protocol === "openai_images";

  const addFiles = useCallback(async (files: FileList | File[]) => {
    const imgs = Array.from(files).filter((f) => f.type.startsWith("image/"));
    const newRefs = await Promise.all(imgs.slice(0, 4).map(fileToRef));
    setRefs((prev) => [...prev, ...newRefs].slice(0, 4));
  }, []);

  // 粘贴图片
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const files = Array.from(e.clipboardData?.files ?? []);
      if (files.length > 0) addFiles(files);
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, [addFiles]);

  const submit = async () => {
    if (!provider) {
      setNotice("请先在「供应商」页添加并保存供应商");
      return;
    }
    if (!prompt.trim()) {
      setNotice("请输入提示词");
      return;
    }
    setSubmitting(true);
    setNotice(null);
    try {
      const lines = prompt.trim().split("\n").map((s) => s.trim()).filter(Boolean);
      const batch = lines.length > 1 ? lines : [prompt.trim()];
      const n = batch.length > 1 ? 1 : count;
      let submitted = 0;
      let failMsg: string | null = null;

      for (const line of batch) {
        try {
          const taskId = await api.generateSubmit({
            provider_id: provider.id,
            prompt: line,
            n,
            size: size || undefined,
            quality: showQuality ? (quality === "auto" ? undefined : quality) : undefined,
            refs,
            mask: supportsMask && mask ? mask : undefined,
          });
          upsert({ id: taskId, status: "running", done: 0, total: n, errors: [], images: [], createdAt: new Date().toISOString(), prompt: line });
          submitted += 1;
        } catch (e) {
          failMsg = String(e);
          break;
        }
      }

      // 界面状态必须和"实际提交了什么"严格对齐：
      // 已提交的行从输入框移除，失败那行连同它后面的原样留着（用户改完可以接着发）；
      // 参考图只在全部成功时才清——部分失败时，剩下的行还要继续用它。
      if (failMsg === null) {
        setPrompt("");
        setRefs([]);
        setMask(null);
        setNotice(batch.length > 1 ? `已提交 ${submitted} 个任务` : null);
      } else {
        setPrompt(batch.slice(submitted).join("\n"));
        setNotice(`已提交 ${submitted}/${batch.length} 条，第 ${submitted + 1} 条提交失败：${failMsg}`);
      }
    } catch (e) {
      setNotice(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  // 一键重试：复用原任务的 prompt / 供应商 / 尺寸 / 质量 / 数量。
  // 参考图本体没入库，所以只有 refCount === 0 的任务才会走到这里（卡片上才有重试按钮）。
  const retry = useCallback(
    async (t: TaskView) => {
      if (!t.providerId || !t.prompt) {
        setNotice("该任务缺少可重试的参数（供应商或提示词为空）");
        return;
      }
      try {
        const taskId = await api.generateSubmit({
          provider_id: t.providerId,
          prompt: t.prompt,
          n: t.n || 1,
          size: t.size ?? undefined,
          quality: t.quality ?? undefined,
          refs: [],
        });
        upsert({
          id: taskId,
          status: "running",
          done: 0,
          total: t.n || 1,
          errors: [],
          images: [],
          createdAt: new Date().toISOString(),
          prompt: t.prompt,
        });
        setNotice("已按原参数重新提交");
      } catch (e) {
        setNotice(`重试失败：${String(e)}`);
      }
    },
    [upsert],
  );

  const delSession = async (id: string) => {
    if (sessions.length <= 1) return setNotice("至少保留一个会话");
    if (!confirm("删除该会话？（不影响已生成图片）")) return;
    await api.sessionDelete(id);
    // 删的是当前会话 → 选第一个；删的是别的 → 保持在当前
    refreshSessions(id === sessionId ? undefined : sessionId);
  };

  // 新会话命名：取最小的未占用编号（存在「会话3」时新建「会话1」，再「会话2」，再「会话4」）
  const newSession = async () => {
    const names = new Set(sessions.map((s) => s.name));
    let i = 1;
    while (names.has(`会话 ${i}`)) i++;
    const id = await api.sessionSave({ name: `会话 ${i}` });
    await refreshSessions(id);
    // 渲染提交后把 tab 条滚到最右，露出新 tab（列表按创建时间升序，新 tab 在末尾）
    requestAnimationFrame(() => requestAnimationFrame(() => {
      const el = tabsRef.current;
      if (el) el.scrollLeft = el.scrollWidth;
    }));
  };

  const startRename = (id: string) => {
    setRenameVal(sessions.find((s) => s.id === id)?.name ?? "");
    setRenamingId(id);
  };

  const commitRename = async () => {
    const id = renamingId;
    setRenamingId(null);
    if (!id) return;
    const name = renameVal.trim();
    if (!name) return;
    await api.sessionSave({ id, name });
    refreshSessions(id);
  };

  // 右键菜单：点击外部 / Esc / resize 关闭
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

  const inputCls =
    "w-full rounded-md border border-neutral-300 bg-white px-3 py-2 text-sm focus:border-orange-500 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100";
  // 参数下拉紧凑版（对齐市面生图工具的 30px 控件规格）
  const selCls =
    "w-full rounded-md border border-neutral-300 bg-white px-2.5 py-1.5 text-xs focus:border-orange-500 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100";
  const labelCls = "mb-0.5 block text-[11px] font-medium text-neutral-500 dark:text-neutral-400";

  return (
    <div className="flex h-full">
      {/* 左：参数区 */}
      <div ref={areaRef} className="w-[380px] shrink-0 space-y-4 overflow-y-auto border-r border-neutral-200 px-5 pb-5 pt-3.5 dark:border-neutral-800"
        onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(e) => { e.preventDefault(); setDragOver(false); addFiles(e.dataTransfer.files); }}>
        <h1 className="text-base font-medium leading-6">工作台</h1>

        {/* 会话 tabs 行：tabs 可横向滚动，+ 固定在行尾；sticky 吸顶，随左栏滚动时保持可见 */}
        <div className="sticky top-0 z-10 flex items-center gap-1 border-b border-neutral-200 bg-white pb-2 pt-1 dark:border-neutral-800">
          <div ref={tabsRef} className="no-scrollbar -mx-1 flex min-w-0 flex-1 items-center gap-1 overflow-x-auto px-1 py-0.5">
            {sessions.map((s) => {
              const active = s.id === sessionId;
              const isRenaming = renamingId === s.id;
              return (
                <div
                  key={s.id}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    setMenu({ x: Math.min(e.clientX, window.innerWidth - 140), y: Math.min(e.clientY, window.innerHeight - 90), id: s.id });
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
                        else if (e.key === "Escape") setRenamingId(null);
                      }}
                      onBlur={commitRename}
                      className="w-[100px] rounded border border-orange-400 bg-white px-1 py-0.5 text-xs outline-none dark:bg-neutral-900"
                    />
                  ) : (
                    <>
                      <button
                        className="max-w-[110px] truncate"
                        title={s.name}
                        onClick={() => {
                          if (active) return;
                          skipSave.current = true;
                          refreshSessions(s.id);
                        }}
                      >{s.name}</button>
                      {sessions.length > 1 && (
                        <button
                          title="删除该会话"
                          aria-label={`删除会话 ${s.name}`}
                          className={`-mr-0.5 rounded text-[13px] leading-none hover:text-red-500 ${
                            active ? "opacity-60 hover:opacity-100" : "opacity-0 group-hover:opacity-60"
                          }`}
                          onClick={(e) => { e.stopPropagation(); delSession(s.id); }}
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
            onClick={newSession}
          >+</button>
        </div>

        <div>
          <label className={labelCls}>供应商 / 模型</label>
          <Select
            className={selCls}
            value={providerId}
            onChange={setProviderId}
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
          <label className={labelCls}>提示词（多行 = 批量任务，每行一张）</label>
          <textarea className={`${inputCls} min-h-[120px] resize-y`} value={prompt}
            onChange={(e) => setPrompt(e.target.value)} placeholder="描述你想要的画面…&#10;需要批量时，一行写一个提示词即可" />
        </div>

        <div>
          <div className="flex items-center justify-between">
            <label className={labelCls}>参考图（最多 4 张，拖拽 / 粘贴 / 点击添加）</label>
            {refs.length > 0 && supportsMask && (
              <button className="text-xs text-orange-600 hover:underline" onClick={() => setMaskEditing(true)}>
                {mask ? "重新涂抹" : "涂抹重绘"}
              </button>
            )}
          </div>
          <button
            className={`flex h-20 w-full items-center justify-center rounded-md border border-dashed text-xs transition-colors ${
              dragOver ? "border-orange-500 bg-orange-50 text-orange-600" : "border-neutral-300 text-neutral-400 hover:border-neutral-400 dark:border-neutral-700"
            }`}
            onClick={() => {
              const input = document.createElement("input");
              input.type = "file";
              input.accept = "image/*";
              input.multiple = true;
              input.onchange = () => input.files && addFiles(input.files);
              input.click();
            }}
          >
            + 添加参考图（图生图 / 局部编辑）
          </button>
          {refs.length > 0 && (
            <div className="mt-2 flex gap-2">
              {refs.map((r, i) => (
                <div key={i} className="relative">
                  <img src={`data:${r.mime};base64,${r.data}`} className={`h-14 w-14 rounded object-cover ${i === 0 && mask ? "outline outline-2 outline-orange-500" : ""}`} alt="" />
                  <button className="absolute -right-1.5 -top-1.5 flex h-5 w-5 items-center justify-center rounded-full bg-neutral-800 text-xs text-white"
                    onClick={() => { setRefs(refs.filter((_, j) => j !== i)); if (i === 0) setMask(null); }}>×</button>
                </div>
              ))}
              {mask && (
                <span className="self-center rounded bg-orange-50 px-2 py-1 text-[11px] text-orange-600 dark:bg-orange-950">
                  重绘模式 · 首图为底图
                  <button className="ml-1 text-orange-400 hover:underline" onClick={() => setMask(null)}>清除</button>
                </span>
              )}
            </div>
          )}
        </div>

        {/* 参数行：尺寸 / 质量 / 数量并排（数量列定宽只放数字放行尾，把宽度让给前两个；质量仅 openai_images 协议显示） */}
        <div className={`grid gap-2 ${showQuality ? "grid-cols-[minmax(0,1fr)_minmax(0,1fr)_4.5rem]" : "grid-cols-[minmax(0,1fr)_4.5rem]"}`}>
          <div>
            <label className={labelCls}>尺寸</label>
            <Select
              className={selCls}
              value={size}
              onChange={setSize}
              options={SIZE_PRESETS.map((s) => ({ value: s, label: s }))}
            />
          </div>
          {showQuality && (
            <div>
              <label className={labelCls}>质量</label>
              <Select
                className={selCls}
                value={quality}
                onChange={setQuality}
                options={["auto", "low", "medium", "high"].map((q) => ({ value: q, label: q }))}
              />
            </div>
          )}
          <div>
            <label className={labelCls}>数量</label>
            <Select
              className={selCls}
              value={String(count)}
              onChange={(v) => setCount(Number(v))}
              options={[1, 2, 3, 4, 6, 8].map((n) => ({ value: String(n), label: String(n) }))}
            />
          </div>
        </div>

        {notice && <div className="rounded-md bg-blue-50 px-3 py-2 text-xs text-blue-700 dark:bg-blue-950 dark:text-blue-400">{notice}</div>}

        <button className="w-full rounded-md bg-orange-600 py-1.5 text-sm font-medium text-white hover:bg-orange-700 disabled:opacity-50"
          onClick={submit} disabled={submitting}>
          {submitting ? "提交中…" : "生成"}
        </button>
      </div>

      {/* 右：任务面板 */}
      <div className="min-w-0 flex-1 overflow-y-auto">
        <div className="sticky top-0 z-10 border-b border-neutral-200 bg-white/90 px-5 py-3 text-sm font-medium backdrop-blur dark:border-neutral-800 dark:bg-neutral-950/90">
          任务队列
        </div>
        <TaskPanel tasks={tasks} imgUrls={imgUrls} onCancel={cancel} onDelete={remove} onRetry={retry} />
      </div>

      {/* 会话 tab 右键菜单 */}
      {menu && (
        <div
          className="fixed z-[100] min-w-[120px] rounded-md border border-neutral-200 bg-white py-1 shadow-lg dark:border-neutral-700 dark:bg-neutral-900"
          style={{ left: menu.x, top: menu.y }}
          onMouseDown={(e) => e.stopPropagation()}
        >
          <button
            className="block w-full px-3 py-1.5 text-left text-xs text-neutral-700 hover:bg-neutral-100 dark:text-neutral-200 dark:hover:bg-neutral-800"
            onClick={() => { startRename(menu.id); setMenu(null); }}
          >重命名</button>
          <button
            className="block w-full px-3 py-1.5 text-left text-xs text-red-600 hover:bg-red-50 dark:hover:bg-red-950"
            onClick={() => { const id = menu.id; setMenu(null); delSession(id); }}
          >删除</button>
        </div>
      )}

      {maskEditing && refs[0] && (
        <MaskEditor
          image={refs[0]}
          onDone={(m) => { setMask(m); setMaskEditing(false); }}
          onClose={() => setMaskEditing(false)}
        />
      )}
    </div>
  );
}
