import { useEffect, useState } from "react";
import { api } from "../../ipc";
import { errorMessage } from "../../lib/error";
import { btnCompactCls, inputCompactCls } from "../../ui/controls";
import type { PresetItem } from "../../types";

interface Props {
  /** 套用配方：把提示词与参数一起送回工作台。 */
  onApply: (p: PresetItem) => void;
  /** 当前表单快照，用来「存为配方」时预填。 */
  current: { prompt: string; size: string; quality: string; seed: string; providerId: string; modelId: string };
  onNotice: (msg: string) => void;
}

/**
 * 配方面板：列出已存的配方，可套用、可删除，也能把当前表单存成新配方。
 * 与「模板」的区别是模板只给提示词文本，配方带完整参数（尺寸/质量/种子/供应商）。
 */
export default function PresetPanel({ onApply, current, onNotice }: Props) {
  const [items, setItems] = useState<PresetItem[]>([]);
  const [name, setName] = useState("");

  const reload = () => api.presetList().then(setItems).catch(() => {});
  useEffect(() => {
    void reload();
  }, []);

  const save = async () => {
    const n = name.trim();
    if (!n) {
      onNotice("给配方起个名字");
      return;
    }
    if (!current.prompt.trim()) {
      onNotice("当前没有提示词，没法存配方");
      return;
    }
    try {
      await api.presetSave({
        name: n,
        prompt: current.prompt.trim(),
        size: current.size || null,
        quality: current.quality || null,
        seed: current.seed.trim() ? Number(current.seed.trim()) : null,
        provider_id: current.providerId || null,
        model_id: current.modelId || null,
      });
      setName("");
      await reload();
      onNotice(`已保存配方「${n}」`);
    } catch (err) {
      onNotice(errorMessage(err));
    }
  };

  const remove = async (p: PresetItem) => {
    if (!confirm(`删除配方「${p.name}」？`)) return;
    try {
      await api.presetDelete(p.id);
      await reload();
    } catch (err) {
      onNotice(errorMessage(err));
    }
  };

  return (
    <div className="rounded-md border border-neutral-200 p-3 dark:border-neutral-800">
      <div className="mb-2 text-xs font-medium text-neutral-500">配方（提示词 + 参数）</div>

      <div className="mb-2 flex gap-1.5">
        <input
          className={`${inputCompactCls} min-w-0 flex-1`}
          placeholder="把当前参数存为配方…"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") void save(); }}
        />
        <button
          className={`${btnCompactCls} shrink-0`}
          onClick={() => void save()}
          disabled={!name.trim() || !current.prompt.trim()}
        >保存</button>
      </div>

      <div className="space-y-1.5">
        {items.map((p) => (
          <div key={p.id} className="flex items-center gap-2">
            <button
              className="min-w-0 flex-1 truncate rounded-md px-2 py-1 text-left text-[11px] hover:bg-orange-50 dark:hover:bg-orange-950"
              onClick={() => onApply(p)}
              title={`${p.prompt}\n${p.size ?? ""} ${p.quality ?? ""} ${p.seed ?? ""}`}
            >
              <span className="font-medium text-neutral-700 dark:text-neutral-200">{p.name}</span>
              <span className="ml-1.5 text-neutral-400">
                {p.size ?? ""} {p.quality ?? ""}{p.seed != null ? ` seed=${p.seed}` : ""}
              </span>
            </button>
            <button
              className="shrink-0 rounded px-1.5 py-0.5 text-[11px] text-red-500 hover:bg-red-50 dark:hover:bg-red-950"
              onClick={() => void remove(p)}
            >删</button>
          </div>
        ))}
        {items.length === 0 && (
          <p className="py-2 text-center text-[11px] text-neutral-400">还没有配方，填好参数后在上方存一个</p>
        )}
      </div>
    </div>
  );
}
