import { getCurrentWindow } from "@tauri-apps/api/window";

type LightAction = "close" | "minimize" | "fullscreen";

interface TrafficLightsProps {
  /** 圆点直径（px）。默认 11。 */
  size?: number;
  /** 圆点之间的间距（px）。默认 8。 */
  gap?: number;
  className?: string;
}

/** macOS 红黄绿基底色（Big Sur+ 风格） */
const DOT_COLORS: Record<LightAction, string> = {
  close: "#ff5f57",
  minimize: "#febc2e",
  fullscreen: "#28c840",
};

/**
 * 三个符号，统一 viewBox 0 0 12 12：
 *   - close: ✕ （两条交叉线）
 *   - minimize: — （一条横线）
 *   - fullscreen: ⤢ （两个对角实心三角，左上 + 右下）
 */
const GLYPHS: Record<LightAction, { fill: boolean; d: string }> = {
  close: { fill: false, d: "M3.5 3.5l5 5M8.5 3.5l-5 5" },
  minimize: { fill: false, d: "M3 6h6" },
  fullscreen: {
    fill: true,
    d: "M1 1 L5 1 L1 5 Z M11 11 L7 11 L11 7 Z",
  },
};

/**
 * 自绘 macOS 红绿灯。窗口是 decorations:false + transparent:true（borderless 透明），
 * 没有系统红黄绿，这里完全接管 close / minimize / fullscreen 三个交互。
 *
 * 绿点行为：macOS 原生绿点是 toggle Mission Control 全屏（不是普通 maximize），
 * 所以这里读 isFullscreen() 切换 setFullscreen()，与原生一致。
 *
 * 设计原则（与本项目参考样式一致）：朴素、永显符号、不做失焦变灰、
 * 不做最大化切图标、不做 hover fade。
 */
export default function TrafficLights({
  size = 11,
  gap = 8,
  className = "",
}: TrafficLightsProps) {
  const win = getCurrentWindow();

  const handle = (action: LightAction) => async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      if (action === "close") {
        await win.close();
      } else if (action === "minimize") {
        await win.minimize();
      } else {
        const fs = await win.isFullscreen();
        await win.setFullscreen(!fs);
      }
    } catch (err) {
      console.error("[traffic-light]", action, err);
    }
  };

  const actions: LightAction[] = ["close", "minimize", "fullscreen"];

  return (
    <div
      className={`flex items-center ${className}`}
      style={{ gap: `${gap}px` }}
      onDoubleClick={(e) => e.stopPropagation()}
    >
      {actions.map((action) => {
        const g = GLYPHS[action];
        return (
          <button
            key={action}
            aria-label={action}
            title={action}
            onClick={handle(action)}
            onMouseDown={(e) => e.stopPropagation()}
            className="flex shrink-0 items-center justify-center rounded-full transition-[filter] duration-100 hover:brightness-110 active:brightness-95"
            style={{
              width: size,
              height: size,
              backgroundColor: DOT_COLORS[action],
            }}
          >
            <svg
              viewBox="0 0 12 12"
              className="pointer-events-none h-[62%] w-[62%]"
              fill={g.fill ? "rgba(0,0,0,0.55)" : "none"}
              stroke={g.fill ? "none" : "rgba(0,0,0,0.55)"}
              strokeWidth={g.fill ? 0 : 1.5}
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d={g.d} />
            </svg>
          </button>
        );
      })}
    </div>
  );
}
