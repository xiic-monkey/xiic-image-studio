import type { PromptTemplate } from "../lib/template.ts";

/**
 * 电商 / 商品类模板。
 *
 * 业内电商图的三件套：①白底主图（干净、可抠）②场景图（营造使用情境）
 * ③细节图（材质与做工）。模特图是第四件常见需求。
 */
export const COMMERCE_TEMPLATES: PromptTemplate[] = [
  {
    id: "ecom-white",
    title: "白底产品图（主图）",
    category: "电商产品",
    summary: "纯白背景居中主图，干净好抠，符合电商主图规范",
    ratio: "1:1",
    fields: [
      { key: "product", label: "产品", required: true, multiline: true, hint: "品类 + 颜色 + 材质 + 关键特征" },
      { key: "angle", label: "角度", default: "四分之三侧前方 15 度，略高于产品中心" },
      { key: "material", label: "材质与做工", default: "哑光表面，边缘做工精细，纹理清晰可见" },
      { key: "shadow", label: "投影", default: "产品下方一道极淡的落地软阴影" },
    ],
    prompt: `生成一张电商产品主图。
产品：{{product}}。
角度：{{angle}}。
材质与做工：{{material}}。
投影：{{shadow}}。
构图：单一产品居中，四周留出均匀安全边距，产品占画面约八成。
光线：柔和均匀的顶部大柔光箱加两侧补光，无高光溢出，色差准确。
背景：纯白无缝背景，背景与产品边缘分离干净。
要求：真实商品摄影质感，画面里没有人物、没有文字、没有道具、没有包装以外的杂物。`,
  },
  {
    id: "ecom-scene",
    title: "场景化带货图",
    category: "电商产品",
    summary: "把产品放进真实使用场景，卖点更直观",
    ratio: "4:3",
    fields: [
      { key: "product", label: "产品", required: true, multiline: true },
      { key: "scene", label: "使用场景", required: true, multiline: true },
      { key: "mood", label: "氛围与色调", default: "自然暖调，通透干净" },
    ],
    prompt: `生成一张场景化的产品展示图。
产品：{{product}}。
场景：{{scene}}。
氛围与色调：{{mood}}。
构图：产品是画面主体，放在场景里最自然的位置，与环境有明确的接触与投影关系。
光线：场景内的真实自然光，方向与色温统一。
质感：商业广告摄影，材质真实，有生活气息但不杂乱。
要求：画面中没有人物面部特写，没有文字与水印。`,
  },
  {
    id: "ecom-model",
    title: "模特上身图",
    category: "电商产品",
    summary: "真人穿着展示，用于服饰与穿戴类商品",
    ratio: "3:4",
    fields: [
      { key: "product", label: "商品", required: true, multiline: true },
      { key: "model", label: "模特形象", default: "亚洲年轻女性模特，身高 168 厘米左右" },
      { key: "pose", label: "姿势", default: "正面站姿微侧身，手自然扶住商品边缘" },
      { key: "scene", label: "场景", default: "干净的浅灰影棚背景" },
    ],
    prompt: `生成一张模特穿着展示图。
商品：{{product}}。
模特：{{model}}。
姿势：{{pose}}。
场景：{{scene}}。
构图：模特从头到脚完整入画，商品是视觉重点，版型与细节清晰可辨。
光线：柔和均匀的影棚光，商品上不留死黑阴影。
质感：真实电商摄影，肤色自然，服装褶皱与垂坠感准确。
要求：画面中没有文字与水印。`,
  },
  {
    id: "ecom-detail",
    title: "产品细节 / 材质特写",
    category: "电商产品",
    summary: "做工与材质微距，用于详情页与卖点证明",
    ratio: "1:1",
    fields: [
      { key: "product", label: "产品", required: true, multiline: true },
      { key: "detail", label: "要突出的细节", required: true, multiline: true, hint: "缝线 / 接口 / 纹理 / 铭牌 / 材质边缘" },
    ],
    prompt: `生成一张产品细节特写图。
产品：{{product}}。
细节：{{detail}}。
镜头：微距镜头，浅景深，焦点落在要表现的细节上。
光线：侧向柔光加轻微轮廓光，勾勒出纹理起伏与边缘。
背景：纯净且与产品同色系的底面。
质感：真实微距摄影，纹理清晰可辨，色彩准确，不做夸张锐化。`,
  },
  {
    id: "ecom-food",
    title: "美食摄影",
    category: "电商产品",
    summary: "餐厅菜单级的美食图，热气与质感都在",
    ratio: "4:3",
    fields: [
      {
        key: "dish",
        label: "菜品",
        required: true,
        multiline: true,
        hint: "菜名 + 主要食材 + 摆盘方式",
      },
      { key: "scene", label: "台面与道具", default: "深色胡桃木桌面，粗陶餐具，旁边散落几片香草" },
      { key: "angle", label: "机位", default: "45 度俯拍，靠近菜品" },
    ],
    prompt: `生成一张餐厅级的美食摄影。
菜品：{{dish}}。
台面与道具：{{scene}}。
机位：{{angle}}。
光线：侧逆光穿过食物边缘，勾出热气与油脂光泽，正面用反光板补亮暗部。
质感：食材纹理清晰，酱汁有流动感，餐具与台面有真实的材质细节。
构图：菜品是绝对主体，画面边缘有轻微虚化的道具作前景与背景。
要求：不出现人物、文字与品牌标识。`,
  },
];
