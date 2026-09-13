import type { Product } from "../types";

export interface CartLine {
  product: Product;
  qty: number;
  unitPrice: number;
}

class Cart {
  lines = $state<CartLine[]>([]);
  discount = $state(0);

  get subtotal() {
    return this.lines.reduce((s, l) => s + l.qty * l.unitPrice, 0);
  }
  get total() {
    return Math.max(0, this.subtotal - this.discount);
  }
  get count() {
    return this.lines.reduce((s, l) => s + l.qty, 0);
  }
  get isEmpty() {
    return this.lines.length === 0;
  }

  add(product: Product, qty = 1) {
    const line = this.lines.find((l) => l.product.id === product.id);
    if (line) line.qty += qty;
    else this.lines.push({ product, qty, unitPrice: product.sellPrice });
  }
  setQty(productId: number, qty: number) {
    const line = this.lines.find((l) => l.product.id === productId);
    if (!line) return;
    if (qty <= 0) this.remove(productId);
    else line.qty = qty;
  }
  remove(productId: number) {
    this.lines = this.lines.filter((l) => l.product.id !== productId);
  }
  clear() {
    this.lines = [];
    this.discount = 0;
  }
}
export const cart = new Cart();
