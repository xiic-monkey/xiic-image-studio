import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import Select from "../components/Select";
import { api } from "../ipc";
import { usePrefill } from "../stores/prefill";

async function pickPaths(multiple: boolean, patterns: string[]): Promise<string[]> {
  const picked = await open({ multiple, filters: [{ name: "files", extensions: patterns }] });
  if (!picked) return [];
  return Array.isArray(picked) ? picked : [picked];
}

export default function ToolsView() {
  const [status, setStatus] = useState<{ available: boolean; path: string; source: string; formats: Record<string, boolean> | null } | null>(null);
  const [customPath, setCustomPath] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [results, setResults] = useState<string[]>([]);
  const [resultUrls, setResultUrls] = useState<Record<string, string>>({});
  const [format, setFormat] = useState("webp");
  const [quality, setQuality] = useState(85);
  const [maxSide, setMaxSide] = useState(0);
  const [fps, setFps] = useState(4);
  const sendPrefill = usePrefill((s) => s.send);

  useEffect(() => {
    api.toolsStatus().then(setStatus);
  }, []);

  const loadUrl = async (path: string) => {
    if (resultUrls[path]) return;
    try {
      const url = await api.toolsFileB64(path);
      setResultUrls((prev) => ({ ...prev, [path]: url }));
    } catch {
      /* ignore */
    }
  };

  useEffect(() => {
    results.forEach(loadUrl);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [results]);

  const run = async (fn: () => Promise<string | string[]>) => {
    setBusy(true);
    setNotice(null);
    try {
      const out = await fn();
      const list = Array.isArray(out) ? out : [out];
      setResults(list);
      setNotice(`完成，共 ${list.length} 个输出`);
    } catch (e) {
      setNotice(String(e));
    } finally {
      setBusy(false);
    }
  };

  const doConvert = async () => {
    const paths = await pickPaths(true, ["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff"]);
    run(() => {
      if (paths.length === 0) return Promise.reject(new Error("未选择文件"));
      return api.toolsConvert(paths, format, quality, maxSide || undefined);
    });
  };

  const doGif = async () => {
    const paths = await pickPaths(true, ["png", "jpg", "jpeg", "webp"]);
    run(() => {
      if (paths.length < 2) return Promise.reject(new Error("GIF 需要选择 2 张以上图片"));
      return api.toolsGif(paths, fps, maxSide || undefined);
    });
  };

  const doFrames = async () => {
    const paths = await pickPaths(false, ["mp4", "mov", "mkv", "avi", "webm"]);
    run(() => {
      if (paths.length === 0) return Promise.reject(new Error("未选择视频"));
      return api.toolsVideoFrames(paths[0], 1, 16, maxSide || undefined);
    });
  };

  const asRef = (path: string) => {
    const url = resultUrls[path];
    if (!url) return;
    const [, data] = url.split(",");
    const mime = url.slice(5, url.indexOf(";"));
    sendPrefill({ prompt: "", refs: [{ name: path.split("/").pop() ?? "ref.png", mime, data }] });
  };

  const cardCls = "rounded-xl border border-neutral-200 p-4 dark:border-neutral-800";
  const btnCls = "rounded-md bg-orange-600 px-3 py-2 text-xs text-white hover:bg-orange-700 disabled:opacity-50";
  const inputCls = "rounded-md border border-neutral-300 bg-white px-2 py-1.5 text-xs dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100";

  return (
    <div className="mx-auto max-w-3xl space-y-4 overflow-y-auto p-6">
      <div>
        <h1 className="text-lg font-medium">工具箱</h1>
        <p className="mt-1 text-xs text-neutral-500">
          {status?.available
            ? `ffmpeg 就绪：${status.path}`
            : "未检测到 ffmpeg——请 brew install ffmpeg，或将 ffmpeg 二进制放入应用数据目录 bin/ 下"}
        </p>
        <div className="mt-2 flex items-center gap-2">
          <input
            className={`${inputCls} flex-1`}
            placeholder="自定义 ffmpeg 路径（留空=自动查找）"
            value={customPath}
            onChange={(e) => setCustomPath(e.target.value)}
          />
          <button
            className="rounded-md border border-neutral-300 px-3 py-1.5 text-xs hover:bg-neutral-100 dark:border-neutral-700 dark:hover:bg-neutral-800"
            onClick={async () => {
              try {
                setStatus(await api.toolsSetFfmpegPath(customPath));
                setNotice(customPath ? "已使用自定义路径" : "已恢复自动查找");
              } catch (e) {
                setNotice(String(e));
              }
            }}
          >保存</button>
        </div>
        <p className="mt-1 text-[10px] text-neutral-400">
          查找顺序：自定义路径 → 应用数据目录 bin/ffmpeg（内置/sidecar）→ /opt/homebrew/bin → /usr/local/bin → PATH
          {status?.source === "custom" && " · 当前：自定义"}
          {status?.source === "bundled" && " · 当前：内置 sidecar（ffmpeg 6.0，含 webp）"}
          {status?.source === "manual" && " · 当前：应用数据目录手动放置"}
          {status?.source === "system" && " · 当前：系统安装"}
          {status?.source === "path" && " · 当前：PATH 兜底"}
        </p>
        {status?.available && status.formats && (
          <p className="mt-1 text-xs text-neutral-400">
            可输出格式：
            {Object.entries(status.formats).filter(([, ok]) => ok).map(([k]) => k).join(" / ") || "无"}
            {!status.formats.webp && "（webp 需自行编译带 libwebp 的 ffmpeg）"}
          </p>
        )}
      </div>

      {notice && <div className="rounded-md bg-blue-50 px-3 py-2 text-xs text-blue-700 dark:bg-blue-950 dark:text-blue-400">{notice}</div>}

      <div className={cardCls}>
        <h2 className="text-sm font-medium">格式转换 / 压缩 / 改尺寸</h2>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <Select
            className={inputCls}
            value={format}
            onChange={setFormat}
            options={["png", "jpg", "webp"].map((f) => {
              const unavailable = !!status?.formats && !status.formats[f];
              return { value: f, label: `${f}${unavailable ? "（不可用）" : ""}`, disabled: unavailable };
            })}
          />
          <label className="flex items-center gap-1 text-xs text-neutral-500">
            质量 {quality}
            <input type="range" min={30} max={100} value={quality} onChange={(e) => setQuality(Number(e.target.value))} />
          </label>
          <Select
            className={inputCls}
            value={String(maxSide)}
            onChange={(v) => setMaxSide(Number(v))}
            options={[
              { value: "0", label: "原尺寸" },
              ...[512, 1024, 2048].map((s) => ({ value: String(s), label: `最长边 ${s}` })),
            ]}
          />
          <button className={btnCls} disabled={busy || !status?.available}
            onClick={doConvert}>选择图片并转换</button>
        </div>
      </div>

      <div className={cardCls}>
        <h2 className="text-sm font-medium">合成 GIF</h2>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <label className="flex items-center gap-1 text-xs text-neutral-500">
            帧率 {fps} fps
            <input type="range" min={1} max={20} value={fps} onChange={(e) => setFps(Number(e.target.value))} />
          </label>
          <button className={btnCls} disabled={busy || !status?.available}
            onClick={doGif}>选择多张图片合成</button>
        </div>
      </div>

      <div className={cardCls}>
        <h2 className="text-sm font-medium">视频抽帧（做参考图 / 垫图）</h2>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <button className={btnCls} disabled={busy || !status?.available}
            onClick={doFrames}>选择视频并抽帧</button>
          <span className="text-xs text-neutral-400">每秒 1 帧，最多 16 帧</span>
        </div>
      </div>

      {results.length > 0 && (
        <div className={cardCls}>
          <h2 className="text-sm font-medium">输出</h2>
          <div className="mt-2 grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-2">
            {results.map((p) => (
              <div key={p} className="group relative">
                {resultUrls[p] ? (
                  <img src={resultUrls[p]} className="aspect-square w-full rounded-lg object-cover" alt="" />
                ) : (
                  <div className="aspect-square w-full animate-pulse rounded-lg bg-neutral-100 dark:bg-neutral-800" />
                )}
                <div className="mt-1 flex items-center justify-between text-[10px] text-neutral-400">
                  <span className="truncate">{p.split("/").pop()}</span>
                </div>
                <div className="absolute right-1 top-1 hidden gap-1 group-hover:flex">
                  <button title="作为参考图送到工作台"
                    className="rounded bg-black/60 px-1.5 py-0.5 text-[10px] text-white"
                    onClick={() => asRef(p)}>→ 工作台</button>
                </div>
              </div>
            ))}
          </div>
          <p className="mt-2 text-[10px] text-neutral-400">输出目录：应用数据目录 images/tools/（可在 Finder 中前往）</p>
        </div>
      )}
    </div>
  );
}
