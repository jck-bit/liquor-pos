const kes = new Intl.NumberFormat("en-KE", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const whole = new Intl.NumberFormat("en-KE");

/** Cents -> "1,250.00" */
export const money = (cents: number) => kes.format(cents / 100);
/** Cents -> "KES 1,250.00" */
export const kesMoney = (cents: number) => `KES ${money(cents)}`;
export const int = (n: number) => whole.format(n);

/** "1250.50" -> 125050. Blank or invalid -> 0. */
export function toCents(text: string | number): number {
  const n = typeof text === "number" ? text : parseFloat(String(text).replace(/[^0-9.-]/g, ""));
  return Number.isFinite(n) ? Math.round(n * 100) : 0;
}
/** 125050 -> "1250.50" for prefilling inputs */
export const fromCents = (cents: number) => (cents / 100).toFixed(2);

export function marginPct(cost: number, price: number): string {
  if (price <= 0) return "—";
  return `${Math.round(((price - cost) / price) * 100)}%`;
}

/** Local date as YYYY-MM-DD */
export function isoDate(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}
export function addDays(iso: string, days: number): string {
  const d = new Date(iso + "T00:00:00");
  d.setDate(d.getDate() + days);
  return isoDate(d);
}
/** Monday of the week containing the date */
export function weekStart(iso: string): string {
  const d = new Date(iso + "T00:00:00");
  const diff = (d.getDay() + 6) % 7;
  return addDays(iso, -diff);
}
export function monthStart(iso: string): string {
  return iso.slice(0, 8) + "01";
}

/** "2026-09-11 14:03:22" -> "11 Sep 2026, 14:03" */
export function fmtDateTime(s: string): string {
  const d = new Date(s.replace(" ", "T"));
  if (isNaN(d.getTime())) return s;
  return d.toLocaleString("en-KE", { day: "2-digit", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit", hour12: false });
}
export function fmtDate(s: string): string {
  const d = new Date(s.slice(0, 10) + "T00:00:00");
  if (isNaN(d.getTime())) return s;
  return d.toLocaleDateString("en-KE", { weekday: "short", day: "2-digit", month: "short" });
}
/** "2026-09-11 14:03:22" -> "14:03" */
export const fmtTime = (s: string) => s.slice(11, 16);

/** How long ago a local timestamp was: "just now", "12 min ago", "3 h ago", "2 days ago". */
export function ago(s: string, now = Date.now()): string {
  const t = new Date(s.replace(" ", "T")).getTime();
  if (isNaN(t)) return s;
  const min = Math.max(0, Math.round((now - t) / 60000));
  if (min < 2) return "just now";
  if (min < 60) return `${min} min ago`;
  if (min < 48 * 60) return `${Math.round(min / 60)} h ago`;
  return `${Math.round(min / 1440)} days ago`;
}

export const receiptNo = (id: number) => `#${String(id).padStart(6, "0")}`;
