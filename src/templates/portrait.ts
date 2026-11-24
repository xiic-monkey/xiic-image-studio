import type { PromptTemplate } from "../lib/template.ts";

/**
 * 人物摄影类模板。
 *
 * 依据 Google 官方生图最佳实践：主体 + 构图 + 动作 + 环境 + 风格 + 编辑指令，
 * 并用摄影/电影术语控制镜头（焦段、机位、光线）；同时强调「描述想要的，
 * 而不是不想要的」。
 */
export const PORTRAIT_TEMPLATES: PromptTemplate[] = [
  {
    id: "photo-portrait",
    title: "写实人像特写",
    category: "人物摄影",
    summary: "自然光写实头像，肤质与眼神可信",
    ratio: "4:5",
    fields: [
      { key: "subject", label: "人物", required: true, multiline: true, hint: "性别、年龄段、发型发色、体型、神态气质" },
      { key: "expression", label: "表情", default: "自然放松，嘴角微微上扬，眼神直视镜头" },
      { key: "outfit", label: "穿着", default: "米色粗针织高领毛衣" },
      { key: "lighting", label: "光线", default: "窗边柔和自然光，从 45 度侧前方进入，柔和阴影" },
      { key: "lens", label: "镜头", default: "85mm 定焦，f/2.0，浅景深" },
      { key: "background", label: "背景", default: "干净的室内墙面，浅景深虚化" },
    ],
    prompt: `生成一张写实人像照片。
人物：{{subject}}。
表情：{{expression}}。
穿着：{{outfit}}。
光线：{{lighting}}。
镜头：{{lens}}。
构图：人物半身入画，眼睛位于画面上三分之一处，头顶留出适当空间。
背景：{{background}}。
质感：真实皮肤纹理与毛孔，干净自然的色彩，照片级细节，不磨皮过度。`,
  },
  {
    id: "photo-fullbody",
    title: "全身人像（模特 / 社媒）",
    category: "人物摄影",
    summary: "完整身形与服装入画，适合穿搭展示与角色定妆",
    ratio: "2:3",
    fields: [
      { key: "subject", label: "人物", required: true, multiline: true },
      { key: "outfit", label: "完整穿着", required: true, multiline: true, hint: "上装、下装、鞋、配饰，模特展示要写全" },
      { key: "pose", label: "姿势", default: "自然站姿，重心放在一条腿上，双手放松垂放" },
      { key: "lighting", label: "光线", default: "户外柔和日光，干净通透" },
      { key: "lens", label: "镜头", default: "50mm 定焦，全身构图无畸变" },
    ],
    prompt: `生成一张全身人物照片。
人物：{{subject}}。
穿着：{{outfit}}。
姿势：{{pose}}。
镜头：{{lens}}，人物从头到脚完整入画，脚不要贴画面下边缘。
光线：{{lighting}}。
背景：简洁的实景背景，纵深虚化，不喧宾夺主。
质感：真实摄影，准确的服装材质与褶皱，自然肤色。`,
  },
  {
    id: "photo-editorial",
    title: "时尚杂志大片",
    category: "人物摄影",
    summary: "杂志级的造型、光影与调性",
    ratio: "2:3",
    fields: [
      { key: "subject", label: "人物", required: true, multiline: true },
      { key: "outfit", label: "造型", required: true, multiline: true },
      { key: "scene", label: "场景", required: true, multiline: true },
      { key: "mood", label: "调性", default: "克制、高级、冷调" },
      { key: "camera", label: "镜头与机位", default: "85mm，低机位轻微仰拍，中远景" },
    ],
    prompt: `生成一张时尚杂志风格的编辑大片。
人物：{{subject}}。
造型：{{outfit}}。
场景：{{scene}}。
调性：{{mood}}。
镜头：{{camera}}。
光线：有方向性的硬光与大片留白，轮廓清晰，阴影干净有层次。
后期：高级杂志的调色逻辑，低饱和的暗部与干净的高光，画面留白给标题排版。
画面：单一主体，无文字。`,
  },
  {
    id: "photo-street",
    title: "街头纪实",
    category: "人物摄影",
    summary: "抓拍感的生活片段，35mm 直出气质",
    ratio: "3:2",
    fields: [
      { key: "subject", label: "人物", required: true, multiline: true },
      { key: "moment", label: "瞬间", required: true, multiline: true, hint: "人物在做什么、情绪如何" },
      { key: "place", label: "地点", required: true, multiline: true },
      { key: "lens", label: "镜头", default: "35mm 定焦，手持快照感" },
    ],
    prompt: `生成一张街头纪实照片。
人物：{{subject}}。
瞬间：{{moment}}。
地点：{{place}}。
镜头：{{lens}}。
构图：抓拍瞬间，人物不必看镜头，画面里保留环境里的生活细节与偶然性。
光线：现场自然光，不打闪光灯。
质感：轻微颗粒与自然的高光溢出，颜色真实不刻意调色。`,
  },
  {
    id: "photo-id",
    title: "证件照 / 职业头像",
    category: "人物摄影",
    summary: "正面免冠、纯色背景，可直接当简历与工牌照",
    ratio: "3:4",
    fields: [
      {
        key: "subject",
        label: "人物",
        required: true,
        multiline: true,
        hint: "性别、年龄、发型、脸型；不要写夸张造型",
      },
      { key: "outfit", label: "服装", default: "深色西装外套配白衬衫" },
      { key: "background", label: "背景色", default: "纯白色" },
    ],
    prompt: `生成一张标准证件照风格的人像。
人物：{{subject}}。
服装：{{outfit}}。
构图：正面免冠，双肩平齐，头部居中，头顶留出约一个额头的空间，取到胸口以上。
表情：自然中性，嘴角放松，双眼平视镜头。
光线：正面均匀柔光，面部没有明显阴影，肤色准确，不磨皮过度。
背景：{{background}}无缝纯色背景，人物边缘干净。
要求：不戴帽子与夸张饰品，不化浓妆，不出现文字与水印。`,
  },
  {
    id: "photo-pet",
    title: "宠物写真",
    category: "人物摄影",
    summary: "毛发与眼神光都清晰的宠物肖像",
    ratio: "4:5",
    fields: [
      {
        key: "pet",
        label: "宠物",
        required: true,
        multiline: true,
        hint: "品种、毛色、体型、年龄感",
      },
      { key: "scene", label: "环境", default: "浅色布艺沙发旁，背景柔和虚化" },
      { key: "mood", label: "氛围", default: "温暖、安静、有陪伴感" },
    ],
    prompt: `生成一张宠物肖像照片。
宠物：{{pet}}。
环境：{{scene}}。
氛围：{{mood}}。
构图：宠物头部与胸部入画，眼睛位于画面上三分之一，视线朝向镜头或侧前方。
光线：窗边自然光，眼睛里有清晰的环形眼神光，毛发边缘有柔和的轮廓光。
质感：真实摄影，毛发一根根可辨，鼻头湿润，胡须清晰，浅景深。
要求：画面里不出现人物、文字与水印。`,
  },
];
