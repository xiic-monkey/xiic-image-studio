/**
 * 全局统一的控件规格。
 *
 * 规则只有两条：
 * 1) 单行控件（输入框 / 下拉 / 按钮）高度一律 30px，不随字号变化——
 *    之前各页各写一份 inputCls，用 py-1.5 / py-2 撑高度，结果模型库 36px、
 *    工具箱 28px，跨页面并排看非常不协调。
 * 2) 字号只分两档：常规 text-sm，密集区 text-xs（工作台参数四列、工具箱表单、
 *    配方面板）。密集区是空间所迫，不是因为「可以更高」。
 *
 * 页面不要再自己拼 inputCls / buttonCls，要改就改这里，避免一处改、另一处不动。
 */

/** 单行控件统一高度 */
export const CTRL_H = "h-[30px]";

const fieldBase =
  "rounded-md border border-neutral-300 bg-white px-2.5 focus:border-orange-500 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100";

/** 常规输入框 / 下拉触发器 */
export const inputCls = `w-full ${CTRL_H} ${fieldBase} text-sm`;

/** 紧凑输入框 / 下拉：同样 30px 高，字号小一号 */
export const inputCompactCls = `w-full ${CTRL_H} ${fieldBase} text-xs`;

/** 多行输入框：高度按内容需要，只统一边框与内边距 */
export const textareaCls =
  "w-full rounded-md border border-neutral-300 bg-white px-2.5 py-2 text-sm focus:border-orange-500 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100";

const btnBase = `inline-flex items-center justify-center ${CTRL_H} rounded-md px-3 transition-colors disabled:opacity-40`;

/** 次要按钮（描边） */
export const btnCls =
  `${btnBase} border border-neutral-300 text-sm text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800`;

/** 主按钮（橙色实心） */
export const btnPrimaryCls = `${btnBase} bg-orange-600 text-sm text-white hover:bg-orange-700 disabled:opacity-50`;

/** 幽灵按钮（无边框） */
export const btnGhostCls = `${btnBase} text-sm text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800`;

/** 紧凑按钮：与输入框同高，字号小一号 */
export const btnCompactCls =
  `${btnBase} border border-neutral-300 text-xs text-neutral-600 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-300 dark:hover:bg-neutral-800`;

/**
 * 开关型按钮（选中态橙色实心）。
 * 单独写一个函数而不是拼 btnCls：两套颜色工具类叠在一起时，
 * 胜出的是 Tailwind 产物里的 CSS 顺序，不是 class 字符串的先后。
 */
export const btnToggleCls = (active: boolean) =>
  `inline-flex ${CTRL_H} items-center justify-center rounded-md border px-3 text-sm transition-colors ${
    active
      ? "border-orange-600 bg-orange-600 text-white hover:bg-orange-700"
      : "border-neutral-300 text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800"
  }`;

/** 列表行内操作按钮：比表单控件更轻，不抢行的视觉重心 */
export const btnRowCls =
  "inline-flex h-[26px] items-center rounded-md px-2 text-xs text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800";

/** 列表行内的危险操作 */
export const btnRowDangerCls =
  "inline-flex h-[26px] items-center rounded-md px-2 text-xs text-red-500 hover:bg-red-50 dark:hover:bg-red-950";

/**
 * 筛选标签：比常规按钮小一号（26px / text-xs），只用于密集的筛选条。
 * 分类标签横向排一长条，用 30px 会撑出又长又高的滚动条，视觉上压过内容。
 */
export const btnChipCls = (active: boolean) =>
  `inline-flex h-[26px] shrink-0 items-center justify-center rounded-md border px-2.5 text-xs transition-colors ${
    active
      ? "border-orange-600 bg-orange-600 text-white hover:bg-orange-700"
      : "border-neutral-300 text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800"
  }`;

/** 表单标签 */
export const labelCls = "mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400";
