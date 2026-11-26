import { useMemo, useState } from "react";
import { api } from "../../ipc";
import {
  composeStep,
  composeTemplate,
  defaultValues,
  type PromptTemplate,
} from "../../lib/template";
import { TEMPLATE_CATEGORIES, TEMPLATE_LIBRARY } from "../../templates";
import type { Notify } from "../../stores/toast";
import { btnChipCls, btnCls, btnCompactCls, btnGhostCls, btnPrimaryCls, btnRowCls, inputCompactCls, labelCls, textareaCls } from "../../ui/controls";

export interface TemplateApplyPayload {
  prompt: string;
  ratio?: string;
  title: string;
  /** true = 覆盖提示词框；false = 追加到末尾 */
  replace: boolean;
}

interface Props {
  onApply: (payload: TemplateApplyPayload) => void;
  onClose: () => void;
  onNotice?: Notify;
}

/** 各分类的模板数，静态数据，模块加载时算一次。 */
const CATEGORY_COUNTS = new Map(
  TEMPLATE_CATEGORIES.map((c) => [
    c,
    TEMPLATE_LIBRARY.filter((t) => t.category === c).length,
  ]),
);

/**
 * 提示词模板面板（模态）。
 *
 * 版式按「筛选条 + 内容区」的成熟做法排：
 * 顶栏（标题/关闭）→ 搜索条整行 → 分类标签整条 → 左列表 + 右详情。
 * 分类放成一整条横向标签，比原先塞在左栏里更省纵向空间，
 * 模板变多以后左栏只承担列表，不再同时承担导航。
 */
