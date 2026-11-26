import { useEffect, type ReactNode } from "react";
import { useToast, type ToastKind } from "../stores/toast";

/**
 * 四档语义色沿用项目里已有的 green / amber / red（供应商测活结果、任务状态标签同款），
 * info 用蓝色系表示纯陈述。每档配齐：外框、左侧色条、图标圆底、图标。
 */
type Tone = {
  box: string;
  bar: string;
  badge: string;
  icon: ReactNode;
};

const TONE: Record<ToastKind, Tone> = {
  info: {
    box: "border-blue-200 bg-blue-50 text-blue-800 dark:border-blue-900 dark:bg-blue-950 dark:text-blue-200",
    bar: "bg-blue-500",
    badge: "bg-blue-100 text-blue-600 dark:bg-blue-900 dark:text-blue-300",
    icon: (
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="M12 11v5" />
        <path d="M12 8h.01" />
      </>
    ),
  },
  success: {
    box: "border-green-200 bg-green-50 text-green-800 dark:border-green-900 dark:bg-green-950 dark:text-green-200",
    bar: "bg-green-500",
    badge: "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300",
    icon: (
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="m8 12 2.5 2.5L16 9" />
      </>
    ),
  },
  warning: {
    box: "border-amber-200 bg-amber-50 text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200",
    bar: "bg-amber-500",
    badge: "bg-amber-100 text-amber-700 dark:bg-amber-900 dark:text-amber-300",
    icon: (
      <>
        <path d="M10.3 3.6 2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.6a2 2 0 0 0-3.4 0Z" />
        <path d="M12 9v4" />
        <path d="M12 17h.01" />
      </>
    ),
  },
  error: {
    box: "border-red-200 bg-red-50 text-red-800 dark:border-red-900 dark:bg-red-950 dark:text-red-200",
    bar: "bg-red-500",
    badge: "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300",
    icon: (
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="m15 9-6 6" />
        <path d="m9 9 6 6" />
      </>
    ),
  },
};

/** 右上角悬浮提示：全局只挂这一个，显示几秒后自动消失。 */
export default function Toast() {
  const message = useToast((s) => s.message);
  const kind = useToast((s) => s.kind);
  const seq = useToast((s) => s.seq);
  const hide = useToast((s) => s.hide);

  useEffect(() => {
    if (!message) return;
    const timer = setTimeout(hide, 3000);
    return () => clearTimeout(timer);
  }, [message, seq, hide]);

  if (!message) return null;
  const tone = TONE[kind];
  return (
    <div
      key={seq}
      role={kind === "error" ? "alert" : "status"}
      className={`toast-enter fixed right-5 top-5 z-[100] flex w-[min(420px,calc(100vw-40px))] items-start gap-3 overflow-hidden rounded-lg border px-4 py-3 pl-5 shadow-xl ${tone.box}`}
    >
      <span className={`absolute left-0 top-0 h-full w-1 ${tone.bar}`} aria-hidden="true" />
      <span
        className={`mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full ${tone.badge}`}
        aria-hidden="true"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="h-3.5 w-3.5"
        >
          {tone.icon}
        </svg>
      </span>
      <span className="min-w-0 flex-1 text-sm font-medium leading-snug">{message}</span>
      <button
        type="button"
        onClick={hide}
        aria-label="关闭提示"
        className="-mr-1 -mt-0.5 rounded p-1 opacity-50 transition hover:bg-black/5 hover:opacity-100 dark:hover:bg-white/10"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          className="h-3.5 w-3.5"
        >
          <path d="M18 6 6 18" />
          <path d="m6 6 12 12" />
        </svg>
      </button>
    </div>
  );
}
