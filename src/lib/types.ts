export type Role = "owner" | "cashier";

export interface User {
  id: number;
  username: string;
  role: Role;
  active: boolean;
}

export interface Product {
  id: number;
  barcode: string | null;
  name: string;
  category: string | null;
  costPrice: number;
  sellPrice: number;
  stockQty: number;
  reorderLevel: number;
  active: boolean;
}

export interface ProductInput {
  id?: number;
  barcode?: string | null;
  name: string;
  category?: string | null;
  costPrice: number;
  sellPrice: number;
  reorderLevel: number;
}

export type PaymentMethod = "cash" | "mpesa";

export interface SaleInput {
  items: { productId: number; qty: number; unitPrice: number }[];
  discount: number;
  paymentMethod: PaymentMethod;
  mpesaCode?: string | null;
  cashTendered?: number | null;
}

export interface Sale {
  id: number;
  cashier: string;
  subtotal: number;
  discount: number;
  total: number;
  paymentMethod: PaymentMethod;
  mpesaCode: string | null;
  cashTendered: number | null;
  changeGiven: number | null;
  status: "completed" | "voided";
  voidReason: string | null;
  itemCount: number;
  createdAt: string;
}

export interface SaleItem {
  id: number;
  productId: number;
  productName: string;
  qty: number;
  unitPrice: number;
  lineTotal: number;
}

export interface SaleDetail {
  sale: Sale;
  items: SaleItem[];
}

export interface ReceiveLine {
  productId: number;
  qty: number;
  unitCost?: number | null;
}

export type MovementReason = "sale" | "void" | "purchase" | "adjustment" | "damage" | "count";

export interface StockMovement {
  id: number;
  productId: number;
  productName: string;
  qtyDelta: number;
  reason: MovementReason;
  refSaleId: number | null;
  note: string | null;
  user: string;
  createdAt: string;
}

export interface AuditEntry {
  id: number;
  user: string | null;
  action: string;
  entity: string;
  entityId: number | null;
  details: string;
  createdAt: string;
}

export interface SalesSummary {
  salesCount: number;
  gross: number;
  discounts: number;
  net: number;
  cost: number;
  profit: number;
  cashTotal: number;
  mpesaTotal: number;
  itemsSold: number;
  voidedCount: number;
}

export interface DailyPoint {
  day: string;
  salesCount: number;
  net: number;
  profit: number;
}

export interface TopProduct {
  productId: number;
  name: string;
  qty: number;
  revenue: number;
  profit: number;
}

export interface StockValue {
  productCount: number;
  units: number;
  costValue: number;
  retailValue: number;
  lowStockCount: number;
}

export interface SyncStatus {
  configured: boolean;
  connected: boolean;
  syncing: boolean;
  pending: number;
  email: string | null;
  lastOk: string | null;
  lastError: string | null;
}

export interface ImportRow {
  name: string;
  barcode?: string | null;
  category?: string | null;
  sellPrice?: number | null;
  costPrice?: number | null;
  stockQty?: number | null;
}
export interface ImportResult {
  created: number;
  updated: number;
  skipped: number;
}
