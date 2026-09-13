use rusqlite::{params, Connection, OptionalExtension};
use tauri::State;

use crate::commands::audit;
use crate::commands::auth::{require_owner, require_user, Session};
use crate::sync::Sync;
use crate::db::Db;
use crate::error::{bad, AppResult};
use crate::models::{Sale, SaleDetail, SaleInput, SaleItem};

const SALE_SELECT: &str = "SELECT s.id, u.username, s.subtotal, s.discount, s.total, s.payment_method, s.mpesa_code,
        s.cash_tendered, s.change_given, s.status, s.void_reason,
        (SELECT COALESCE(SUM(qty), 0) FROM sale_items WHERE sale_id = s.id), s.created_at
     FROM sales s JOIN users u ON u.id = s.user_id";

fn row_to_sale(r: &rusqlite::Row) -> rusqlite::Result<Sale> {
    Ok(Sale {
        id: r.get(0)?,
        cashier: r.get(1)?,
        subtotal: r.get(2)?,
        discount: r.get(3)?,
        total: r.get(4)?,
        payment_method: r.get(5)?,
        mpesa_code: r.get(6)?,
        cash_tendered: r.get(7)?,
        change_given: r.get(8)?,
        status: r.get(9)?,
        void_reason: r.get(10)?,
        item_count: r.get(11)?,
        created_at: r.get(12)?,
    })
}

