import { TEMPLATES, type TemplateItem } from "../../templates";

interface Props {
  onPick: (item: TemplateItem) => void;
  onClose: () => void;
}

export default function TemplatePanel({ onPick, onClose }: Props) {
  return (
    <div className="rounded-md border border-neutral-200 p-3 dark:border-neutral-800">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-xs font-medium text-neutral-500">插入预设模板（追加到提示词）</span>
        <button className="text-xs text-neutral-400 hover:underline" onClick={onClose}>关闭</button>
      </div>
      <div className="space-y-2">
        {TEMPLATES.map((c) => (
          <div key={c.cat}>
            <div className="text-[11px] font-medium text-neutral-500">{c.cat}</div>
            <div className="mt-1 flex flex-wrap gap-1.5">
              {c.items.map((t) => (
                <button
                  key={t.title}
                  className="rounded-md border border-neutral-200 px-2 py-1 text-[11px] hover:border-orange-300 hover:bg-orange-50 dark:border-neutral-800 dark:hover:border-orange-700 dark:hover:bg-orange-950"
                  onClick={() => onPick(t)}
                >{t.title}</button>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
