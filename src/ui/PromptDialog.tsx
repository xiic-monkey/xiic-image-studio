import { useEffect, useRef, useState } from "react";
import { btnGhostCls, btnPrimaryCls, inputCls } from "./controls";

export interface PromptDialogProps {
  open: boolean;
  title: string;
  placeholder?: string;
  defaultValue?: string;
  confirmLabel?: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}

/**
 * 通用单行输入弹窗，替代浏览器原生 prompt（暗色主题下是白框，且无法跟随应用风格）。
 * 由调用方用 open 控制显隐；确认时回传 trim 后的值，空值不提交。
 */
export default function PromptDialog({
  open,
  title,
  placeholder,
  defaultValue = "",
  confirmLabel = "确定",
  onSubmit,
  onCancel,
}: PromptDialogProps) {
  const [value, setValue] = useState(defaultValue);
  const inputRef = useRef<HTMLInputElement>(null);

  // 每次打开都回到默认值并聚焦——否则上一次的输入会残留到下一次
  useEffect(() => {
    if (!open) return;
    setValue(defaultValue);
    const t = setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 0);
    return () => clearTimeout(t);
  }, [open, defaultValue]);

  useEffect(() => {
    if (!open) return;
    // 捕获阶段拦截 Escape：弹窗可能盖在预览层上，不能让下层快捷键跟着触发
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onCancel();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [open, onCancel]);

  if (!open) return null;

  const submit = () => {
    const v = value.trim();
    if (v) onSubmit(v);
  };

  return (
    <div
      className="fixed inset-0 z-[80] flex items-center justify-center bg-black/40 p-6"
      onClick={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div className="w-[420px] rounded-xl bg-white p-5 shadow-xl dark:bg-neutral-900">
        <h2 className="mb-3 text-sm font-medium">{title}</h2>
        <input
          ref={inputRef}
          className={inputCls}
          placeholder={placeholder}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            // 弹窗可能在预览层之上：别让 Escape / 方向键同时触发下层快捷键
            e.stopPropagation();
            if (e.key === "Enter") submit();
          }}
        />
        <div className="mt-4 flex justify-end gap-2">
          <button className={btnGhostCls} onClick={onCancel}>取消</button>
          <button className={btnPrimaryCls} onClick={submit} disabled={!value.trim()}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
