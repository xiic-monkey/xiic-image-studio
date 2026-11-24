import type { PromptTemplate } from "../lib/template.ts";
import { templateCategories } from "../lib/template.ts";
import { CHARACTER_TEMPLATES } from "./character.ts";
import { COMMERCE_TEMPLATES } from "./commerce.ts";
import { DESIGN_TEMPLATES } from "./design.ts";
import { ILLUSTRATION_TEMPLATES } from "./illustration.ts";
import { PORTRAIT_TEMPLATES } from "./portrait.ts";
import { SCENE_TEMPLATES } from "./scene.ts";
import { SOCIAL_TEMPLATES } from "./social.ts";

export type { PromptTemplate, TemplateField, TemplateStep } from "../lib/template.ts";

/**
 * 内置提示词模板库。
 *
 * 分类与条目按业内工作流排：
 * - 角色设计：定案图 → 三视图 / 短剧设定图 → 表情差分 → 服装 → 配色 → 道具 → 场景延展
 * - 人物摄影：特写 / 全身 / 杂志 / 街头 / 证件照 / 宠物
 * - 电商产品：主图 / 场景 / 模特 / 细节 / 美食
 * - 插画风格：二次元 / 扁平 / 水彩 / 概念 + 热门风格（动画电影 / 3D / 粘土 / 潮玩 / 像素 / 等距 / 素描 / 赛博朋克）
 * - 场景分镜：场景 / 分镜 / 主视觉 / 室内 / 建筑
 * - 设计品牌：Logo / 海报 / UI / 包装 / 印花 / 专辑封面
 * - 社媒物料：小红书封面 / 视频缩略图 / 壁纸 / 贴纸包
 */
export const TEMPLATE_LIBRARY: PromptTemplate[] = [
  ...CHARACTER_TEMPLATES,
  ...PORTRAIT_TEMPLATES,
  ...COMMERCE_TEMPLATES,
  ...ILLUSTRATION_TEMPLATES,
  ...SCENE_TEMPLATES,
  ...DESIGN_TEMPLATES,
  ...SOCIAL_TEMPLATES,
];

/** 模板分类（按首次出现顺序）。 */
export const TEMPLATE_CATEGORIES = templateCategories(TEMPLATE_LIBRARY);

/** 按 id 找模板。 */
export function findTemplate(id: string): PromptTemplate | undefined {
  return TEMPLATE_LIBRARY.find((t) => t.id === id);
}
