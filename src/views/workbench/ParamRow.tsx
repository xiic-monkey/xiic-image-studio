import Select from "../../components/Select";
import { isSeedEnabled } from "../../lib/capabilities.ts";
import type { Capabilities } from "../../types";
import { labelCls, selCls } from "./styles";

interface Props {
  caps?: Capabilities;
  size: string;
  quality: string;
  seed: string;
  count: number;
  onSize: (v: string) => void;
  onQuality: (v: string) => void;
  onSeed: (v: string) => void;
  onCount: (v: number) => void;
}

export default function ParamRow({ caps, size, quality, seed, count, onSize, onQuality, onSeed, onCount }: Props) {
  const sizeOptions = caps?.sizes ?? [];
  const qualityOptions = caps?.quality ?? [];
  // 列宽要跟实际渲染出来的控件数量对上，否则会多出空列
  const paramCols =
    sizeOptions.length > 0 && qualityOptions.length > 0
      ? "grid-cols-[minmax(0,1fr)_minmax(0,1fr)_3.5rem_3.5rem]"
      : sizeOptions.length > 0 || qualityOptions.length > 0
        ? "grid-cols-[minmax(0,1fr)_3.5rem_3.5rem]"
        : "grid-cols-[3.5rem_3.5rem]";

  return (
    <div className={`grid gap-2 ${paramCols}`}>
      {sizeOptions.length > 0 && (
        <div>
          <label className={labelCls}>{caps?.sizeLabel ?? "尺寸"}</label>
          <Select
            className={selCls}
            value={size}
            onChange={onSize}
            options={sizeOptions.map((s) => ({ value: s, label: s }))}
          />
        </div>
      )}
      {qualityOptions.length > 0 && (
        <div>
          <label className={labelCls}>质量</label>
          <Select
            className={selCls}
            value={quality}
            onChange={onQuality}
            options={qualityOptions.map((q) => ({ value: q, label: q }))}
          />
        </div>
      )}
      <div>
        <label className={labelCls} title="仅部分协议/模型支持，以能力表为准">种子</label>
        <input
          type="number"
          className={selCls}
          value={seed}
          onChange={(e) => onSeed(e.target.value)}
          placeholder="随机"
          disabled={caps ? !isSeedEnabled(caps) : false}
          title={caps && !isSeedEnabled(caps) ? "当前协议/模型不支持种子" : "仅部分协议/模型支持，以能力表为准"}
        />
      </div>
      <div>
        <label className={labelCls}>数量</label>
        <Select
          className={selCls}
          value={String(count)}
          onChange={(v) => onCount(Number(v))}
          options={[1, 2, 3, 4, 6, 8].map((n) => ({ value: String(n), label: String(n) }))}
        />
      </div>
    </div>
  );
}
