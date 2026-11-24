import type { PromptTemplate } from "../lib/template.ts";

/**
 * 插画 / 风格类模板。
 *
 * 画风与媒介要写具体（媒介 + 笔触 + 上色方式），业内经验是
 * 「medium + line quality + palette + rendering」四件套最稳，
 * 只写「二次元风格」模型会自由发挥。
 */
export const ILLUSTRATION_TEMPLATES: PromptTemplate[] = [
  {
    id: "illus-anime",
    title: "二次元角色立绘",
    category: "插画风格",
    summary: "干净的赛璐璐角色插画，适合 OC 与角色宣传图",
    ratio: "2:3",
    fields: [
      { key: "character", label: "角色形象", required: true, multiline: true },
      { key: "costume", label: "服装", required: true, multiline: true },
      { key: "style", label: "画风", default: "日系赛璐璐动画风格，干净锐利的线稿，柔和的渐变上色，饱和但不过曝的色彩" },
      { key: "background", label: "背景", default: "简洁的纯色背景，带一点点光晕" },
    ],
    prompt: `生成一张二次元角色插画。
角色：{{character}}。
服装：{{costume}}。
画风：{{style}}。
构图：单人半身或全身入画，姿态自然有张力，视线与身体形成明确的动势。
背景：{{background}}。
光线：柔和的主光加淡淡的边缘光，人物轮廓清晰。
要求：不出现文字、水印与多余人物。`,
  },
  {
    id: "illus-flat",
    title: "扁平插画",
    category: "插画风格",
    summary: "几何化色块与极简造型，适合信息图与 banner",
    ratio: "3:2",
    fields: [
      { key: "subject", label: "主体", required: true, multiline: true },
      { key: "palette", label: "配色", default: "三色配色：深蓝、暖黄、米白" },
      { key: "style", label: "风格细节", default: "几何化剪影，简化的大色块，粗细一致的线条" },
    ],
    prompt: `生成一张扁平风格插画。
主体：{{subject}}。
配色：{{palette}}。
风格细节：{{style}}。
构图：主体明确居中，形状概括干净，视觉重心稳定。
光线：不用写实光影，用色块与留白表达层次。
背景：纯色底，色块之间界限清晰。
要求：画面中不出现文字与水印。`,
  },
  {
    id: "illus-watercolor",
    title: "水彩绘本",
    category: "插画风格",
    summary: "纸纹水彩与彩铅质感，适合儿童绘本与治愈系插画",
    ratio: "4:3",
    fields: [
      { key: "subject", label: "主体", required: true, multiline: true },
      { key: "scene", label: "场景", required: true, multiline: true },
      { key: "mood", label: "氛围", default: "温暖、宁静、有一点童话感" },
    ],
    prompt: `生成一张水彩绘本风格的插画。
主体：{{subject}}。
场景：{{scene}}。
氛围：{{mood}}。
画风：纸上水彩与彩铅结合，能看到纸纹与颜料沉淀，边缘有自然的水痕晕染，笔触松弛。
光线：柔和漫射的午后光线。
构图：画面留白充足，主体周围留出干净的呼吸空间。
要求：不出现文字与水印。`,
  },
  {
    id: "illus-concept",
    title: "游戏概念设定图",
    category: "插画风格",
    summary: "世界观概念原画，强调氛围与设计意图",
    ratio: "16:9",
    fields: [
      { key: "subject", label: "主体与环境", required: true, multiline: true, hint: "角色或场景 + 它所处的世界" },
      { key: "art", label: "美术方向", default: "厚涂概念原画，笔触概括，色彩块面分明" },
      { key: "mood", label: "氛围", required: true, multiline: true },
      { key: "focus", label: "设计重点", default: "整体剪影与色彩关系，交代设计意图" },
    ],
    prompt: `生成一张游戏概念原画（concept art）。
主体与环境：{{subject}}。
美术方向：{{art}}。
氛围：{{mood}}。
设计重点：{{focus}}。
构图：以中远景交代主体与环境的比例关系，剪影清晰可读。
光线：能塑形的光源，色温对比明确，暗部有细节。
质感：概念设计稿的完成度，画面里不出现文字说明与标注线。`,
  },
  {
    id: "illus-anime-film",
    title: "日式动画电影（吉卜力风）",
    category: "插画风格",
    summary: "手绘赛璐璐动画电影感，柔和自然色与厚实背景",
    ratio: "16:9",
    fields: [
      {
        key: "scene",
        label: "画面内容",
        required: true,
        multiline: true,
        hint: "主体 + 所在环境 + 正在发生什么",
      },
      { key: "mood", label: "氛围", default: "安静、温暖、带一点怀旧" },
    ],
    prompt: `生成一张日式手绘动画电影风格的单幅画面。
画面内容：{{scene}}。
氛围：{{mood}}。
画风：赛璐璐平涂的人物与手绘水粉背景，线条柔和干净，天空有厚实、有体积感的积云，草地与树叶用成组的色块表现，颜色偏低饱和的自然色。
光线：柔和的自然光，云层缝隙漏下的光斑，阴影边缘不生硬。
构图：像动画电影里的一个定格镜头，画面有前中后景的层次。
要求：不要 3D 渲染的塑料质感，不要数字感过强的光滑渐变，画面里不出现文字与水印。`,
  },
  {
    id: "illus-3d-toon",
    title: "3D 动画电影（皮克斯风）",
    category: "插画风格",
    summary: "圆润可爱的 3D 动画长片质感",
    ratio: "16:9",
    fields: [
      {
        key: "subject",
        label: "角色 / 主体",
        required: true,
        multiline: true,
        hint: "外形、年龄感、服装、性格",
      },
      { key: "scene", label: "场景", required: true, multiline: true },
      { key: "mood", label: "氛围", default: "明亮、欢快、有故事感" },
    ],
    prompt: `生成一张 3D 动画电影风格的单幅画面。
主体：{{subject}}。
场景：{{scene}}。
氛围：{{mood}}。
造型：角色比例夸张圆润，头大眼大，五官概括，服装有清晰的布料厚度与褶皱。
渲染：动画长片级 3D 渲染，次表面散射的皮肤，柔和的全局光照，接触阴影干净，浅景深。
色彩：明快的电影级调色，高光柔和不刺眼。
要求：不要写实照片感，不要恐怖谷的人脸，画面里不出现文字与水印。`,
  },
  {
    id: "illus-clay",
    title: "粘土定格动画",
    category: "插画风格",
    summary: "手工粘土质感，指纹与微缩布景都在",
    ratio: "4:3",
    fields: [
      { key: "subject", label: "主体", required: true, multiline: true },
      { key: "scene", label: "微缩场景", required: true, multiline: true },
    ],
    prompt: `生成一张粘土定格动画风格的画面。
主体：{{subject}}。
微缩场景：{{scene}}。
材质：所有造型都由手工粘土捏成，表面有指纹、压痕与轻微的不对称，边缘圆钝，能看到手工修整的痕迹。
布景：微缩模型布景，纸板、木片与织物搭建，接缝自然可见。
光线：小型棚拍的柔和布光，轻微顶光，投影短而软。
质感：定格动画的逐帧质感，景深较浅，画面有细小的灰尘与颗粒。
要求：不要光滑的 3D 塑料感，画面里不出现文字与水印。`,
  },
  {
    id: "illus-blindbox",
    title: "盲盒公仔（3D 潮玩）",
    category: "插画风格",
    summary: "Q 版手办造型，PVC 材质与棚拍光",
    ratio: "1:1",
    fields: [
      {
        key: "character",
        label: "角色造型",
        required: true,
        multiline: true,
        hint: "发型、服装、配色、一个标志性道具",
      },
      { key: "palette", label: "配色", default: "马卡龙色系，主色浅粉与奶油白" },
    ],
    prompt: `生成一张 3D 潮玩盲盒公仔的产品图。
角色造型：{{character}}。
配色：{{palette}}。
比例：Q 版两头半身，头大身小，四肢短圆。
材质：PVC 与哑光漆，发丝与衣角有轻微的模具分件线，眼睛是高光清晰的滴胶质感。
姿态：站在圆形底座上，动作可爱但稳定。
光线：影棚柔光箱照明，干净的接触阴影，背景是柔和的单色渐变。
要求：产品级渲染精度，画面里不出现文字、包装盒与水印。`,
  },
  {
    id: "illus-pixel",
    title: "像素艺术",
    category: "插画风格",
    summary: "16-bit 像素画，有限调色板与硬边像素",
    ratio: "1:1",
    fields: [
      { key: "subject", label: "画面内容", required: true, multiline: true },
      { key: "view", label: "视角", default: "四分之三俯视的等距视角" },
    ],
    prompt: `生成一张像素艺术插画。
画面内容：{{subject}}。
视角：{{view}}。
像素规范：16-bit 风格，硬边像素，不做抗锯齿，不做平滑渐变；每个色块都由明确的像素点组成。
调色板：限制在 24 色以内，颜色之间有清晰的明暗分层，暗部用冷色偏移。
构图：主体居中，边缘留出至少 16 像素的安全边距。
要求：不要 3D 渲染，不要模糊与景深，画面里不出现文字与水印。`,
  },
  {
    id: "illus-isometric",
    title: "等距微缩场景",
    category: "插画风格",
    summary: "45 度等距视角的小世界，细节密度高",
    ratio: "1:1",
    fields: [
      {
        key: "scene",
        label: "小世界",
        required: true,
        multiline: true,
        hint: "一个地点或一组建筑，写清里面有什么",
      },
      { key: "style", label: "渲染风格", default: "干净的 3D 渲染，柔和的全局光照，低饱和暖色" },
    ],
    prompt: `生成一张等距视角的微缩场景插画。
小世界：{{scene}}。
渲染风格：{{style}}。
视角：严格的等距投影，相机从 45 度俯视，三个可见面受力均匀，没有透视畸变。
构图：整个场景像一块悬浮的方形地台，边缘有干净的地层剖面，主体建筑居中。
细节：屋顶、窗框、植被、路灯、小道具的密度要高，但每个元素轮廓清晰。
光线：柔和的顶光加轻微的环境光遮蔽，投影方向统一。
要求：不要人物特写，不要文字与水印。`,
  },
  {
    id: "illus-sketch",
    title: "铅笔素描",
    category: "插画风格",
    summary: "纸上铅笔 / 炭笔的单色手稿质感",
    ratio: "4:3",
    fields: [
      { key: "subject", label: "描绘对象", required: true, multiline: true },
      { key: "paper", label: "纸张与工具", default: "米白色粗纹素描纸，2B 铅笔与软炭笔" },
    ],
    prompt: `生成一张铅笔素描手稿。
描绘对象：{{subject}}。
纸张与工具：{{paper}}。
技法：排线塑造明暗，亮部留白，暗部用交叉排线压深，边缘有轻微的橡皮擦拭痕迹。
构图：主体占据画面主要位置，四周留出手稿般的空白。
质感：能看清纸张纹理与石墨反光，线条有轻重变化。
要求：单色，不上色，不出现签名、文字与水印。`,
  },
  {
    id: "illus-cyberpunk",
    title: "赛博朋克霓虹",
    category: "插画风格",
    summary: "雨夜霓虹与湿路面倒影，电影感强",
    ratio: "16:9",
    fields: [
      { key: "subject", label: "主体", required: true, multiline: true },
      { key: "scene", label: "环境", required: true, multiline: true },
      { key: "palette", label: "霓虹主色", default: "青色与洋红，少量琥珀色点缀" },
    ],
    prompt: `生成一张赛博朋克风格的夜景画面。
主体：{{subject}}。
环境：{{scene}}。
霓虹主色：{{palette}}。
光线：霓虹灯是主要光源，湿润的路面与金属表面有清晰的反光，空气里有体积雾与光晕。
构图：低机位广角，主体与密集的城市结构形成压迫感，画面前景有虚化的遮挡物。
质感：电影级调色，高光有轻微辉光，暗部保留细节。
要求：招牌与屏幕上的文字全部虚化或做成不可读的图形，不出现真实品牌与可读文字。`,
  },
];
