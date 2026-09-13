import type { User } from "../types";

export type Screen = "sell" | "products" | "stock" | "sales" | "reports" | "audit" | "users" | "settings";

class Session {
  user = $state<User | null>(null);
  settings = $state<Record<string, string>>({});
  screen = $state<Screen>("sell");

  get isOwner() {
    return this.user?.role === "owner";
  }
  get storeName() {
    return this.settings.store_name || "Liquor POS";
  }
}
export const session = new Session();

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
