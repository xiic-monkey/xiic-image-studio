import { useState } from "react";
import GalleryView from "./views/GalleryView";
import PromptsView from "./views/PromptsView";
import ToolsView from "./views/ToolsView";
import ProvidersView from "./views/ProvidersView";
import WorkbenchView from "./views/WorkbenchView";
import TrafficLights from "./components/TrafficLights";

type Tab = "workbench" | "gallery" | "prompts" | "tools" | "providers";

// 是否显示前端自绘红绿灯。
// 当前关闭：窗口已恢复系统原生标题栏 + 系统红绿灯（见 tauri.conf.json 的 decorations:true）。
// 想切回自绘版，把这里改成 true 即可，组件代码与样式无需改动。
const SHOW_TRAFFIC_LIGHTS = false;

const NAV: { key: Tab; label: string; icon: string }[] = [
  { key: "workbench", label: "工作台", icon: "M12 5v14M5 12h14" },
  { key: "gallery", label: "画廊", icon: "M4 5h16v14H4zM4 15l5-5 4 4 3-3 4 4" },
  { key: "prompts", label: "提示词", icon: "M4 6h16M4 12h10M4 18h7" },
  { key: "tools", label: "工具箱", icon: "M14 7l3 3-8.5 8.5H5.5V15.5zM12 9l3 3" },
  { key: "providers", label: "供应商", icon: "M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12v9M12 12L4 7.5" },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("workbench");

  return (
    <div className="relative flex h-full overflow-hidden bg-white text-neutral-900 dark:bg-neutral-950 dark:text-neutral-100">
      <aside className="flex w-14 shrink-0 flex-col items-center gap-1 border-r border-neutral-200 pt-2 pb-3 dark:border-neutral-800">
        {SHOW_TRAFFIC_LIGHTS && (
          <TrafficLights size={11} gap={8} className="mb-4" />
        )}
        <div className="mb-3 flex h-8 w-8 items-center justify-center rounded-lg bg-orange-600 text-sm font-medium text-white">
          IS
        </div>
        {NAV.map((n) => (
          <button
            key={n.key}
            title={n.label}
            onClick={() => setTab(n.key)}
            className={`flex h-10 w-10 items-center justify-center rounded-lg transition-colors ${
              tab === n.key
                ? "bg-orange-50 text-orange-600 dark:bg-orange-950"
                : "text-neutral-400 hover:bg-neutral-100 hover:text-neutral-600 dark:hover:bg-neutral-800"
            }`}
          >
            <svg viewBox="0 0 24 24" className="h-5 w-5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
              <path d={n.icon} />
            </svg>
          </button>
        ))}
        <div className="flex-1" />
        <div className="pb-1 text-[10px] text-neutral-300 dark:text-neutral-700">v0.1</div>
      </aside>
      <main className="min-w-0 flex-1 overflow-y-auto">
        {tab === "workbench" && <WorkbenchView />}
        {tab === "gallery" && <GalleryView />}
        {tab === "prompts" && <PromptsView />}
        {tab === "tools" && <ToolsView />}
        {tab === "providers" && <ProvidersView />}
      </main>
    </div>
  );
}
