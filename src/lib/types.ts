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
  /** Receipt number on the computer that made the sale. */
  receiptNo: number;
  /** Name of the computer that made the sale. */
  till: string | null;
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
  refReceiptNo: number | null;
  /** Set when the movement was made on another computer. */
  till: string | null;
}

export interface AuditEntry {
  id: number;
  user: string | null;
  action: string;
  entity: string;
  entityId: number | null;
  details: string;
  createdAt: string;
  /** Set when the entry was made on another computer. */
  till: string | null;
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

export interface ShopInfo {
  id: string;
  name: string;
  connected: boolean;
}
export interface ShopList {
  shops: ShopInfo[];
  current: string;
}

// ---------- All shops dashboard ----------

export interface Till {
  name: string;
  lastSeen: string | null;
  thisComputer: boolean;
}
export interface ShopFigures {
  summary: SalesSummary;
  stock: StockValue;
  lastSaleAt: string | null;
  daily: DailyPoint[];
  tills: Till[];
  pending: number;
  lastSynced: string | null;
}
export interface ShopOverview {
  id: string;
  name: string;
  isOpen: boolean;
  connected: boolean;
  syncing: boolean;
  syncError: string | null;
  locked: boolean;
  error: string | null;
  figures: ShopFigures | null;
}
export interface BestSeller {
  name: string;
  qty: number;
  revenue: number;
  /** Quantity per shop, in the same order as AllShopsOverview.shops. */
  perShop: number[];
}
export interface AllShopsOverview {
  shops: ShopOverview[];
  bestSellers: BestSeller[];
}
export interface ShopRef {
  id: string;
  name: string;
  error: string | null;
}
export interface StockCell {
  qty: number;
  reorder: number;
  price: number;
  low: boolean;
}
export interface StockRow {
  name: string;
  category: string | null;
  cells: (StockCell | null)[];
  total: number;
  priceDiffers: boolean;
}
export interface StockMatrix {
  shops: ShopRef[];
  rows: StockRow[];
}
export interface ShopSale extends Sale {
  shopId: string;
  shopName: string;
}
export interface ReceiptStore {
  name: string;
  address: string | null;
  phone: string | null;
  footer: string | null;
}
export interface ShopSaleDetail {
  shopId: string;
  shopName: string;
  detail: SaleDetail;
  store: ReceiptStore;
}
