import type { PromptTemplate } from "../lib/template.ts";

/**
 * 场景 / 分镜类模板。
 *
 * 分镜模板按业内 storyboard 习惯走「一格一镜」的描述顺序：
 * 景别 → 运镜 → 主体动作 → 环境 → 光线。
 */
export const SCENE_TEMPLATES: PromptTemplate[] = [
  {
    id: "scene-concept",
    title: "场景概念图",
    category: "场景分镜",
    summary: "一个地点的整体设定，供后续所有镜头复用",
    ratio: "16:9",
    fields: [
      { key: "place", label: "地点", required: true, multiline: true },
      { key: "time", label: "时间与天气", default: "黄昏，阴天转晴" },
      { key: "style", label: "画风", default: "写实概念设计图，厚涂质感" },
      { key: "focus", label: "画面重点", default: "空间结构与关键陈设，交代这个地点能用来做什么" },
    ],
    prompt: `生成一张场景概念图。
地点：{{place}}。
时间与天气：{{time}}。
画风：{{style}}。
画面重点：{{focus}}。
构图：广角中景，交代空间布局、材质与陈设，人物只作为比例参照或完全不出镜。
光线：能读出空间结构的光，阴影方向统一，材质受光清晰。
质感：真实可信的环境摄影级细节，画面里不出现文字与标注。`,
  },
  {
    id: "storyboard",
    title: "分镜故事板",
    category: "场景分镜",
    summary: "一格一镜的镜头描述，画前先把镜头顺序定下来",
    ratio: "16:9",
    fields: [
      { key: "shot", label: "镜头内容", required: true, multiline: true, hint: "景别 + 运镜 + 主体动作，例如「中景，跟拍她推开便利店的门」" },
      { key: "subject", label: "主体", required: true, multiline: true },
      { key: "location", label: "环境", required: true, multiline: true },
      { key: "style", label: "画风", default: "灰阶分稿，线稿清晰，标注简洁" },
    ],
    prompt: `生成一张分镜故事板画面。
主体：{{subject}}。
环境：{{location}}。
镜头内容：{{shot}}。
画风：{{style}}。
版式：单格画面，边缘有简单的画框与序号位置，画面主体清晰。
光线：服从环境光，能读出人物与空间的关系。
要求：画面中不出现可读文字，画面只呈现镜头内容本身。`,
  },
  {
    id: "scene-keyvisual",
    title: "影视级主视觉 / 海报",
    category: "场景分镜",
    summary: "留出标题排版空间的主视觉",
    ratio: "2:3",
    fields: [
      { key: "subject", label: "主体", required: true, multiline: true },
      { key: "scene", label: "环境", required: true, multiline: true },
      { key: "mood", label: "调性", required: true, multiline: true },
      { key: "textArea", label: "文字留白位置", default: "画面上方三分之一大面积干净留白" },
    ],
    prompt: `生成一张影视级主视觉海报。
主体：{{subject}}。
环境：{{scene}}。
调性：{{mood}}。
文字留白：{{textArea}}，留白区域干净、对比可控，不放任何图形。
构图：主体压住画面下三分之二，视觉重心稳定，边缘有轻微暗角。
光线：戏剧性的方向光与体积雾，轮廓与主体分离清晰。
质感：高质量电影海报级渲染，画面中不出现任何文字与水印。`,
  },
  {
    id: "scene-interior",
    title: "室内设计效果图",
    category: "场景分镜",
    summary: "可给客户看的空间效果图，材质与尺度准确",
    ratio: "3:2",
    fields: [
      {
        key: "space",
        label: "空间与布局",
        required: true,
        multiline: true,
        hint: "房间类型、面积感、主要家具与动线",
      },
      { key: "style", label: "风格", default: "现代简约，原木与微水泥，低饱和暖色" },
      { key: "light", label: "采光", default: "大面落地窗的自然光，加少量暖色室内灯" },
    ],
    prompt: `生成一张室内设计效果图。
空间与布局：{{space}}。
风格：{{style}}。
采光：{{light}}。
镜头：24mm 广角，视线高度约 1.5 米，两点透视，垂直线保持竖直。
材质：木纹、石材、织物、金属的反射与粗糙度准确，家具比例符合人体尺度。
光线：自然光与室内灯色温统一，墙面与顶面过渡柔和，有真实的环境光遮蔽。
要求：空间整洁，不出现人物、文字与水印。`,
  },
  {
    id: "scene-arch",
    title: "建筑外观表现",
    category: "场景分镜",
    summary: "建筑效果图 / 建筑摄影，结构与材质清晰",
    ratio: "3:2",
    fields: [
      {
        key: "building",
        label: "建筑",
        required: true,
        multiline: true,
        hint: "类型、体量、层数、立面材料",
      },
      { key: "surround", label: "周边环境", default: "浅水景与低矮绿化，远处是干净的天空" },
      { key: "time", label: "时间光线", default: "傍晚蓝调时刻，室内灯刚亮起" },
    ],
    prompt: `生成一张建筑外观表现图。
建筑：{{building}}。
周边环境：{{surround}}。
时间光线：{{time}}。
视角：人视角或轻微仰拍的两点透视，建筑垂直线保持竖直，比例不失真。
材质：玻璃、金属、混凝土与木材的反射和质感真实，立面的分格与构造逻辑清楚。
光线：天空与建筑的光比控制在可读范围内，室内灯光透出但不曝光过度。
要求：画面里不出现人物特写、文字与品牌标识。`,
  },
];
