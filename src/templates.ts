// 内置预设模板（DESGIN #12）。用户从工作台「模板」入口插入到提示词框，
// 可选附带 size / quality；插入时只追加提示词文本，不覆盖用户已填内容。

export interface TemplateItem {
  title: string;
  prompt: string;
  size?: string;
  quality?: string;
}

export interface TemplateCategory {
  cat: string;
  items: TemplateItem[];
}

export const TEMPLATES: TemplateCategory[] = [
  {
    cat: "电商产品",
    items: [
      {
        title: "白底产品图",
        prompt: "专业电商产品摄影，纯白背景，居中构图，柔和环形光，高分辨率细节，商业级质感，8k",
        size: "1024x1024",
        quality: "high",
      },
      {
        title: "场景化带货",
        prompt: "产品置于自然生活场景中使用，浅景深，暖色调，真实材质质感，商业广告风格",
        size: "1536x1024",
      },
    ],
  },
  {
    cat: "人像头像",
    items: [
      {
        title: "写实头像",
        prompt: "写实风人物头像特写，自然光，细腻皮肤纹理，胶片质感，浅景深",
        size: "1024x1024",
        quality: "high",
      },
      {
        title: "卡通头像",
        prompt: "二次元动漫风格头像，大眼睛，清新配色，干净线条，精致上色",
        size: "1024x1024",
      },
    ],
  },
  {
    cat: "插画",
    items: [
      {
        title: "奇幻概念插画",
        prompt: "奇幻概念插画，宏大场景，戏剧性光影，丰富细节，数字绘画风格",
        size: "1536x1024",
      },
      {
        title: "扁平插画",
        prompt: "现代扁平风格插画，明快配色，几何形状，极简构图，矢量感",
        size: "1536x1024",
      },
    ],
  },
  {
    cat: "摄影",
    items: [
      {
        title: "风光摄影",
        prompt: "壮丽自然风光摄影，黄金时刻光线，广角构图，高动态范围，电影感",
        size: "1536x1024",
      },
      {
        title: "城市街拍",
        prompt: "城市街头摄影，手持快照感，自然色彩，生活化瞬间，35mm 镜头",
        size: "1024x1536",
      },
    ],
  },
];
