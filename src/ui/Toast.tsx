import { useEffect } from "react";
import { useToast } from "../stores/toast";

/** 右上角悬浮提示：全局只挂这一个，显示几秒后自动消失。 */
export default function Toast() {
  const message = useToast((s) => s.message);
  const seq = useToast((s) => s.seq);
  const hide = useToast((s) => s.hide);

  useEffect(() => {
    if (!message) return;
    const timer = setTimeout(hide, 3000);
    return () => clearTimeout(timer);
  }, [message, seq, hide]);

  if (!message) return null;
  return (
    <div
      role="status"
      className="fixed right-5 top-5 z-[100] max-w-[420px] rounded-md border border-neutral-200 bg-white px-4 py-2.5 text-sm leading-snug text-neutral-700 shadow-lg dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-200"
    >
      {message}
    </div>
  );
}
