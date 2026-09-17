import { invoke } from "@tauri-apps/api/core";
import type {
  AllShopsOverview, ShopSale, ShopSaleDetail, StockMatrix,
  AuditEntry, DailyPoint, ImportResult, ImportRow, PaymentMethod, Product, ProductInput, ReceiveLine, Sale, SaleDetail,
  SaleInput, SalesSummary, ShopList, StockMovement, StockValue, SyncStatus, TopProduct, User,
} from "./types";

/** Thin typed wrapper over every backend command. Errors reject with a plain message string. */
export const api = {
  // auth
  login: (username: string, pin: string) => invoke<User>("login", { username, pin }),
  logout: () => invoke<void>("logout"),
  currentUser: () => invoke<User | null>("current_user"),

  // users
  listUsers: () => invoke<User[]>("list_users"),
  createUser: (username: string, pin: string, role: string) => invoke<User>("create_user", { username, pin, role }),
  setUserActive: (userId: number, active: boolean) => invoke<void>("set_user_active", { userId, active }),
  changePin: (userId: number, newPin: string) => invoke<void>("change_pin", { userId, newPin }),

  // products
  listProducts: (query = "", includeInactive = false) => invoke<Product[]>("list_products", { query, includeInactive }),
  findProductByBarcode: (barcode: string) => invoke<Product | null>("find_product_by_barcode", { barcode }),
  listCategories: () => invoke<string[]>("list_categories"),
  saveProduct: (input: ProductInput) => invoke<Product>("save_product", { input }),
  setProductActive: (productId: number, active: boolean) => invoke<void>("set_product_active", { productId, active }),
  importProducts: (rows: ImportRow[]) => invoke<ImportResult>("import_products", { rows }),

  // stock
  receiveStock: (lines: ReceiveLine[], note?: string) => invoke<void>("receive_stock", { lines, note: note || null }),
  /** For reason "count", pass the counted level; qtyDelta is ignored. */
  adjustStock: (productId: number, qtyDelta: number, reason: string, note?: string, counted?: number) =>
    invoke<Product>("adjust_stock", { productId, qtyDelta, reason, note: note || null, counted: counted ?? null }),
  listStockMovements: (productId?: number, limit = 200) =>
    invoke<StockMovement[]>("list_stock_movements", { productId: productId ?? null, limit }),
  lowStockProducts: () => invoke<Product[]>("low_stock_products"),

  // sales
  createSale: (input: SaleInput) => invoke<SaleDetail>("create_sale", { input }),
  getSale: (saleId: number) => invoke<SaleDetail>("get_sale", { saleId }),
  listSales: (from: string, to: string, paymentMethod?: PaymentMethod, limit = 500) =>
    invoke<Sale[]>("list_sales", { from, to, paymentMethod: paymentMethod ?? null, limit }),
  voidSale: (saleId: number, reason: string) => invoke<SaleDetail>("void_sale", { saleId, reason }),

  // reports
  salesSummary: (from: string, to: string) => invoke<SalesSummary>("sales_summary", { from, to }),
  dailySales: (from: string, to: string) => invoke<DailyPoint[]>("daily_sales", { from, to }),
  topProducts: (from: string, to: string, limit = 20) => invoke<TopProduct[]>("top_products", { from, to, limit }),
  stockValue: () => invoke<StockValue>("stock_value"),
  exportSalesCsv: (from: string, to: string) => invoke<string>("export_sales_csv", { from, to }),

  // audit + system
  listAudit: (limit = 200, offset = 0) => invoke<AuditEntry[]>("list_audit", { limit, offset }),
  getSettings: () => invoke<Record<string, string>>("get_settings"),
  updateSettings: (values: Record<string, string>) => invoke<void>("update_settings", { values }),
  backupDatabase: () => invoke<string>("backup_database"),
  databasePath: () => invoke<string>("database_path"),

  // cloud sync
  configureSync: (url: string, anonKey: string, email: string, password: string) =>
    invoke<SyncStatus>("configure_sync", { url, anonKey, email, password }),
  disableSync: () => invoke<void>("disable_sync"),
  syncNow: () => invoke<void>("sync_now"),
  syncStatus: () => invoke<SyncStatus>("sync_status"),

  // shops on this computer
  listShops: () => invoke<ShopList>("list_shops"),
  enterShop: (shopId: string) => invoke<ShopList>("enter_shop", { shopId }),
  addShop: (name: string) => invoke<ShopList>("add_shop", { name }),

  // all shops dashboard (owner only)
  allShopsOverview: (from: string, to: string, shopId?: string) =>
    invoke<AllShopsOverview>("all_shops_overview", { from, to, shopId: shopId ?? null }),
  allShopsStock: (shopId?: string) => invoke<StockMatrix>("all_shops_stock", { shopId: shopId ?? null }),
  allShopsSales: (from: string, to: string, shopId?: string, paymentMethod?: PaymentMethod, limit = 300) =>
    invoke<ShopSale[]>("all_shops_sales", { from, to, shopId: shopId ?? null, paymentMethod: paymentMethod ?? null, limit }),
  shopSaleDetail: (shopId: string, saleId: number) => invoke<ShopSaleDetail>("shop_sale_detail", { shopId, saleId }),
  refreshAllShops: () => invoke<void>("refresh_all_shops"),
};
