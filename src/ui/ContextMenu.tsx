export interface ContextMenuItem {
  label: string;
  onClick: () => void;
  danger?: boolean;
}

/**
 * 画廊与任务队列共用的右键菜单：风格、文案位置、边界收敛只在这里定义一处。
 * 关闭由调用方负责（全局 click / resize / Escape / 滚动）。
 */
export default function ContextMenu({ x, y, items }: { x: number; y: number; items: ContextMenuItem[] }) {
  return (
    <div
      className="fixed z-[70] w-40 rounded-md border border-neutral-200 bg-white py-1 text-sm shadow-lg dark:border-neutral-700 dark:bg-neutral-900"
      style={{
        left: Math.min(x, window.innerWidth - 168),
        top: Math.min(y, window.innerHeight - items.length * 28 - 8),
      }}
      onClick={(e) => e.stopPropagation()}
      onMouseDown={(e) => e.stopPropagation()}
    >
      {items.map((item) => (
        <button
          key={item.label}
          className={`block w-full px-3 py-1.5 text-left hover:bg-neutral-100 dark:hover:bg-neutral-800 ${
            item.danger ? "text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-950" : ""
          }`}
          onClick={item.onClick}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