export default function TemplatePanel({ onApply, onClose, onNotice }: Props) {
  const [q, setQ] = useState("");
  const [cat, setCat] = useState<string>("");
  const [activeId, setActiveId] = useState<string>(TEMPLATE_LIBRARY[0].id);
  const [values, setValues] = useState<Record<string, string>>(() =>
    defaultValues(TEMPLATE_LIBRARY[0]),
  );

  const searching = q.trim().length > 0;
  // 搜索是跨分类的，所以搜索时把标签高亮收回到「全部」，避免标签和结果对不上。
  const shownCat = searching ? "" : cat;

  const list = useMemo(() => {
    const kw = q.trim().toLowerCase();
    if (!kw) {
      return cat ? TEMPLATE_LIBRARY.filter((t) => t.category === cat) : TEMPLATE_LIBRARY;
    }
    return TEMPLATE_LIBRARY.filter(
      (t) =>
        t.title.toLowerCase().includes(kw) ||
        t.summary.toLowerCase().includes(kw) ||
        t.category.includes(kw) ||
        (t.tags ?? []).some((tag) => tag.toLowerCase().includes(kw)),
    );
  }, [q, cat]);

  // 列表按分类分组：选具体分类时只有一组，就不显示组头。
  const groups = useMemo(() => {
    const map = new Map<string, PromptTemplate[]>();
    for (const t of list) {
      const arr = map.get(t.category);
      if (arr) arr.push(t);
      else map.set(t.category, [t]);
    }
    return TEMPLATE_CATEGORIES.filter((c) => map.has(c)).map((c) => ({
      category: c,
      items: map.get(c) ?? [],
    }));
  }, [list]);

  const active: PromptTemplate =
    TEMPLATE_LIBRARY.find((t) => t.id === activeId) ?? TEMPLATE_LIBRARY[0];

  const pick = (t: PromptTemplate) => {
    setActiveId(t.id);
    setValues(defaultValues(t));
  };

  const changeCat = (next: string) => {
    setQ("");
    setCat(next);
    const first = TEMPLATE_LIBRARY.find((t) => next === "" || t.category === next);
    if (first) pick(first);
  };

  const composed = composeTemplate(active, values);
  const missing = composed.missing;

  const apply = (prompt: string, replace: boolean) => {
    if (prompt.trim().length === 0) return;
    onApply({ prompt, ratio: active.ratio, title: active.title, replace });
  };

  const copy = async () => {
    await navigator.clipboard.writeText(composed.prompt);
    onNotice?.("提示词已复制", "success");
  };

  const saveToLibrary = async () => {
    if (!composed.prompt.trim()) return;
    await api.promptSave({
      title: active.title,
      content: composed.prompt,
      tags: [active.category],
    });
    onNotice?.(`已存入提示词库：${active.title}`, "success");
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="flex h-[82vh] w-[min(1080px,94vw)] flex-col overflow-hidden rounded-xl bg-white shadow-xl dark:bg-neutral-900">
        {/* 顶栏 */}
        <div className="flex shrink-0 items-center gap-2 px-4 pb-2 pt-3">
          <span className="text-sm font-medium">提示词模板</span>
          <span className="text-[11px] text-neutral-400">
            {TEMPLATE_LIBRARY.length} 个模板 · 业内标准结构 · 填字段即可生成
          </span>
          <button className={`${btnGhostCls} ml-auto`} onClick={onClose}>
            关闭
          </button>
        </div>

        {/* 搜索：整行 */}
        <div className="shrink-0 px-4 pb-2">
          <div className="relative">
            <svg
              className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-neutral-400"
              viewBox="0 0 20 20"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              aria-hidden
            >
              <circle cx="8.5" cy="8.5" r="5.5" />
              <path d="M13 13l4 4" strokeLinecap="round" />
            </svg>
            <input
              className={`${inputCompactCls} pl-8`}
              placeholder="搜模板名 / 分类 / 关键词…"
              value={q}
              onChange={(e) => setQ(e.target.value)}
            />
          </div>
        </div>

        {/* 分类标签：整条横向，窄屏可左右滚动；滚动条会压住标签，直接隐藏 */}
        <div className="shrink-0 overflow-x-auto border-b border-neutral-200 px-4 pb-2 [scrollbar-width:none] dark:border-neutral-800 [&::-webkit-scrollbar]:hidden">
          <div className="flex w-max gap-1">
            <button className={btnChipCls(shownCat === "")} onClick={() => changeCat("")}>
              全部
              <span className="ml-1 text-[10px] opacity-70">{TEMPLATE_LIBRARY.length}</span>
            </button>
            {TEMPLATE_CATEGORIES.map((c) => (
              <button
                key={c}
                className={btnChipCls(shownCat === c)}
                onClick={() => changeCat(c)}
              >
                {c}
                <span className="ml-1 text-[10px] opacity-70">{CATEGORY_COUNTS.get(c)}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="flex min-h-0 flex-1">
          {/* 左：模板列表 */}
          <div className="flex w-64 shrink-0 flex-col border-r border-neutral-200 dark:border-neutral-800">
            <div className="min-h-0 flex-1 overflow-y-auto p-2">
              {groups.map((g) => (
                <div key={g.category}>
                  {shownCat === "" && (
                    <div className="flex items-baseline gap-1 px-2 pb-1 pt-2 text-[10px] font-medium tracking-wide text-neutral-400">
                      {g.category}
                      <span className="opacity-60">{g.items.length}</span>
                    </div>
                  )}
                  {g.items.map((t) => (
                    <button
                      key={t.id}
                      className={`mb-1 w-full rounded-md px-2 py-1.5 text-left ${
                        t.id === active.id
                          ? "bg-orange-50 ring-1 ring-orange-300 dark:bg-orange-950/50 dark:ring-orange-800"
                          : "hover:bg-neutral-100 dark:hover:bg-neutral-800"
                      }`}
                      onClick={() => pick(t)}
                    >
                      <div className="text-xs font-medium">{t.title}</div>
                      <div className="mt-0.5 line-clamp-2 text-[11px] leading-4 text-neutral-500">
                        {t.summary}
                      </div>
                    </button>
                  ))}
                </div>
              ))}
              {list.length === 0 && (
                <p className="py-10 text-center text-xs text-neutral-400">没有匹配的模板</p>
              )}
            </div>
          </div>

          {/* 右：模板详情 */}
          <div className="flex min-w-0 flex-1 flex-col">
            <div className="min-h-0 flex-1 overflow-y-auto p-4">
              <div className="flex flex-wrap items-center gap-2">
                <h2 className="text-sm font-medium">{active.title}</h2>
                <span className="rounded bg-neutral-100 px-1.5 py-0.5 text-[10px] text-neutral-500 dark:bg-neutral-800">
                  {active.category}
                </span>
                {active.ratio && (
                  <span className="rounded bg-neutral-100 px-1.5 py-0.5 text-[10px] text-neutral-500 dark:bg-neutral-800">
                    建议 {active.ratio}
                  </span>
                )}
                {active.needsRef && (
                  <span className="rounded bg-amber-50 px-1.5 py-0.5 text-[10px] text-amber-700 dark:bg-amber-950/50 dark:text-amber-400">
                    建议先上传参考图
                  </span>
                )}
              </div>
              <p className="mt-1 text-[11px] text-neutral-500">{active.summary}</p>

              {/* 字段 */}
              <div className="mt-4 space-y-2.5">
                {active.fields.map((f) => (
                  <div key={f.key}>
                    <label className={labelCls}>
                      {f.label}
                      {f.required && <span className="ml-1 text-orange-500">*</span>}
                    </label>
                    {f.multiline ? (
                      <textarea
                        className={`${textareaCls} min-h-[64px] resize-y text-xs`}
                        placeholder={f.default ?? f.hint ?? ""}
                        value={values[f.key] ?? ""}
                        onChange={(e) => setValues((prev) => ({ ...prev, [f.key]: e.target.value }))}
                      />
                    ) : (
                      <input
                        className={inputCompactCls}
                        placeholder={f.default ?? f.hint ?? ""}
                        value={values[f.key] ?? ""}
                        onChange={(e) => setValues((prev) => ({ ...prev, [f.key]: e.target.value }))}
                      />
                    )}
                    {/* 提示只在和 placeholder 不同时再显示一遍，避免同一句话出现两次 */}
                    {f.hint && f.hint !== (f.default ?? f.hint) && (
                      <p className="mt-0.5 text-[10px] leading-4 text-neutral-400">{f.hint}</p>
                    )}
                  </div>
                ))}
              </div>

              {/* 预览 */}
              <div className="mt-4">
                <div className="mb-1 flex items-center gap-2 text-[11px] text-neutral-500">
                  <span>合成预览</span>
                  {missing.length > 0 && (
                    <span className="text-orange-600">还差：{missing.join("、")}</span>
                  )}
                </div>
                <pre className="max-h-48 overflow-y-auto whitespace-pre-wrap rounded-md bg-neutral-50 p-2 text-[11px] leading-4 text-neutral-700 dark:bg-neutral-800/60 dark:text-neutral-200">
                  {composed.prompt || "（填上面的字段就会出现提示词）"}
                </pre>
              </div>

              {/* 工作流步骤 */}
              {active.steps && active.steps.length > 0 && (
                <div className="mt-4">
                  <div className="mb-1 text-[11px] text-neutral-500">
                    分步工作流（按顺序出图，每步都可单独插入）
                  </div>
                  <div className="space-y-1.5">
                    {active.steps.map((s) => {
                      const text = composeStep(s, values);
                      return (
                        <div
                          key={s.title}
                          className="rounded-md border border-neutral-200 p-2 dark:border-neutral-800"
                        >
                          <div className="flex items-center gap-2">
                            <span className="text-[11px] font-medium">{s.title}</span>
                            <div className="ml-auto flex gap-1">
                              <button
                                className={btnRowCls}
                                onClick={() => void navigator.clipboard.writeText(text)}
                              >
                                复制
                              </button>
                              <button
                                className={btnCompactCls}
                                onClick={() => apply(text, true)}
                              >
                                插入
                              </button>
                            </div>
                          </div>
                          <p className="mt-1 line-clamp-2 text-[10px] leading-4 text-neutral-500">
                            {text}
                          </p>
                          {s.note && (
                            <p className="mt-1 text-[10px] leading-4 text-amber-600 dark:text-amber-400">
                              {s.note}
                            </p>
                          )}
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>

            {/* 操作条常驻底部，模板再长也点得到 */}
            <div className="flex shrink-0 flex-wrap gap-2 border-t border-neutral-200 px-4 py-3 dark:border-neutral-800">
              <button
                className={btnPrimaryCls}
                disabled={missing.length > 0 || composed.prompt.trim().length === 0}
                onClick={() => apply(composed.prompt, false)}
              >
                追加到提示词
              </button>
              <button
                className={btnCls}
                disabled={missing.length > 0 || composed.prompt.trim().length === 0}
                onClick={() => apply(composed.prompt, true)}
              >
                替换提示词
              </button>
              <button className={btnCompactCls} onClick={() => void copy()}>
                复制
              </button>
              <button className={btnCompactCls} onClick={() => void saveToLibrary()}>
                存入提示词库
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
