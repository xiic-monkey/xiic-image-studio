import type { PromptTemplate } from "../lib/template.ts";

/**
 * 设计 / 品牌类模板。
 *
 * 这类模板对「文字排版」敏感，建议配 Ideogram（排版最强）或 Gemini；
 * 提示词按 Google 官方建议写成「为……设计……」并交代用途与上下文，
 * 文字尽量短，字越多错字越多。
 */
export const DESIGN_TEMPLATES: PromptTemplate[] = [
  {
    id: "design-logo",
    title: "品牌 Logo / 图标",
    category: "设计品牌",
    summary: "交代品牌用途与风格诉求的 logo 方案",
    ratio: "1:1",
    fields: [
      { key: "brand", label: "品牌 / 用途", required: true, multiline: true, hint: "品牌名 + 行业 + 目标人群" },
      { key: "style", label: "风格诉求", default: "极简几何，笔画粗壮，缩到很小也能认出来" },
      { key: "symbol", label: "核心符号", default: "一个能代表这门生意的抽象图形" },
    ],
    prompt: `为{{brand}}设计一个品牌标志。
风格：{{style}}。
核心符号：{{symbol}}。
版式：单个标志居中展示，纯白背景，周围留出充足留白。
要求：图形简洁，轮廓清晰，缩放后依然可辨识；画面中只出现图形本身，不出现任何文字、说明、样机与阴影。`,
  },
  {
    id: "design-poster",
    title: "带字海报",
    category: "设计品牌",
    summary: "短标题 + 明确版式，海报类模板的文字别写长",
    ratio: "2:3",
    fields: [
      { key: "title", label: "标题文字", required: true, hint: "控制在 8 个字以内，字越多越容易错字" },
      { key: "theme", label: "主题与信息", required: true, multiline: true },
      { key: "style", label: "视觉风格", default: "现代平面设计，强烈的主色对比，几何构成，干净的字体排版" },
    ],
    prompt: `为{{theme}}设计一张海报。
标题文字：{{title}}。
视觉风格：{{style}}。
版式：标题位于视觉主位，字体粗壮清晰，与背景对比强烈；其余信息按层级排布，留白干净。
画面：完整的成品海报效果，文字拼写准确、字形完整、可读性强。`,
  },
  {
    id: "design-ui",
    title: "UI 界面稿",
    category: "设计品牌",
    summary: "可交付给前端的界面结构图",
    ratio: "16:9",
    fields: [
      { key: "product", label: "产品 / 页面类型", required: true, multiline: true, hint: "是什么产品的什么页面" },
      { key: "modules", label: "页面模块", required: true, multiline: true, hint: "要包含哪些区块，按顺序列" },
      { key: "style", label: "视觉风格", default: "现代简洁，浅色背景，圆角卡片，统一的图标语言，清晰的信息层级" },
    ],
    prompt: `设计一张{{product}}的界面稿。
模块：{{modules}}。
视觉风格：{{style}}。
版式：按信息层级自上而下排布模块，对齐与间距统一，组件语言一致。
要求：像真实可用的产品界面截图，细节完整、比例真实；画面中不出现可读文字与乱码字符。`,
  },
  {
    id: "design-packaging",
    title: "产品包装设计",
    category: "设计品牌",
    summary: "盒型 / 瓶型的包装效果图，留出品牌位",
    ratio: "1:1",
    fields: [
      { key: "product", label: "产品与品类", required: true, multiline: true },
      { key: "style", label: "包装风格", default: "极简高级，哑光材质，局部烫金，留白充足" },
      { key: "structure", label: "包装结构", default: "立式纸盒，正面与侧面同时可见" },
    ],
    prompt: `为{{product}}设计产品包装。
包装结构：{{structure}}。
包装风格：{{style}}。
版式：正面以品牌图形或产品名为主视觉，信息按层级排布，留白干净；侧面保留必要的信息区。
材质：哑光纸、局部烫金或压凹工艺，边缘挺括，能看到纸张厚度。
展示：产品包装以 45 度角立在纯色背景上，柔和棚拍光，干净的接触阴影。
要求：画面中不出现可读文字与乱码字符，用图形与色块表现品牌信息。`,
  },
  {
    id: "design-merch",
    title: "T 恤 / 周边印花",
    category: "设计品牌",
    summary: "适合丝网印刷的印花图案，颜色数受限",
    ratio: "1:1",
    fields: [
      { key: "theme", label: "图案主题", required: true, multiline: true },
      { key: "style", label: "风格", default: "复古美式插画，粗线条，做旧质感" },
      { key: "colors", label: "印色数", default: "四色以内：米白、墨黑、砖红、芥末黄" },
    ],
    prompt: `为{{theme}}设计一个 T 恤印花图案。
风格：{{style}}。
印色：{{colors}}，每个颜色都是纯色块，不做渐变与半透明叠加。
构图：图案集中在画面中央，适合放在胸前；外轮廓完整闭合，边缘干净。
工艺：模拟丝网印刷的效果，颜色有轻微的套印错位与做旧颗粒，图案以外的部分是纯色底。
要求：不出现真实品牌、签名与水印；文字只用短词或直接不用文字。`,
  },
  {
    id: "design-album",
    title: "专辑封面",
    category: "设计品牌",
    summary: "方形封面视觉，留出标题与厂牌位",
    ratio: "1:1",
    fields: [
      {
        key: "concept",
        label: "音乐与概念",
        required: true,
        multiline: true,
        hint: "曲风、情绪、想表达的画面",
      },
      { key: "style", label: "视觉风格", default: "胶片摄影质感，颗粒明显，构图大胆，色彩克制" },
    ],
    prompt: `为{{concept}}设计一张专辑封面。
视觉风格：{{style}}。
版式：正方形封面，主体视觉占据画面，上方或下方留出一块干净区域用于放置专辑名。
质感：像真实印刷的唱片封面，有纸张纹理、轻微套印偏差与边缘磨损。
构图：一个强记忆点的核心画面，避免元素堆砌。
要求：画面中不出现可读文字、条形码与厂牌标识。`,
  },
];
