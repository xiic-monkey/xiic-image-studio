import type { RefImage } from "../../types";
import { labelCls } from "./styles";

interface Props {
  refs: RefImage[];
  mask: RefImage | null;
  maskEnabled: boolean;
  dragOver: boolean;
  onAddFiles: (files: FileList | File[]) => void;
  onRemove: (index: number) => void;
  onClearMask: () => void;
  onEditMask: () => void;
}

export default function RefImages({
  refs, mask, maskEnabled, dragOver, onAddFiles, onRemove, onClearMask, onEditMask,
}: Props) {
  return (
    <div>
      <div className="flex items-center justify-between">
        <label className={labelCls}>参考图（最多 4 张，拖拽 / 粘贴 / 点击添加）</label>
        {refs.length > 0 && maskEnabled && (
          <button className="text-xs text-orange-600 hover:underline" onClick={onEditMask}>
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
          input.onchange = () => input.files && onAddFiles(input.files);
          input.click();
        }}
      >
        + 添加参考图（图生图 / 局部编辑）
      </button>
      {refs.length > 0 && (
        <div className="mt-2 flex gap-2">
          {refs.map((r, i) => (
            <div key={i} className="relative">
              <img
                src={`data:${r.mime};base64,${r.data}`}
                className={`h-14 w-14 rounded object-cover ${i === 0 && mask ? "outline outline-2 outline-orange-500" : ""}`}
                alt=""
              />
              <button
                className="absolute -right-1.5 -top-1.5 flex h-5 w-5 items-center justify-center rounded-full bg-neutral-800 text-xs text-white"
                onClick={() => onRemove(i)}
              >×</button>
            </div>
          ))}
          {mask && (
            <span className="self-center rounded bg-orange-50 px-2 py-1 text-[11px] text-orange-600 dark:bg-orange-950">
              重绘模式 · 首图为底图
              <button className="ml-1 text-orange-400 hover:underline" onClick={onClearMask}>清除</button>
            </span>
          )}
        </div>
      )}
    </div>
  );
}
