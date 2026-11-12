interface Props {
  submitting: boolean;
  tplOpen: boolean;
  onToggleTemplates: () => void;
  onImportBatch: () => void;
  onSubmit: () => void;
}

export default function SubmitBar({ submitting, tplOpen, onToggleTemplates, onImportBatch, onSubmit }: Props) {
  return (
    <div className="flex gap-2">
      <button
        className="rounded-md border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-300 dark:hover:bg-neutral-800"
        onClick={onToggleTemplates}
        aria-expanded={tplOpen}
        disabled={submitting}
      >模板</button>
      <button
        className="rounded-md border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-300 dark:hover:bg-neutral-800"
        onClick={onImportBatch}
        disabled={submitting}
      >导入批量</button>
      <button
        className="flex-1 rounded-md bg-orange-600 py-1.5 text-sm font-medium text-white hover:bg-orange-700 disabled:opacity-50"
        onClick={onSubmit}
        disabled={submitting}
      >{submitting ? "提交中…" : "生成"}</button>
    </div>
  );
}
