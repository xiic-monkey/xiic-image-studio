/**
 * 提示词模板的数据结构与合成逻辑。
 *
 * 模板 = 一组待填字段（槽位）+ 一段带 {{占位符}} 的提示词骨架。
 * 选模板 → 填字段 → 合成提示词 → 插进工作台的提示词框。
 *
 * 设计取舍：
 * - 不用「负向提示词」独立字段：当前后端协议没有 negative 参数，
 *   模板里的排除项一律写成正向描述（Google 官方也是这个建议）。
 * - 尺寸不写死像素：模板只声明建议画面比例，由 pickSizeForRatio
 *   按当前模型的能力表挑最接近的合法尺寸，避免换供应商就发非法参数。
 */

/** 模板里的一个待填字段（槽位）。 */
export interface TemplateField {
  /** 占位符名，提示词里用 {{key}} 引用。 */
  key: string;
  /** 界面上的字段名。 */
  label: string;
  /** 输入框下方的写法提示。 */
  hint?: string;
  /** 预填的常用值；用户没填就用它。 */
  default?: string;
  /** 必填项：没填不允许插入。 */
  required?: boolean;
  /** 多行输入（描述性字段用）。 */
  multiline?: boolean;
}

/** 工作流里的一步：独立的一条提示词。 */
export interface TemplateStep {
  title: string;
  prompt: string;
  note?: string;
}

/** 一个提示词模板。 */
export interface PromptTemplate {
  id: string;
  title: string;
  /** 所属分类，用于面板分组。 */
  category: string;
  /** 一句话说明这个模板产出什么。 */
  summary: string;
  fields: TemplateField[];
  /** 提示词骨架，含 {{占位符}}。 */
  prompt: string;
  /** 建议画面比例，如 "16:9"；按能力表挑最接近的合法尺寸。 */
  ratio?: string;
  /** 工作流：同一模板下的多步提示词，按顺序执行。 */
  steps?: TemplateStep[];
  /** 是否建议配合参考图（角色一致性类模板）。 */
  needsRef?: boolean;
  tags?: string[];
}

/** 合成结果。missing 里是还没填的必填字段名。 */
export interface ComposedTemplate {
  prompt: string;
  missing: string[];
}

/** 模板列表按分类去重后的顺序（保持首次出现顺序）。 */
export function templateCategories(list: PromptTemplate[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const t of list) {
    if (!seen.has(t.category)) {
      seen.add(t.category);
      out.push(t.category);
    }
  }
  return out;
}

/** 模板的初始字段值（取各字段的 default）。 */
export function defaultValues(tpl: PromptTemplate): Record<string, string> {
  const values: Record<string, string> = {};
  for (const f of tpl.fields) values[f.key] = f.default ?? "";
  return values;
}

/**
 * 合成模板：把字段值填进骨架。
 * 未填的可选字段按「整段省略」处理——空片段、空括号、空行都会被清掉，
 * 不会在提示词里留下 ", ," 这类脏东西。
 */
export function composeTemplate(tpl: PromptTemplate, values: Record<string, string>): ComposedTemplate {
  const missing = tpl.fields
    .filter((f) => f.required && !(values[f.key] ?? "").trim())
    .map((f) => f.label);
  return { prompt: composeText(tpl.prompt, values), missing };
}

/** 合成工作流里的一步（用同一批字段值）。 */
export function composeStep(
  step: TemplateStep,
  values: Record<string, string>,
): string {
  return composeText(step.prompt, values);
}

/** 占位符替换 + 空段清理。 */
export function composeText(text: string, values: Record<string, string>): string {
  const filled = text.replace(/\{\{\s*([\w-]+)\s*\}\}/g, (_m, key: string) =>
    (values[key] ?? "").trim(),
  );
  return tidy(filled);
}

/**
 * 清理合成结果：丢掉空片段、空括号、空行，统一分隔。
 *
 * 按「子句」处理——中文提示词里分句常用「。」，英文常用「,」，
 * 两种都算子句边界。可选字段没填时留下的「附加：。」这种残渣
 * （冒号后面没内容）整句丢掉，连同它的句号。
 */
function tidy(text: string): string {
  const lines = text.split("\n").map((raw) => {
    const kept: string[] = [];
    for (const clause of splitClauses(raw)) {
      const body = clause.body.replace(/[（(]\s*[）)]/g, "").trim();
      if (!body) continue;
      // 「标签：」后面没内容 → 整句丢掉（连同它的分隔符）
      if (/[：:]$/.test(body)) continue;
      kept.push(body + clause.delim);
    }
    return kept.join("").replace(/[ \t]{2,}/g, " ");
  });
  return lines
    .map((l) => l.trim())
    .filter((l) => l.length > 0)
    .join("\n");
}

/** 把一行按中英文逗号/句号/分号切成子句，保留各自的收尾分隔符。 */
function splitClauses(line: string): { body: string; delim: string }[] {
  const out: { body: string; delim: string }[] = [];
  let body = "";
  for (const ch of line) {
    if (ch === "," || ch === "，" || ch === "。" || ch === "；" || ch === ";") {
      out.push({ body, delim: ch });
      body = "";
    } else {
      body += ch;
    }
  }
  if (body.trim()) out.push({ body, delim: "" });
  return out;
}

/** "1024x1536" / "1024*768" / "3:2" → 宽高比数值；认不出来返回 undefined。 */
export function parseRatio(value: string): number | undefined {
  const m = /^(\d+(?:\.\d+)?)\s*[:x*×]\s*(\d+(?:\.\d+)?)$/.exec(value.trim());
  if (!m) return undefined;
  const w = Number(m[1]);
  const h = Number(m[2]);
  if (!w || !h) return undefined;
  return w / h;
}

/**
 * 按建议比例挑当前模型能力表里最接近的合法尺寸。
 * 能力表为空（协议不收 size）→ undefined，不硬塞一个上游会拒的值。
 */
export function pickSizeForRatio(
  sizes: string[] | undefined,
  ratio?: string,
): string | undefined {
  if (!sizes || sizes.length === 0 || !ratio) return undefined;
  if (sizes.includes(ratio)) return ratio;
  const target = parseRatio(ratio);
  if (target === undefined) return undefined;
  let best: { size: string; diff: number } | undefined;
  for (const s of sizes) {
    const r = parseRatio(s);
    if (r === undefined) continue;
    const diff = Math.abs(r - target);
    if (!best || diff < best.diff - 1e-9) best = { size: s, diff };
  }
  return best?.size;
}
