//! All shops dashboard: every shop on this computer, side by side.
//!
//! Each shop is read from its own database file through a short-lived, read-only
//! connection, with the same report queries the single-shop screens use. Shops
//! never share a connection, so nothing can leak from one shop into another.
//! The numbers for a shop that is not open are as fresh as its last background
//! sync (see `sync::run_others`).

use std::collections::BTreeMap;

use rusqlite::{params, Connection, Transaction, TransactionBehavior};
use serde::Serialize;
use tauri::State;

use crate::commands::auth::{require_owner, Session};
use crate::commands::reports::{check_range, daily_sales_for, sales_summary_for, stock_value_for};
use crate::commands::sales::{list_sales_for, load_sale};
use crate::db;
use crate::error::AppResult;
use crate::models::{DailyPoint, Sale, SaleDetail, SalesSummary, StockValue};
use crate::shops::{Shop, Shops};
use crate::sync::{pending_count, setting, Sync};

const BEST_SELLERS: usize = 15;

// ---------- what the screen receives ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Till {
    pub name: String,
    pub last_seen: Option<String>,
    pub this_computer: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopFigures {
    pub summary: SalesSummary,
    pub stock: StockValue,
    pub last_sale_at: Option<String>,
    pub daily: Vec<DailyPoint>,
    pub tills: Vec<Till>,
    /// Changes made on this computer that have not reached the shop's cloud yet.
    pub pending: i64,
    pub last_synced: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopOverview {
    pub id: String,
    pub name: String,
    pub is_open: bool,
    pub connected: bool,
    pub syncing: bool,
    pub sync_error: Option<String>,
    /// The logged-in person is not an owner in this shop, so its figures are withheld.
    pub locked: bool,
    pub error: Option<String>,
    pub figures: Option<ShopFigures>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BestSeller {
    pub name: String,
    pub qty: i64,
    pub revenue: i64,
    /// Quantity sold per shop, in the same order as `AllShopsOverview::shops`.
    pub per_shop: Vec<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllShopsOverview {
    pub shops: Vec<ShopOverview>,
    pub best_sellers: Vec<BestSeller>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopRef {
    pub id: String,
    pub name: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StockCell {
    pub qty: i64,
    pub reorder: i64,
    pub price: i64,
    pub low: bool,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StockRow {
    pub name: String,
    pub category: Option<String>,
    /// One cell per shop, `None` when that shop does not stock the product.
    pub cells: Vec<Option<StockCell>>,
    pub total: i64,
    pub price_differs: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockMatrix {
    pub shops: Vec<ShopRef>,
    pub rows: Vec<StockRow>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopSale {
    pub shop_id: String,
    pub shop_name: String,
    #[serde(flatten)]
    pub sale: Sale,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptStore {
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub footer: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopSaleDetail {
    pub shop_id: String,
    pub shop_name: String,
    pub detail: SaleDetail,
    pub store: ReceiptStore,
}

// ---------- reading one shop ----------

/// The logged-in owner, the shops their login unlocked, and the shop filter they picked.
pub struct Viewer {
    pub username: String,
    pub unlocked: Vec<String>,
    /// Show only this shop. `None` shows every shop.
    pub only: Option<String>,
}

impl Viewer {
    fn from_session(session: &Session, only: Option<String>) -> AppResult<Self> {
        let owner = require_owner(session)?;
        Ok(Self { username: owner.username, unlocked: session.unlocked(), only: only.filter(|id| !id.is_empty()) })
    }
    fn shops(&self, shops: &Shops) -> Vec<Shop> {
        shops.all().into_iter().filter(|s| self.only.as_ref().is_none_or(|id| *id == s.id)).collect()
    }
}

enum Unreadable {
    Locked,
    Failed(String),
}

impl<E: std::fmt::Display> From<E> for Unreadable {
    fn from(e: E) -> Self {
        Unreadable::Failed(e.to_string())
    }
}

impl Unreadable {
    fn message(&self, shop: &str) -> String {
        match self {
            Unreadable::Locked => format!("Your login does not open {shop}. Sign in with {shop}'s owner username and PIN, then give both shops the same PIN to see them together."),
            Unreadable::Failed(e) => e.clone(),
        }
    }
}

/// Run `f` against one shop's database: read-only, inside a single read
/// transaction so all of that shop's figures describe the same moment, and only
/// if the login unlocked that shop and is still an active owner there.
fn read_shop<T>(
    shops: &Shops,
    shop: &Shop,
    who: &Viewer,
    f: impl FnOnce(&Connection) -> AppResult<T>,
) -> Result<T, Unreadable> {
    if !who.unlocked.contains(&shop.id) {
        return Err(Unreadable::Locked);
    }
    let conn = db::open_existing(&shops.path_of(shop))?;
    conn.pragma_update(None, "query_only", 1)?;
    // Explicitly DEFERRED: the connection default is IMMEDIATE, which would take the write lock.
    let tx = Transaction::new_unchecked(&conn, TransactionBehavior::Deferred)?;
    let is_owner: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM users WHERE username = ?1 AND role = 'owner' AND active = 1)",
        params![who.username],
        |r| r.get(0),
    )?;
    if !is_owner {
        return Err(Unreadable::Locked);
    }
    Ok(f(&tx)?)
}

fn tills(conn: &Connection) -> AppResult<Vec<Till>> {
    let me = setting(conn, "device_id").unwrap_or_default();
    // Grouped by name: a computer that was set up twice appears once, with its latest sighting.
    let mut stmt = conn.prepare(
        "SELECT COALESCE(NULLIF(name, ''), device_id) AS till, MAX(last_seen), MAX(device_id = ?1)
         FROM devices GROUP BY till ORDER BY 2 DESC",
    )?;
    let rows = stmt.query_map(params![me], |r| {
        Ok(Till { name: r.get(0)?, last_seen: r.get(1)?, this_computer: r.get::<_, i64>(2)? == 1 })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// (product name, quantity, revenue) sold in the range, for the best-seller merge.
fn sold_by_name(conn: &Connection, from: &str, to: &str) -> AppResult<Vec<(String, i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT si.product_name, SUM(si.qty), SUM(si.line_total)
         FROM sale_items si JOIN sales s ON s.id = si.sale_id
         WHERE s.status = 'completed' AND s.created_at >= ?1 AND s.created_at < date(?2, '+1 day')
         GROUP BY si.product_name",
    )?;
    let rows = stmt.query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[derive(Debug, Clone)]
pub struct ProductStock {
    pub name: String,
    pub category: Option<String>,
    pub qty: i64,
    pub reorder: i64,
    pub price: i64,
}

fn product_stock(conn: &Connection) -> AppResult<Vec<ProductStock>> {
    let mut stmt =
        conn.prepare("SELECT name, category, stock_qty, reorder_level, sell_price FROM products WHERE active = 1")?;
    let rows = stmt.query_map([], |r| {
        Ok(ProductStock { name: r.get(0)?, category: r.get(1)?, qty: r.get(2)?, reorder: r.get(3)?, price: r.get(4)? })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

// ---------- merging shops (pure, unit-tested) ----------

/// Products are matched across shops by name, because every shop has its own ids.
pub fn name_key(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// One row per product name, one cell per shop, sorted by name.
pub fn merge_stock(per_shop: &[Vec<ProductStock>]) -> Vec<StockRow> {
    let shops = per_shop.len();
    let mut rows: BTreeMap<String, StockRow> = BTreeMap::new();
    for (i, products) in per_shop.iter().enumerate() {
        for p in products {
            let row = rows.entry(name_key(&p.name)).or_insert_with(|| StockRow {
                name: p.name.split_whitespace().collect::<Vec<_>>().join(" "),
                category: None,
                cells: vec![None; shops],
                total: 0,
                price_differs: false,
            });
            if row.category.is_none() {
                row.category = p.category.clone().filter(|c| !c.trim().is_empty());
            }
            // The same name twice inside one shop: add the quantities together.
            let cell = row.cells[i].get_or_insert(StockCell { qty: 0, reorder: p.reorder, price: p.price, low: false });
            cell.qty += p.qty;
            cell.low = cell.qty <= cell.reorder;
            row.total += p.qty;
        }
    }
    let mut rows: Vec<StockRow> = rows.into_values().collect();
    for row in &mut rows {
        let mut prices = row.cells.iter().flatten().map(|c| c.price);
        if let Some(first) = prices.next() {
            row.price_differs = prices.any(|p| p != first);
        }
    }
    rows
}

/// Best sellers across every shop, by combined revenue.
pub fn merge_best_sellers(per_shop: &[Vec<(String, i64, i64)>], limit: usize) -> Vec<BestSeller> {
    let shops = per_shop.len();
    let mut merged: BTreeMap<String, BestSeller> = BTreeMap::new();
    for (i, sold) in per_shop.iter().enumerate() {
        for (name, qty, revenue) in sold {
            let row = merged.entry(name_key(name)).or_insert_with(|| BestSeller {
                name: name.split_whitespace().collect::<Vec<_>>().join(" "),
                qty: 0,
                revenue: 0,
                per_shop: vec![0; shops],
            });
            row.qty += qty;
            row.revenue += revenue;
            row.per_shop[i] += qty;
        }
    }
    let mut rows: Vec<BestSeller> = merged.into_values().collect();
    rows.sort_by(|a, b| b.revenue.cmp(&a.revenue).then_with(|| a.name.cmp(&b.name)));
    rows.truncate(limit);
    rows
}

// ---------- commands ----------
// `async` runs them off the main thread, so reading several databases never stalls the window.

#[tauri::command(async)]
pub fn all_shops_overview(
    shops: State<Shops>,
    session: State<Session>,
    sync: State<Sync>,
    from: String,
    to: String,
    shop_id: Option<String>,
) -> AppResult<AllShopsOverview> {
    let who = Viewer::from_session(&session, shop_id)?;
    check_range(&from, &to)?;
    let open_status = sync.status();
    Ok(build_overview(&shops, &who, &from, &to, |shop, is_open| {
        if is_open {
            (open_status.syncing, open_status.last_error.clone())
        } else {
            let other = sync.other(&shop.id);
            (other.syncing, other.last_error)
        }
    }))
}

/// `sync_state(shop, is_open)` returns (syncing, last sync error) for a shop.
fn build_overview(
    shops: &Shops,
    who: &Viewer,
    from: &str,
    to: &str,
    sync_state: impl Fn(&Shop, bool) -> (bool, Option<String>),
) -> AllShopsOverview {
    let mut out = Vec::new();
    let mut sold = Vec::new();
    for shop in who.shops(shops) {
        let is_open = shops.is_current(&shop.id);
        let (syncing, sync_error) = sync_state(&shop, is_open);
        let read = read_shop(shops, &shop, who, |conn| {
            let figures = ShopFigures {
                summary: sales_summary_for(conn, from, to)?,
                stock: stock_value_for(conn)?,
                last_sale_at: conn.query_row("SELECT MAX(created_at) FROM sales WHERE status = 'completed'", [], |r| r.get(0))?,
                daily: daily_sales_for(conn, from, to)?,
                tills: tills(conn)?,
                pending: pending_count(conn),
                last_synced: setting(conn, "sync_last_ok"),
            };
            Ok((figures, sold_by_name(conn, from, to)?))
        });
        let (figures, locked, error) = match read {
            Ok((figures, names)) => {
                sold.push(names);
                (Some(figures), false, None)
            }
            Err(e) => {
                sold.push(Vec::new());
                (None, matches!(e, Unreadable::Locked), Some(e.message(&shop.name)))
            }
        };
        out.push(ShopOverview {
            id: shop.id,
            name: shop.name,
            is_open,
            connected: shop.connected,
            syncing,
            sync_error,
            locked,
            error,
            figures,
        });
    }
    AllShopsOverview { shops: out, best_sellers: merge_best_sellers(&sold, BEST_SELLERS) }
}

#[tauri::command(async)]
pub fn all_shops_stock(shops: State<Shops>, session: State<Session>, shop_id: Option<String>) -> AppResult<StockMatrix> {
    let who = Viewer::from_session(&session, shop_id)?;
    Ok(build_stock(&shops, &who))
}

fn build_stock(shops: &Shops, who: &Viewer) -> StockMatrix {
    let mut refs = Vec::new();
    let mut per_shop = Vec::new();
    for shop in who.shops(shops) {
        let (products, error) = match read_shop(shops, &shop, who, product_stock) {
            Ok(products) => (products, None),
            Err(e) => (Vec::new(), Some(e.message(&shop.name))),
        };
        per_shop.push(products);
        refs.push(ShopRef { id: shop.id, name: shop.name, error });
    }
    StockMatrix { shops: refs, rows: merge_stock(&per_shop) }
}

#[tauri::command(async)]
pub fn all_shops_sales(
    shops: State<Shops>,
    session: State<Session>,
    from: String,
    to: String,
    shop_id: Option<String>,
    payment_method: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<ShopSale>> {
    let who = Viewer::from_session(&session, shop_id)?;
    check_range(&from, &to)?;
    let limit = limit.unwrap_or(300).clamp(1, 2000);
    Ok(build_sales(&shops, &who, &from, &to, payment_method.as_deref(), limit))
}

fn build_sales(shops: &Shops, who: &Viewer, from: &str, to: &str, payment_method: Option<&str>, limit: i64) -> Vec<ShopSale> {
    let mut all = Vec::new();
    for shop in who.shops(shops) {
        // A shop that cannot be read is simply absent here; the Overview tab says why.
        if let Ok(sales) = read_shop(shops, &shop, who, |conn| list_sales_for(conn, from, to, payment_method, limit)) {
            all.extend(sales.into_iter().map(|sale| ShopSale { shop_id: shop.id.clone(), shop_name: shop.name.clone(), sale }));
        }
    }
    all.sort_by(|a, b| b.sale.created_at.cmp(&a.sale.created_at));
    all.truncate(limit as usize);
    all
}

/// A receipt from any shop, with that shop's own receipt header. Read-only:
/// voiding a sale still means opening its shop.
#[tauri::command(async)]
pub fn shop_sale_detail(
    shops: State<Shops>,
    session: State<Session>,
    shop_id: String,
    sale_id: i64,
) -> AppResult<ShopSaleDetail> {
    let who = Viewer::from_session(&session, None)?;
    let shop = shops.get(&shop_id)?;
    read_shop(&shops, &shop, &who, |conn| {
        Ok(ShopSaleDetail {
            shop_id: shop.id.clone(),
            shop_name: shop.name.clone(),
            detail: load_sale(conn, sale_id)?,
            store: ReceiptStore {
                name: setting(conn, "store_name").unwrap_or_else(|| shop.name.clone()),
                address: setting(conn, "store_address"),
                phone: setting(conn, "store_phone"),
                footer: setting(conn, "receipt_footer"),
            },
        })
    })
    .map_err(|e| crate::error::bad(e.message(&shop.name)))
}

/// Ask the background sync to bring every shop up to date now.
#[tauri::command]
pub fn refresh_all_shops(session: State<Session>, sync: State<Sync>) -> AppResult<()> {
    require_owner(&session)?;
    sync.kick_all();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, qty: i64, reorder: i64, price: i64) -> ProductStock {
        ProductStock { name: name.into(), category: Some("Beers".into()), qty, reorder, price }
    }

    #[test]
    fn stock_is_matched_by_name_across_shops() {
        let kimbo = vec![p("TUSKER LAGER CAN", 11, 5, 30000), p("Kimbo Only", 3, 5, 10000), p("BALOZI CAN", 2, 5, 30000)];
        let vintage = vec![p("tusker  lager can", 22, 5, 32000), p("BALOZI CAN", 20, 5, 30000), p("BALOZI CAN", 9, 5, 30000)];
        let rows = merge_stock(&[kimbo, vintage]);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["BALOZI CAN", "Kimbo Only", "TUSKER LAGER CAN"], "sorted by name, one row per product");

        let tusker = &rows[2];
        assert_eq!(tusker.cells[0].as_ref().unwrap().qty, 11);
        assert_eq!(tusker.cells[1].as_ref().unwrap().qty, 22, "case and spacing differences still match");
        assert_eq!(tusker.total, 33);
        assert!(tusker.price_differs);

        let kimbo_only = &rows[1];
        assert!(kimbo_only.cells[1].is_none(), "a product one shop does not stock has no cell there");
        assert!(kimbo_only.cells[0].as_ref().unwrap().low);
        assert!(!kimbo_only.price_differs);

        let balozi = &rows[0];
        assert_eq!(balozi.cells[1].as_ref().unwrap().qty, 29, "a name repeated inside a shop is added up");
        assert!(balozi.cells[0].as_ref().unwrap().low && !balozi.cells[1].as_ref().unwrap().low);
        assert_eq!(balozi.total, 31);
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("liquorpos-overview-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Two shops on one computer, each with its own database, read side by side.
    #[test]
    fn every_shop_is_read_from_its_own_database() {
        let dir = temp_dir("two-shops");
        let shops = Shops::load(&dir).unwrap();
        let second = shops.add("Second shop").unwrap();
        let first = shops.current();

        let fill = |shop: &Shop, stock: i64, total: i64, extra_owner: bool| {
            let conn = db::open_shop(&shops.path_of(shop)).unwrap();
            conn.execute_batch(&format!(
                "INSERT INTO products (name, sell_price, stock_qty) VALUES ('Shared product', 30000, {stock});
                 INSERT INTO sales (user_id, subtotal, total, payment_method, created_at) VALUES (1, {total}, {total}, 'cash', '2026-03-05 10:00:00');
                 INSERT INTO sale_items (sale_id, product_id, product_name, qty, unit_price, unit_cost, line_total)
                   VALUES (1, 1, 'Shared product', 2, {total} / 2, 0, {total});"
            )).unwrap();
            if !extra_owner {
                conn.execute("UPDATE users SET role = 'cashier' WHERE username = 'admin'", []).unwrap();
            }
        };
        fill(&first, 4, 60_000, true);
        fill(&second, 40, 90_000, true);

        let viewer = |username: &str, unlocked: &[&str], only: Option<&str>| Viewer {
            username: username.into(),
            unlocked: unlocked.iter().map(|s| s.to_string()).collect(),
            only: only.map(str::to_string),
        };
        let both = [first.id.as_str(), second.id.as_str()];
        let admin = viewer("admin", &both, None);

        let o = build_overview(&shops, &viewer("ADMIN", &both, None), "2026-03-05", "2026-03-05", |_, _| (false, None));
        let nets: Vec<i64> = o.shops.iter().map(|s| s.figures.as_ref().unwrap().summary.net).collect();
        assert_eq!(nets, [60_000, 90_000], "each shop reports its own sales; usernames match without case");
        assert_eq!(o.shops.iter().filter(|s| s.is_open).count(), 1);
        assert_eq!(o.best_sellers[0].per_shop, vec![2, 2]);
        assert_eq!(o.best_sellers[0].revenue, 150_000);

        let m = build_stock(&shops, &admin);
        assert_eq!(m.rows.len(), 1);
        assert_eq!(m.rows[0].total, 44);
        assert!(m.rows[0].cells[0].as_ref().unwrap().low && !m.rows[0].cells[1].as_ref().unwrap().low);

        let sales = build_sales(&shops, &admin, "2026-03-05", "2026-03-05", None, 10);
        assert_eq!(sales.len(), 2);

        // Filtering by shop narrows every view to that shop.
        let only_second = viewer("admin", &both, Some(&second.id));
        assert_eq!(build_sales(&shops, &only_second, "2026-03-05", "2026-03-05", None, 10).len(), 1);
        let o = build_overview(&shops, &only_second, "2026-03-05", "2026-03-05", |_, _| (false, None));
        assert_eq!(o.shops.len(), 1);
        assert_eq!(o.best_sellers[0].per_shop, vec![2]);
        assert_eq!(build_stock(&shops, &only_second).rows[0].total, 40);

        // A shop the login did not unlock stays locked, even though the username is an owner there.
        let o = build_overview(&shops, &viewer("admin", &[&first.id], None), "2026-03-05", "2026-03-05", |_, _| (false, None));
        assert!(o.shops[1].locked && o.shops[1].figures.is_none() && o.shops[0].figures.is_some());

        // Someone who is no longer an owner in the second shop sees it locked too.
        db::open_shop(&shops.path_of(&second)).unwrap().execute("UPDATE users SET role = 'cashier'", []).unwrap();
        let o = build_overview(&shops, &admin, "2026-03-05", "2026-03-05", |_, _| (false, None));
        assert!(o.shops[1].locked && o.shops[1].figures.is_none() && o.shops[0].figures.is_some());
        assert_eq!(build_sales(&shops, &admin, "2026-03-05", "2026-03-05", None, 10).len(), 1);

        // Reading never writes: no audit rows, nothing queued beyond what each shop already had.
        let conn = db::open_existing(&shops.path_of(&first)).unwrap();
        let audit: i64 = conn.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0)).unwrap();
        assert_eq!(audit, 0);
    }

    /// Optional: point LIQUORPOS_APPDATA_COPY at a COPY of the app data folder to see real figures.
    #[test]
    fn real_shops_report() {
        let Some(dir) = std::env::var_os("LIQUORPOS_APPDATA_COPY").map(std::path::PathBuf::from) else { return };
        let shops = Shops::load(&dir).unwrap();
        let admin = Viewer { username: "admin".into(), unlocked: shops.all().into_iter().map(|s| s.id).collect(), only: None };
        let (from, to) = (std::env::var("FROM").unwrap_or("2026-09-01".into()), std::env::var("TO").unwrap_or("2026-09-30".into()));
        let o = build_overview(&shops, &admin, &from, &to, |_, _| (false, None));
        for s in &o.shops {
            match &s.figures {
                Some(f) => eprintln!(
                    "REPORT {}: sales {} net {} cash {} mpesa {} items {} | products {} units {} retail {} low {} | last sale {:?} synced {:?} tills {:?}",
                    s.name, f.summary.sales_count, f.summary.net, f.summary.cash_total, f.summary.mpesa_total, f.summary.items_sold,
                    f.stock.product_count, f.stock.units, f.stock.retail_value, f.stock.low_stock_count, f.last_sale_at, f.last_synced,
                    f.tills.iter().map(|t| t.name.as_str()).collect::<Vec<_>>()
                ),
                None => eprintln!("REPORT {}: unreadable: {:?}", s.name, s.error),
            }
        }
        for b in o.best_sellers.iter().take(5) {
            eprintln!("REPORT best seller {} qty {} revenue {} per shop {:?}", b.name, b.qty, b.revenue, b.per_shop);
        }
        let m = build_stock(&shops, &admin);
        let only_one = m.rows.iter().filter(|r| r.cells.iter().flatten().count() == 1).count();
        let differs = m.rows.iter().filter(|r| r.price_differs).count();
        eprintln!("REPORT stock rows {} (in one shop only {}, price differs {})", m.rows.len(), only_one, differs);
        let sales = build_sales(&shops, &admin, &from, &to, None, 5);
        for s in &sales {
            eprintln!("REPORT sale {} #{} {} {} {:?}", s.shop_name, s.sale.receipt_no, s.sale.created_at, s.sale.total, s.sale.till);
        }
        assert!(o.shops.iter().all(|s| s.figures.is_some()), "every shop should be readable by admin");
    }

    #[test]
    fn best_sellers_rank_by_combined_revenue() {
        let kimbo = vec![("TUSKER LAGER CAN".to_string(), 10, 300_000), ("KONYAGI 250ML".to_string(), 2, 70_000)];
        let vintage = vec![("Tusker Lager Can".to_string(), 1, 30_000), ("KIBAO VODKA 250ML".to_string(), 12, 420_000)];
        let rows = merge_best_sellers(&[kimbo, vintage, Vec::new()], 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "KIBAO VODKA 250ML");
        assert_eq!(rows[0].per_shop, vec![0, 12, 0]);
        assert_eq!(rows[1].name, "TUSKER LAGER CAN");
        assert_eq!((rows[1].qty, rows[1].revenue), (11, 330_000));
        assert_eq!(rows[1].per_shop, vec![10, 1, 0]);
    }
}
