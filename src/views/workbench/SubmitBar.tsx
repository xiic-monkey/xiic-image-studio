import { btnCls, btnPrimaryCls } from "../../ui/controls";

interface Props {
  submitting: boolean;
  tplOpen: boolean;
  presetOpen: boolean;
  onToggleTemplates: () => void;
  onTogglePresets: () => void;
  onImportBatch: () => void;
  onSubmit: () => void;
}

export default function SubmitBar({
  submitting, tplOpen, presetOpen, onToggleTemplates, onTogglePresets, onImportBatch, onSubmit,
}: Props) {
  return (
    <div className="flex gap-2">
      <button
        className={btnCls}
        onClick={onToggleTemplates}
        aria-expanded={tplOpen}
        disabled={submitting}
      >模板</button>
      <button
        className={btnCls}
        onClick={onTogglePresets}
        aria-expanded={presetOpen}
        disabled={submitting}
      >配方</button>
      <button
        className={btnCls}
        onClick={onImportBatch}
        disabled={submitting}
      >导入批量</button>
      <button
        className={`${btnPrimaryCls} flex-1 font-medium`}
        onClick={onSubmit}
        disabled={submitting}
      >{submitting ? "提交中…" : "生成"}</button>
    </div>
  );
}
