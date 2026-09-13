import { useEffect, useRef, useState } from "react";
import type { RefImage } from "../types";

interface Props {
  image: RefImage; // 底图
  onDone: (mask: RefImage | null) => void;
  onClose: () => void;
}

/** 局部重绘蒙版编辑器：在底图上涂抹，涂抹区域导出为透明（OpenAI edits 规范） */
export default function MaskEditor({ image, onDone, onClose }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [brush, setBrush] = useState(40);
  const [hasPaint, setHasPaint] = useState(false);
  const painting = useRef(false);
  const last = useRef<{ x: number; y: number } | null>(null);

  // 用原始尺寸初始化画布
  useEffect(() => {
    const canvas = canvasRef.current!;
    const img = new Image();
    img.onload = () => {
      canvas.width = img.naturalWidth;
      canvas.height = img.naturalHeight;
      const ctx = canvas.getContext("2d")!;
      ctx.drawImage(img, 0, 0);
    };
    img.src = `data:${image.mime};base64,${image.data}`;
  }, [image]);

  const pos = (e: React.PointerEvent) => {
    const canvas = canvasRef.current!;
    const rect = canvas.getBoundingClientRect();
    return {
      x: ((e.clientX - rect.left) / rect.width) * canvas.width,
      y: ((e.clientY - rect.top) / rect.height) * canvas.height,
    };
  };

  const paint = (e: React.PointerEvent) => {
    if (!painting.current) return;
    const canvas = canvasRef.current!;
    const ctx = canvas.getContext("2d")!;
    const p = pos(e);
    ctx.globalCompositeOperation = "destination-out";
    ctx.lineWidth = brush * (canvas.width / 480);
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.beginPath();
    if (last.current) {
      ctx.moveTo(last.current.x, last.current.y);
    } else {
      ctx.moveTo(p.x, p.y);
    }
    ctx.lineTo(p.x, p.y);
    ctx.stroke();
    last.current = p;
    setHasPaint(true);
  };

  const clear = () => {
    const canvas = canvasRef.current!;
    const img = new Image();
    img.onload = () => {
      const ctx = canvas.getContext("2d")!;
      ctx.globalCompositeOperation = "source-over";
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(img, 0, 0);
      setHasPaint(false);
    };
    img.src = `data:${image.mime};base64,${image.data}`;
  };

  const done = () => {
    if (!hasPaint) return onDone(null);
    const dataUrl = canvasRef.current!.toDataURL("image/png");
    const [, data] = dataUrl.split(",");
    onDone({ name: "mask.png", mime: "image/png", data });
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6" onClick={onClose}>
      <div className="max-h-full w-[560px] overflow-y-auto rounded-xl bg-white p-5 dark:bg-neutral-900" onClick={(e) => e.stopPropagation()}>
        <h2 className="mb-2 text-base font-medium">涂抹要重绘的区域</h2>
        <p className="mb-3 text-xs text-neutral-500">在图上按住鼠标涂抹（涂抹处会被 AI 重新生成）。仅 OpenAI Images 协议支持精确蒙版，其他协议将作为普通图生图处理。</p>
        <div className="overflow-hidden rounded-lg border border-neutral-200 dark:border-neutral-700">
          <canvas
            ref={canvasRef}
            className="block max-h-[420px] w-full cursor-crosshair object-contain touch-none"
            onPointerDown={(e) => { painting.current = true; last.current = null; paint(e); }}
            onPointerMove={paint}
            onPointerUp={() => { painting.current = false; last.current = null; }}
            onPointerLeave={() => { painting.current = false; last.current = null; }}
          />
        </div>
        <div className="mt-3 flex items-center gap-3">
          <span className="text-xs text-neutral-500">笔刷</span>
          <input type="range" min={10} max={120} value={brush} onChange={(e) => setBrush(Number(e.target.value))} className="flex-1" />
          <button className="text-xs text-neutral-500 hover:underline" onClick={clear}>清除涂抹</button>
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <button className="rounded-md px-4 py-2 text-sm text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800" onClick={() => { onDone(null); }}>取消</button>
          <button className="rounded-md bg-orange-600 px-4 py-2 text-sm text-white hover:bg-orange-700" onClick={done}>完成</button>
        </div>
      </div>
    </div>
  );
}
