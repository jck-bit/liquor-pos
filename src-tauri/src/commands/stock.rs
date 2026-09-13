use rusqlite::params;
use tauri::State;

use crate::commands::audit;
use crate::commands::auth::{require_owner, require_user, Session};
use crate::sync::Sync;
use crate::commands::products::{row_to_product, PRODUCT_COLS};
use crate::db::Db;
use crate::error::{bad, AppResult};
use crate::models::{Product, ReceiveLine, StockMovement};

/// Record a delivery. Adds to stock and, when a unit cost is given, updates the
/// product's cost price so profit reports use the latest buying price.
#[tauri::command]
pub fn receive_stock(
    db: State<Db>,
    session: State<Session>,
    sync: State<Sync>,
    lines: Vec<ReceiveLine>,
    note: Option<String>,
) -> AppResult<()> {
    let user = require_user(&session)?;
    if lines.is_empty() {
        return Err(bad("Add at least one product"));
    }
    let mut conn = db.lock();
    let tx = conn.transaction()?;
    {
        let mut upd = tx.prepare_cached(
            "UPDATE products SET stock_qty = stock_qty + ?1, cost_price = COALESCE(?2, cost_price),
                updated_at = datetime('now', 'localtime') WHERE id = ?3",
        )?;
        let mut mv = tx.prepare_cached(
            "INSERT INTO stock_movements (product_id, qty_delta, reason, note, user_id) VALUES (?1, ?2, 'purchase', ?3, ?4)",
        )?;
        for line in &lines {
            if line.qty <= 0 {
                return Err(bad("Quantity must be at least 1"));
            }
            if upd.execute(params![line.qty, line.unit_cost, line.product_id])? == 0 {
                return Err(bad("Product not found"));
            }
            mv.execute(params![line.product_id, line.qty, note, user.id])?;
        }
    }
    let total_units: i64 = lines.iter().map(|l| l.qty).sum();
    audit::log(
        &tx,
        Some(user.id),
        "receive_stock",
        "stock",
        None,
        serde_json::json!({ "lines": lines.len(), "units": total_units, "note": note }),
    )?;
    tx.commit()?;
    drop(conn);
    sync.kick();
    Ok(())
}

/// Manual correction by the owner: stock count, damage, or other adjustment.
#[tauri::command]
pub fn adjust_stock(
    db: State<Db>,
    session: State<Session>,
    sync: State<Sync>,
    product_id: i64,
    qty_delta: i64,
    reason: String,
    note: Option<String>,
) -> AppResult<Product> {
    let owner = require_owner(&session)?;
    if !matches!(reason.as_str(), "adjustment" | "damage" | "count") {
        return Err(bad("Invalid reason"));
    }
    if qty_delta == 0 {
        return Err(bad("Change cannot be zero"));
    }
    let mut conn = db.lock();
    let tx = conn.transaction()?;
    let before: i64 = tx
        .query_row("SELECT stock_qty FROM products WHERE id = ?1", params![product_id], |r| r.get(0))
        .map_err(|_| bad("Product not found"))?;
    tx.execute(
        "UPDATE products SET stock_qty = stock_qty + ?1, updated_at = datetime('now', 'localtime') WHERE id = ?2",
        params![qty_delta, product_id],
    )?;
    tx.execute(
        "INSERT INTO stock_movements (product_id, qty_delta, reason, note, user_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![product_id, qty_delta, reason, note, owner.id],
    )?;
    audit::log(
        &tx,
        Some(owner.id),
        "adjust_stock",
        "product",
        Some(product_id),
        serde_json::json!({ "reason": reason, "delta": qty_delta, "before": before, "after": before + qty_delta, "note": note }),
    )?;
    let sql = format!("SELECT {PRODUCT_COLS} FROM products WHERE id = ?1");
    let product = tx.query_row(&sql, params![product_id], row_to_product)?;
    tx.commit()?;
    drop(conn);
    sync.kick();
    Ok(product)
}

#[tauri::command]
pub fn list_stock_movements(
    db: State<Db>,
    session: State<Session>,
    product_id: Option<i64>,
    limit: Option<i64>,
) -> AppResult<Vec<StockMovement>> {
    require_user(&session)?;
    let conn = db.lock();
    let mut stmt = conn.prepare_cached(
        "SELECT m.id, m.product_id, p.name, m.qty_delta, m.reason, m.ref_sale_id, m.note, u.username, m.created_at
         FROM stock_movements m
         JOIN products p ON p.id = m.product_id
         JOIN users u ON u.id = m.user_id
         WHERE (?1 IS NULL OR m.product_id = ?1)
         ORDER BY m.id DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![product_id, limit.unwrap_or(200)], |r| {
        Ok(StockMovement {
            id: r.get(0)?,
            product_id: r.get(1)?,
            product_name: r.get(2)?,
            qty_delta: r.get(3)?,
            reason: r.get(4)?,
            ref_sale_id: r.get(5)?,
            note: r.get(6)?,
            user: r.get(7)?,
            created_at: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[tauri::command]
pub fn low_stock_products(db: State<Db>, session: State<Session>) -> AppResult<Vec<Product>> {
    require_user(&session)?;
    let conn = db.lock();
    let sql = format!(
        "SELECT {PRODUCT_COLS} FROM products WHERE active = 1 AND stock_qty <= reorder_level ORDER BY stock_qty ASC, name"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map([], row_to_product)?;
    Ok(rows.collect::<Result<_, _>>()?)
}
