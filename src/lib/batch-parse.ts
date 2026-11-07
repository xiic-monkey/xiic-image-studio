/**
 * 批量文件解析。抽成纯函数是为了能直接测：
 * 过去用 `line.split(",")` 切字段，提示词里带逗号就被切断——逗号后的内容
 * 还会被当成尺寸/质量下发到上游，轻则参数非法，重则生成内容完全跑偏。
 *
 * 格式按文件扩展名判定（.csv / .jsonl / .txt），认不出扩展名时再看内容：
 * 首行以 `{` 开头算 JSONL，以 `prompt,` 开头算 CSV，其余按纯文本。
 * 纯文本模式下整行就是一条提示词，不再按逗号切——那是切断提示词的元凶。
 */

export interface BatchItem {
  prompt: string;
  size?: string;
  quality?: string;
  seed?: string;
  n?: string;
}

export type BatchFormat = "csv" | "jsonl" | "text";

/** 按扩展名判定格式；认不出扩展名时退回看内容。 */
export function detectFormat(text: string, fileName?: string): BatchFormat {
  const ext = fileName?.toLowerCase().split(".").pop();
  if (ext === "csv") return "csv";
  if (ext === "jsonl" || ext === "ndjson") return "jsonl";
  if (ext === "txt" || ext === "text") return "text";

  const first = text
    .split(/\r?\n/)
    .map((s) => s.trim())
    .find(Boolean);
  if (!first) return "text";
  if (first.startsWith("{")) return "jsonl";
  if (/^prompt\s*(,|$)/i.test(first)) return "csv";
  return "text";
}

/**
 * 把整段文本按 CSV 记录切开：引号内的换行属于字段内容，不是记录分隔符。
 * `""` 是转义后的一个引号。
 */
export function parseCsvRecords(text: string): string[][] {
  const records: string[][] = [];
  let fields: string[] = [];
  let cur = "";
  let inQuotes = false;
  let i = 0;

  const endField = () => {
    fields.push(cur);
    cur = "";
  };
  const endRecord = () => {
    endField();
    records.push(fields);
    fields = [];
  };

  while (i < text.length) {
    const ch = text[i];
    if (inQuotes) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          cur += '"';
          i += 2;
          continue;
        }
        inQuotes = false;
        i += 1;
        continue;
      }
      cur += ch;
      i += 1;
      continue;
    }
    if (ch === '"') {
      inQuotes = true;
      i += 1;
      continue;
    }
    if (ch === ",") {
      endField();
      i += 1;
      continue;
    }
    if (ch === "\r") {
      i += 1;
      continue;
    }
    if (ch === "\n") {
      endRecord();
      i += 1;
      continue;
    }
    cur += ch;
    i += 1;
  }
  // 文件末尾没有换行时，最后一条记录还没结
  if (cur.length > 0 || fields.length > 0) endRecord();
  return records;
}

/** 一行 CSV 的字段切分（引号内的逗号不算分隔符）。 */
export function parseCsvLine(line: string): string[] {
  const [record] = parseCsvRecords(line);
  return record ?? [];
}

function itemFromJson(line: string): BatchItem | null {
  try {
    const o = JSON.parse(line);
    const p = o.prompt ?? o.p;
    if (!p) return null;
    return {
      prompt: String(p),
      size: o.size,
      quality: o.quality,
      seed: o.seed != null ? String(o.seed) : undefined,
      n: o.n != null ? String(o.n) : undefined,
    };
  } catch {
    return null;
  }
}

/**
 * 解析批量文件。`fileName` 用于判格式；不传时按内容猜。
 */
export function parseBatch(text: string, fileName?: string): BatchItem[] {
  const format = detectFormat(text, fileName);
  const items: BatchItem[] = [];

  if (format === "csv") {
    const records = parseCsvRecords(text);
    for (const [index, record] of records.entries()) {
      const [p, sz, q, sd, n] = record;
      const prompt = (p ?? "").trim();
      if (!prompt) continue;
      // 表头行（prompt,size,quality,seed）不当作数据
      if (index === 0 && /^prompt$/i.test(prompt)) continue;
      items.push({
        prompt,
        size: sz?.trim() || undefined,
        quality: q?.trim() || undefined,
        seed: sd?.trim() || undefined,
        n: n?.trim() || undefined,
      });
    }
    return items;
  }

  const lines = text
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter(Boolean);
  for (const line of lines) {
    if (format === "jsonl") {
      // JSONL：每行一个 JSON 对象；不是 JSON 的行当裸提示词
      const parsed = itemFromJson(line);
      items.push(parsed ?? { prompt: line });
      continue;
    }
    // 纯文本：整行就是一条提示词，逗号属于提示词本身
    items.push({ prompt: line });
  }
  return items.filter((i) => i.prompt);
}
