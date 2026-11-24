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
  scanMode: boolean;
  scanCount: number;
  onSize: (v: string) => void;
  onQuality: (v: string) => void;
  onSeed: (v: string) => void;
  onCount: (v: number) => void;
  onScanMode: (v: boolean) => void;
  onScanCount: (v: number) => void;
}

export default function ParamRow({
  caps, size, quality, seed, count, scanMode, scanCount,
  onSize, onQuality, onSeed, onCount, onScanMode, onScanCount,
}: Props) {
  const sizeOptions = caps?.sizes ?? [];
  const qualityOptions = caps?.quality ?? [];
  const seedEnabled = caps ? isSeedEnabled(caps) : false;
  // 列宽要跟实际渲染出来的控件数量对上，否则会多出空列
  const paramCols =
    sizeOptions.length > 0 && qualityOptions.length > 0
      ? "grid-cols-[minmax(0,1fr)_minmax(0,1fr)_3.5rem_3.5rem]"
      : sizeOptions.length > 0 || qualityOptions.length > 0
        ? "grid-cols-[minmax(0,1fr)_3.5rem_3.5rem]"
        : "grid-cols-[3.5rem_3.5rem]";

  return (
    <div>
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
          <label className={labelCls} title={scanMode ? "扫描的起始种子，逐个 +1" : "仅部分协议/模型支持，以能力表为准"}>
            {scanMode ? "起始种子" : "种子"}
          </label>
          <input
            type="number"
            className={selCls}
            value={seed}
            onChange={(e) => onSeed(e.target.value)}
            placeholder={scanMode ? "0" : "随机"}
            disabled={caps ? !isSeedEnabled(caps) : false}
            title={caps && !isSeedEnabled(caps) ? "当前协议/模型不支持种子" : scanMode ? "扫描的起始种子，逐个 +1" : "仅部分协议/模型支持，以能力表为准"}
          />
        </div>
        {/* 扫描模式每个任务只出 1 张，「数量」不参与，留着会误导 */}
        {!scanMode && (
          <div>
            <label className={labelCls}>数量</label>
            <Select
              className={selCls}
              value={String(count)}
              onChange={(v) => onCount(Number(v))}
              options={[1, 2, 3, 4, 6, 8].map((n) => ({ value: String(n), label: String(n) }))}
            />
          </div>
        )}
      </div>

      <div className="mt-2 flex items-center gap-2">
        <button
          type="button"
          role="switch"
          aria-checked={scanMode}
          disabled={caps ? !seedEnabled : false}
          onClick={() => onScanMode(!scanMode)}
          title={caps && !seedEnabled ? "当前协议/模型不支持种子，无法扫描" : "同一条提示词，按连续种子拆成多个任务"}
          className={`rounded-md px-2 py-0.5 text-[11px] transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
            scanMode
              ? "bg-orange-100 font-medium text-orange-700 dark:bg-orange-950 dark:text-orange-400"
              : "text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
          }`}
        >{`种子扫描 ${scanMode ? "开" : "关"}`}</button>
        {scanMode && (
          <>
            <Select
              className={`${selCls} w-20!`}
              value={String(scanCount)}
              onChange={(v) => onScanCount(Number(v))}
              ariaLabel="扫描张数"
              options={[2, 4, 6, 8, 12, 16, 24, 32].map((n) => ({ value: String(n), label: `${n} 张` }))}
            />
            <span className="text-[11px] text-neutral-500">
              种子 {seed.trim() || "0"} 起连续 {scanCount} 个，每任务 1 张
            </span>
          </>
        )}
      </div>
    </div>
  );
}
