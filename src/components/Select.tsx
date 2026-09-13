import { useEffect, useLayoutEffect, useRef, useState } from "react";

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

interface SelectProps {
  value: string;
  onChange: (v: string) => void;
  options: SelectOption[];
  className?: string;       // 透传给 trigger（沿用现有 inputCls）
  placeholder?: string;
  ariaLabel?: string;
}

/**
 * 自定义下拉，替代原生 <select>。
 * 原生 select 展开面板在 macOS / WebKit 由系统渲染成 NSMenu 风格，CSS 无法覆盖。
 * 这里 trigger 仍走 Tailwind 输入框样式（className 继承 inputCls 即可），
 * 菜单面板自绘：圆角白底 + 自定义滚动条 + 选中 ✓ + hover 高亮 + 键盘可访问。
 */
export default function Select({
  value, onChange, options, className = "", placeholder, ariaLabel,
}: SelectProps) {
  const [open, setOpen] = useState(false);
  const [highlight, setHighlight] = useState(-1);
  const [flip, setFlip] = useState(false); // 下方空间不够时翻到 trigger 上方
  const wrapRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  const selected = options.find((o) => o.value === value);
  const display = selected?.label ?? placeholder ?? "";

  // 打开时：把 highlight 落到当前选中项
  useLayoutEffect(() => {
    if (!open) return;
    const idx = options.findIndex((o) => o.value === value);
    setHighlight(idx >= 0 ? idx : 0);
  }, [open, value, options]);

  // 打开时：根据 trigger 下方可用空间决定展开方向
  useLayoutEffect(() => {
    if (!open) return;
    const t = triggerRef.current;
    if (!t) return;
    const r = t.getBoundingClientRect();
    const below = window.innerHeight - r.bottom;
    const need = Math.min(240, options.length * 32 + 16);
    setFlip(below < need && r.top > below);
    const onWin = () => setOpen(false);
    window.addEventListener("resize", onWin);
    window.addEventListener("scroll", onWin, true);
    return () => {
      window.removeEventListener("resize", onWin);
      window.removeEventListener("scroll", onWin, true);
    };
  }, [open, options.length]);

  // 点击外部 / Esc 关闭
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!wrapRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const move = (dir: 1 | -1) => {
    if (options.length === 0) return;
    setHighlight((h) => {
      const start = h < 0 ? 0 : h;
      for (let i = 1; i <= options.length; i++) {
        const ni = (start + dir * i + options.length * 1024) % options.length;
        if (!options[ni].disabled) return ni;
      }
      return h;
    });
  };

  const commit = (i: number) => {
    const o = options[i];
    if (!o || o.disabled) return;
    onChange(o.value);
    setOpen(false);
    triggerRef.current?.focus();
  };

  const onTriggerKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      setOpen(true);
    }
  };

  const onMenuKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") { e.preventDefault(); move(1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); move(-1); }
    else if (e.key === "Enter" || e.key === " ") { e.preventDefault(); commit(highlight); }
    else if (e.key === "Tab") { setOpen(false); }
    else if (e.key === "Home") { e.preventDefault(); setHighlight(0); }
    else if (e.key === "End") { e.preventDefault(); setHighlight(options.length - 1); }
  };

  return (
    <div ref={wrapRef} className="relative">
      <button
        ref={triggerRef}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={ariaLabel}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={onTriggerKey}
        className={`flex w-full items-center justify-between gap-2 text-left ${className}`}
      >
        <span className={`min-w-0 flex-1 truncate ${selected ? "" : "text-neutral-400"}`}>
          {display}
        </span>
        <svg
          viewBox="0 0 20 20"
          aria-hidden="true"
          className={`h-4 w-4 shrink-0 text-neutral-400 transition-transform duration-150 ${open ? "rotate-180" : ""}`}
          fill="none" stroke="currentColor" strokeWidth="1.6"
        >
          <path d="M5 8l5 5 5-5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {open && (
        <div
          role="listbox"
          tabIndex={-1}
          ref={(el) => el?.focus()}
          onKeyDown={onMenuKey}
          className={`select-menu absolute left-0 right-0 z-50 max-h-60 overflow-auto rounded-md border border-neutral-200 bg-white py-1 shadow-lg outline-none dark:border-neutral-700 dark:bg-neutral-900 ${
            flip ? "bottom-full mb-1" : "top-full mt-1"
          }`}
        >
          {options.length === 0 && (
            <div className="px-3 py-2 text-xs text-neutral-400">（无选项）</div>
          )}
          {options.map((o, i) => {
            const isSel = o.value === value;
            const isHl = i === highlight;
            return (
              <div
                key={o.value}
                role="option"
                aria-selected={isSel}
                aria-disabled={o.disabled}
                onMouseEnter={() => setHighlight(i)}
                onClick={() => commit(i)}
                className={`flex cursor-pointer items-center gap-2 px-3 py-1.5 text-sm ${
                  o.disabled
                    ? "cursor-not-allowed text-neutral-300 dark:text-neutral-600"
                    : isHl
                      ? "bg-orange-50 text-orange-600 dark:bg-orange-950 dark:text-orange-400"
                      : "text-neutral-700 dark:text-neutral-200"
                } ${isSel && !o.disabled ? "font-medium" : ""}`}
              >
                <span
                  className={`inline-flex h-3 w-3 shrink-0 items-center justify-center text-orange-600 dark:text-orange-400 ${
                    isSel ? "opacity-100" : "opacity-0"
                  }`}
                  aria-hidden="true"
                >✓</span>
                <span className="min-w-0 flex-1 truncate">{o.label}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
