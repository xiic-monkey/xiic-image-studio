import type { PromptTemplate } from "../lib/template.ts";

/**
 * 社媒物料类模板。
 *
 * 这类图的共同点是「先定尺寸，再定信息层级」：封面/缩略图必须在信息流里
 * 缩小也能读，壁纸要避开系统 UI，贴纸要能被裁切。提示词里把安全区、
 * 文字位与裁切边距写死，比堆风格词更有用。
 */
export const SOCIAL_TEMPLATES: PromptTemplate[] = [
  {
    id: "social-xhs",
    title: "小红书封面",
    category: "社媒物料",
    summary: "3:4 竖版封面，大字标题 + 强对比主体",
    ratio: "3:4",
    fields: [
      { key: "title", label: "标题文字", required: true, hint: "6-10 个字，短才有冲击力" },
      {
        key: "subject",
        label: "封面主体",
        required: true,
        multiline: true,
        hint: "人物 / 产品 / 场景，写清最吸引人的那一点",
      },
      { key: "style", label: "视觉风格", default: "干净明亮的实拍感，主题色统一，标题用粗黑体" },
    ],
    prompt: `生成一张小红书风格的竖版封面图。
封面主体：{{subject}}。
标题文字：{{title}}。
视觉风格：{{style}}。
版式：标题放在画面上三分之一，字号大、对比强，用描边或色块衬底保证可读；主体占据中下部，四周留出少量呼吸空间。
构图：3:4 竖版，视觉重心偏上，缩到信息流尺寸也能一眼看清标题和主体。
要求：中文标题拼写准确、字形完整；除标题外不出现其他文字、水印与二维码。`,
  },
  {
    id: "social-thumb",
    title: "视频封面 / 缩略图",
    category: "社媒物料",
    summary: "16:9 高对比缩略图，缩到很小也能读",
    ratio: "16:9",
    fields: [
      { key: "title", label: "标题文字", required: true, hint: "控制在 8 个字以内" },
      {
        key: "subject",
        label: "画面主体",
        required: true,
        multiline: true,
        hint: "人物表情 / 关键物体，要有情绪或冲突感",
      },
      { key: "style", label: "风格", default: "高饱和、高对比，轮廓光强，画面简洁" },
    ],
    prompt: `生成一张视频平台缩略图。
画面主体：{{subject}}。
标题文字：{{title}}。
风格：{{style}}。
版式：主体占画面一侧，表情或动作夸张；标题放在另一侧，粗体大字，带描边或色块底，远看清晰。
构图：16:9 横版，画面元素不超过三个，避免细节堆砌。
要求：中文标题拼写准确、字形完整；不出现其他文字、水印与平台标识。`,
  },
  {
    id: "social-wallpaper",
    title: "手机壁纸",
    category: "社媒物料",
    summary: "9:16 竖屏壁纸，避开状态栏与图标区",
    ratio: "9:16",
    fields: [
      { key: "theme", label: "主题", required: true, multiline: true },
      { key: "palette", label: "色调", default: "低饱和的深蓝与雾灰，少量暖光点缀" },
      { key: "style", label: "风格", default: "极简氛围感，大面积留白，细节耐看" },
    ],
    prompt: `生成一张手机壁纸。
主题：{{theme}}。
色调：{{palette}}。
风格：{{style}}。
构图：9:16 竖屏，视觉主体放在画面中部偏下；顶部约 15% 与底部约 20% 保持简洁，避开状态栏与图标区。
画面：层次干净，主体边缘清晰，缩略图与全屏观看都耐看。
要求：不出现文字、水印、人物面部特写与复杂纹理。`,
  },
  {
    id: "social-sticker",
    title: "贴纸包（九宫格）",
    category: "社媒物料",
    summary: "同一角色的多动作贴纸阵列，带白边可直接裁",
    ratio: "1:1",
    fields: [
      { key: "character", label: "角色形象", required: true, multiline: true },
      {
        key: "actions",
        label: "动作 / 表情清单",
        default: "打招呼、比心、大笑、生气、哭泣、睡觉、吃东西、点赞、疑惑",
      },
      { key: "style", label: "画风", default: "Q 版卡通，粗描边，平涂上色，表情夸张" },
    ],
    prompt: `生成一张贴纸包（sticker sheet）：同一个角色的九种动作，排成 3x3 九宫格。
角色：{{character}}。
动作与表情：{{actions}}。
画风：{{style}}。
一致性：九格是同一个角色、同一套配色与服装，头身比一致，只有动作和表情不同。
贴纸规范：每个图案外围有一圈均匀的白色描边，图案之间留出清晰间距，背景为纯色。
要求：画面中不出现文字、水印与对话框。`,
  },
];
