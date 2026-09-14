import type { ShopList, SyncStatus, User } from "../types";

export type Screen = "sell" | "products" | "stock" | "sales" | "reports" | "audit" | "users" | "settings";

class Session {
  user = $state<User | null>(null);
  settings = $state<Record<string, string>>({});
  screen = $state<Screen>("sell");
  shops = $state<ShopList | null>(null);
  /** One-off message for the login screen, e.g. after adding a shop. */
  notice = $state<string | null>(null);

  get isOwner() {
    return this.user?.role === "owner";
  }
  /** Only computers with more than one shop show the shop picker. */
  get multiShop() {
    return (this.shops?.shops.length ?? 0) > 1;
  }
  get storeName() {
    return this.settings.store_name || "Liquor POS";
  }
}
export const session = new Session();

class SyncState {
  status = $state<SyncStatus | null>(null);

  /** True when the cloud has not accepted anything for over an hour while changes are waiting. */
  get stale() {
    const s = this.status;
    if (!s?.configured || s.pending === 0) return false;
    if (!s.lastOk) return true;
    const last = new Date(s.lastOk.replace(" ", "T")).getTime();
    return Date.now() - last > 60 * 60 * 1000;
  }
}
export const syncState = new SyncState();

export type Toast = { id: number; kind: "info" | "success" | "error"; text: string };

class Toasts {
  list = $state<Toast[]>([]);
  private seq = 0;

  push(kind: Toast["kind"], text: string) {
    const id = ++this.seq;
    this.list.push({ id, kind, text });
    setTimeout(() => this.dismiss(id), kind === "error" ? 6000 : 2500);
  }
  dismiss(id: number) {
    this.list = this.list.filter((t) => t.id !== id);
  }
  success(text: string) {
    this.push("success", text);
  }
  info(text: string) {
    this.push("info", text);
  }
  /** Accepts the string Tauri rejects with, or any Error. */
  error(e: unknown) {
    const text = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
    this.push("error", text);
  }
}
export const toasts = new Toasts();
