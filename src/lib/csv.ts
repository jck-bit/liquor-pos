import type { ImportRow } from "./types";
import { toCents } from "./format";

/** Small RFC 4180 parser: handles quoted fields, escaped quotes and CRLF. */
export function parseCsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"') {
        if (text[i + 1] === '"') { field += '"'; i++; }
        else quoted = false;
      } else field += c;
    } else if (c === '"') quoted = true;
    else if (c === ",") { row.push(field); field = ""; }
    else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      row.push(field); field = "";
      if (row.some((f) => f.trim() !== "")) rows.push(row);
      row = [];
    } else field += c;
  }
  row.push(field);
  if (row.some((f) => f.trim() !== "")) rows.push(row);
  return rows;
}

type Field = keyof ImportRow;
const ALIASES: Record<Field, string[]> = {
  name: ["name", "product", "productname", "item", "itemname", "description"],
  barcode: ["barcode", "code", "ean", "upc", "sku"],
  category: ["category", "cat", "type", "group"],
  sellPrice: ["price", "sellprice", "sellingprice", "sell", "retail", "retailprice", "unitprice"],
  costPrice: ["cost", "costprice", "buyingprice", "buyprice", "purchaseprice"],
  stockQty: ["stock", "qty", "quantity", "physicalqty", "count", "onhand", "instock", "physical"],
};

export interface Mapping {
  columns: Partial<Record<Field, number>>;
  headers: string[];
}

/** Guess which spreadsheet column holds which field from the header row. */
export function detectColumns(headers: string[]): Mapping {
  const norm = headers.map((h) => h.toLowerCase().replace(/[^a-z0-9]/g, ""));
  const columns: Partial<Record<Field, number>> = {};
  for (const [field, names] of Object.entries(ALIASES) as [Field, string[]][]) {
    const idx = norm.findIndex((h) => names.includes(h));
    if (idx >= 0) columns[field] = idx;
  }
  return { columns, headers };
}

export function rowsToImport(rows: string[][], m: Mapping): ImportRow[] {
  const get = (r: string[], f: Field) => (m.columns[f] === undefined ? undefined : (r[m.columns[f]!] ?? "").trim());
  const num = (v: string | undefined) => (v === undefined || v === "" ? null : parseInt(v.replace(/[^0-9-]/g, "")) || 0);
  const money = (v: string | undefined) => (v === undefined || v === "" ? null : toCents(v));
  return rows
    .map((r) => ({
      name: get(r, "name") ?? "",
      barcode: get(r, "barcode") || null,
      category: get(r, "category") || null,
      sellPrice: money(get(r, "sellPrice")),
      costPrice: money(get(r, "costPrice")),
      stockQty: num(get(r, "stockQty")),
    }))
    .filter((r) => r.name !== "");
}