fn load_sale(conn: &Connection, id: i64) -> AppResult<SaleDetail> {
    let sql = format!("{SALE_SELECT} WHERE s.id = ?1");
    let sale = conn
        .query_row(&sql, params![id], row_to_sale)
        .optional()?
        .ok_or_else(|| bad("Sale not found"))?;
    let mut stmt = conn.prepare_cached(
        "SELECT id, product_id, product_name, qty, unit_price, line_total FROM sale_items WHERE sale_id = ?1 ORDER BY id",
    )?;
    let items = stmt
        .query_map(params![id], |r| {
            Ok(SaleItem {
                id: r.get(0)?,
                product_id: r.get(1)?,
                product_name: r.get(2)?,
                qty: r.get(3)?,
                unit_price: r.get(4)?,
                line_total: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(SaleDetail { sale, items })
}

/// Complete a sale. Everything (sale, lines, stock deduction, movement log,
/// audit) happens in one transaction: either all of it is saved or none of it.
#[tauri::command]
pub fn create_sale(db: State<Db>, session: State<Session>, sync: State<Sync>, input: SaleInput) -> AppResult<SaleDetail> {
    let user = require_user(&session)?;
    if input.items.is_empty() {
        return Err(bad("The cart is empty"));
    }
    if input.discount < 0 {
        return Err(bad("Discount cannot be negative"));
    }

    let mut conn = db.lock();
    let tx = conn.transaction()?;

    let allow_negative: String = tx
        .query_row("SELECT value FROM settings WHERE key = 'allow_negative_stock'", [], |r| r.get(0))
        .unwrap_or_else(|_| "0".into());
    let allow_negative = allow_negative == "1";

    // Validate every line and snapshot product name/cost before touching anything.
    struct Line {
        product_id: i64,
        name: String,
        qty: i64,
        unit_price: i64,
        unit_cost: i64,
    }
    let mut lines: Vec<Line> = Vec::with_capacity(input.items.len());
    let mut subtotal = 0i64;
    {
        let mut get = tx.prepare_cached(
            "SELECT name, cost_price, stock_qty, active FROM products WHERE id = ?1",
        )?;
        for item in &input.items {
            if item.qty <= 0 {
                return Err(bad("Quantity must be at least 1"));
            }
            if item.unit_price < 0 {
                return Err(bad("Price cannot be negative"));
            }
            let (name, cost, stock, active): (String, i64, i64, i64) = get
                .query_row(params![item.product_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .optional()?
                .ok_or_else(|| bad("A product in the cart no longer exists"))?;
            if active == 0 {
                return Err(bad(format!("{name} is no longer on sale")));
            }
            if !allow_negative && stock < item.qty {
                return Err(bad(format!("Not enough stock for {name}: only {stock} left")));
            }
            subtotal += item.qty * item.unit_price;
            lines.push(Line { product_id: item.product_id, name, qty: item.qty, unit_price: item.unit_price, unit_cost: cost });
        }
    }

    let total = subtotal - input.discount;
    if total < 0 {
        return Err(bad("Discount is larger than the sale"));
    }

    let (mpesa_code, cash_tendered, change_given) = match input.payment_method.as_str() {
        "cash" => {
            let tendered = input.cash_tendered.unwrap_or(total);
            if tendered < total {
                return Err(bad("Cash given is less than the total"));
            }
            (None, Some(tendered), Some(tendered - total))
        }
        "mpesa" => {
            let code = input.mpesa_code.unwrap_or_default().trim().to_uppercase();
            if code.len() < 8 || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err(bad("Enter the M-Pesa transaction code (e.g. QGH7XK2M9P)"));
            }
            let used: Option<i64> = tx
                .query_row("SELECT id FROM sales WHERE mpesa_code = ?1", params![code], |r| r.get(0))
                .optional()?;
            if let Some(sale_id) = used {
                return Err(bad(format!("M-Pesa code {code} was already used on sale #{sale_id}")));
            }
            (Some(code), None, None)
        }
        _ => return Err(bad("Payment method must be cash or mpesa")),
    };

    tx.execute(
        "INSERT INTO sales (user_id, subtotal, discount, total, payment_method, mpesa_code, cash_tendered, change_given)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![user.id, subtotal, input.discount, total, input.payment_method, mpesa_code, cash_tendered, change_given],
    )?;
    let sale_id = tx.last_insert_rowid();

    {
        let mut ins = tx.prepare_cached(
            "INSERT INTO sale_items (sale_id, product_id, product_name, qty, unit_price, unit_cost, line_total)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        let mut upd = tx.prepare_cached(
            "UPDATE products SET stock_qty = stock_qty - ?1, updated_at = datetime('now', 'localtime') WHERE id = ?2",
        )?;
        let mut mv = tx.prepare_cached(
            "INSERT INTO stock_movements (product_id, qty_delta, reason, ref_sale_id, user_id) VALUES (?1, ?2, 'sale', ?3, ?4)",
        )?;
        for l in &lines {
            ins.execute(params![sale_id, l.product_id, l.name, l.qty, l.unit_price, l.unit_cost, l.qty * l.unit_price])?;
            upd.execute(params![l.qty, l.product_id])?;
            mv.execute(params![l.product_id, -l.qty, sale_id, user.id])?;
        }
    }

    audit::log(
        &tx,
        Some(user.id),
        "sale",
        "sale",
        Some(sale_id),
        serde_json::json!({ "total": total, "discount": input.discount, "method": input.payment_method, "items": lines.len() }),
    )?;

    let detail = load_sale(&tx, sale_id)?;
    tx.commit()?;
    drop(conn);
    sync.kick();
    Ok(detail)
}

#[tauri::command]
pub fn get_sale(db: State<Db>, session: State<Session>, sale_id: i64) -> AppResult<SaleDetail> {
    require_user(&session)?;
    let conn = db.lock();
    load_sale(&conn, sale_id)
}

#[tauri::command]
pub fn list_sales(
    db: State<Db>,
    session: State<Session>,
    from: String,
    to: String,
    payment_method: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<Sale>> {
    require_user(&session)?;
    let conn = db.lock();
    let sql = format!(
        "{SALE_SELECT} WHERE s.created_at >= ?1 AND s.created_at < date(?2, '+1 day')
           AND (?3 IS NULL OR s.payment_method = ?3)
         ORDER BY s.id DESC LIMIT ?4"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(params![from, to, payment_method, limit.unwrap_or(500)], row_to_sale)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Owner-only. Marks the sale voided and puts every item back into stock.
/// The original sale row is kept so the audit trail stays complete.
#[tauri::command]
pub fn void_sale(db: State<Db>, session: State<Session>, sync: State<Sync>, sale_id: i64, reason: String) -> AppResult<SaleDetail> {
    let owner = require_owner(&session)?;
    let reason = reason.trim().to_string();
    if reason.is_empty() {
        return Err(bad("A reason is required to void a sale"));
    }
    let mut conn = db.lock();
    let tx = conn.transaction()?;

    let status: String = tx
        .query_row("SELECT status FROM sales WHERE id = ?1", params![sale_id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| bad("Sale not found"))?;
    if status == "voided" {
        return Err(bad("This sale is already voided"));
    }

    tx.execute(
        "UPDATE sales SET status = 'voided', voided_by = ?1, voided_at = datetime('now', 'localtime'), void_reason = ?2 WHERE id = ?3",
        params![owner.id, reason, sale_id],
    )?;

    let items: Vec<(i64, i64)> = tx
        .prepare_cached("SELECT product_id, qty FROM sale_items WHERE sale_id = ?1")?
        .query_map(params![sale_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    {
        let mut upd = tx.prepare_cached(
            "UPDATE products SET stock_qty = stock_qty + ?1, updated_at = datetime('now', 'localtime') WHERE id = ?2",
        )?;
        let mut mv = tx.prepare_cached(
            "INSERT INTO stock_movements (product_id, qty_delta, reason, ref_sale_id, note, user_id) VALUES (?1, ?2, 'void', ?3, ?4, ?5)",
        )?;
        for (product_id, qty) in &items {
            upd.execute(params![qty, product_id])?;
            mv.execute(params![product_id, qty, sale_id, reason, owner.id])?;
        }
    }

    audit::log(&tx, Some(owner.id), "void", "sale", Some(sale_id), serde_json::json!({ "reason": reason }))?;
    let detail = load_sale(&tx, sale_id)?;
    tx.commit()?;
    drop(conn);
    sync.kick();
    Ok(detail)
}
