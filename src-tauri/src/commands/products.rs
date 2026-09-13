use rusqlite::{params, OptionalExtension, Row};
use tauri::State;

use crate::commands::audit;
use crate::commands::auth::{require_owner, require_user, Session};
use crate::db::Db;
use crate::error::{bad, AppResult};
use crate::models::{Product, ProductInput};

pub const PRODUCT_COLS: &str =
    "id, barcode, name, category, cost_price, sell_price, stock_qty, reorder_level, active";

pub fn row_to_product(r: &Row) -> rusqlite::Result<Product> {
    Ok(Product {
        id: r.get(0)?,
        barcode: r.get(1)?,
        name: r.get(2)?,
        category: r.get(3)?,
        cost_price: r.get(4)?,
        sell_price: r.get(5)?,
        stock_qty: r.get(6)?,
        reorder_level: r.get(7)?,
        active: r.get::<_, i64>(8)? != 0,
    })
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

#[tauri::command]
pub fn list_products(
    db: State<Db>,
    session: State<Session>,
    query: Option<String>,
    include_inactive: Option<bool>,
) -> AppResult<Vec<Product>> {
    require_user(&session)?;
    let q = query.unwrap_or_default().trim().to_string();
    let like = format!("%{q}%");
    let conn = db.lock();
    let sql = format!(
        "SELECT {PRODUCT_COLS} FROM products
         WHERE (?1 = 1 OR active = 1)
           AND (?2 = '' OR name LIKE ?3 OR barcode = ?2 OR category LIKE ?3)
         ORDER BY name COLLATE NOCASE LIMIT 500"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(params![include_inactive.unwrap_or(false) as i64, q, like], row_to_product)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Exact barcode lookup, used by the scanner on the sell screen.
#[tauri::command]
pub fn find_product_by_barcode(db: State<Db>, session: State<Session>, barcode: String) -> AppResult<Option<Product>> {
    require_user(&session)?;
    let conn = db.lock();
    let sql = format!("SELECT {PRODUCT_COLS} FROM products WHERE barcode = ?1 AND active = 1");
    let mut stmt = conn.prepare_cached(&sql)?;
    let product = stmt.query_row(params![barcode.trim()], row_to_product).optional()?;
    Ok(product)
}

#[tauri::command]
pub fn list_categories(db: State<Db>, session: State<Session>) -> AppResult<Vec<String>> {
    require_user(&session)?;
    let conn = db.lock();
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT category FROM products WHERE category IS NOT NULL AND category != '' ORDER BY category",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[tauri::command]
pub fn save_product(db: State<Db>, session: State<Session>, input: ProductInput) -> AppResult<Product> {
    let owner = require_owner(&session)?;
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(bad("Product name is required"));
    }
    if input.sell_price < 0 || input.cost_price < 0 {
        return Err(bad("Prices cannot be negative"));
    }
    let barcode = clean(input.barcode);
    let category = clean(input.category);

    let mut conn = db.lock();
    let tx = conn.transaction()?;

    if let Some(code) = &barcode {
        let clash: Option<i64> = tx
            .query_row(
                "SELECT id FROM products WHERE barcode = ?1 AND id != ?2",
                params![code, input.id.unwrap_or(0)],
                |r| r.get(0),
            )
            .optional()?;
        if clash.is_some() {
            return Err(bad(format!("Barcode {code} is already used by another product")));
        }
    }

    let id = match input.id {
        Some(id) => {
            let sql = format!("SELECT {PRODUCT_COLS} FROM products WHERE id = ?1");
            let old = tx
                .query_row(&sql, params![id], row_to_product)
                .optional()?
                .ok_or_else(|| bad("Product not found"))?;
            tx.execute(
                "UPDATE products SET barcode = ?1, name = ?2, category = ?3, cost_price = ?4, sell_price = ?5,
                    reorder_level = ?6, updated_at = datetime('now', 'localtime') WHERE id = ?7",
                params![barcode, name, category, input.cost_price, input.sell_price, input.reorder_level, id],
            )?;
            let mut changes = serde_json::Map::new();
            if old.sell_price != input.sell_price {
                changes.insert("sellPrice".into(), serde_json::json!([old.sell_price, input.sell_price]));
            }
            if old.cost_price != input.cost_price {
                changes.insert("costPrice".into(), serde_json::json!([old.cost_price, input.cost_price]));
            }
            if old.name != name {
                changes.insert("name".into(), serde_json::json!([old.name, name]));
            }
            if old.barcode != barcode {
                changes.insert("barcode".into(), serde_json::json!([old.barcode, barcode]));
            }
            audit::log(&tx, Some(owner.id), "update", "product", Some(id), serde_json::Value::Object(changes))?;
            id
        }
        None => {
            tx.execute(
                "INSERT INTO products (barcode, name, category, cost_price, sell_price, reorder_level)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![barcode, name, category, input.cost_price, input.sell_price, input.reorder_level],
            )?;
            let id = tx.last_insert_rowid();
            audit::log(
                &tx,
                Some(owner.id),
                "create",
                "product",
                Some(id),
                serde_json::json!({ "name": name, "sellPrice": input.sell_price, "costPrice": input.cost_price }),
            )?;
            id
        }
    };

    let sql = format!("SELECT {PRODUCT_COLS} FROM products WHERE id = ?1");
    let product = tx.query_row(&sql, params![id], row_to_product)?;
    tx.commit()?;
    Ok(product)
}

#[tauri::command]
pub fn set_product_active(db: State<Db>, session: State<Session>, product_id: i64, active: bool) -> AppResult<()> {
    let owner = require_owner(&session)?;
    let conn = db.lock();
    conn.execute(
        "UPDATE products SET active = ?1, updated_at = datetime('now', 'localtime') WHERE id = ?2",
        params![active as i64, product_id],
    )?;
    audit::log(&conn, Some(owner.id), if active { "activate" } else { "deactivate" }, "product", Some(product_id), serde_json::json!({}))?;
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRow {
    pub name: String,
    pub barcode: Option<String>,
    pub category: Option<String>,
    pub sell_price: Option<i64>,
    pub cost_price: Option<i64>,
    pub stock_qty: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub created: i64,
    pub updated: i64,
    pub skipped: i64,
}

/// Bulk import from a spreadsheet. Matches existing products by barcode, then by
/// name (case-insensitive). Existing rows get their prices/category updated and
/// their stock set to the sheet's count; new rows are created with opening stock.
/// One transaction: either the whole sheet goes in or none of it.
#[tauri::command]
pub fn import_products(db: State<Db>, session: State<Session>, rows: Vec<ImportRow>) -> AppResult<ImportResult> {
    let owner = require_owner(&session)?;
    let mut conn = db.lock();
    let tx = conn.transaction()?;
    let mut result = ImportResult { created: 0, updated: 0, skipped: 0 };

    for row in rows {
        let name = row.name.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            result.skipped += 1;
            continue;
        }
        let barcode = clean(row.barcode);
        let category = clean(row.category);

        let existing: Option<(i64, i64)> = match &barcode {
            Some(code) => tx
                .query_row("SELECT id, stock_qty FROM products WHERE barcode = ?1", params![code], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?,
            None => None,
        }
        .or(tx
            .query_row("SELECT id, stock_qty FROM products WHERE name = ?1 COLLATE NOCASE", params![name], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?);

        match existing {
            Some((id, current_stock)) => {
                tx.execute(
                    "UPDATE products SET
                        category = COALESCE(?1, category),
                        sell_price = COALESCE(?2, sell_price),
                        cost_price = COALESCE(?3, cost_price),
                        barcode = COALESCE(?4, barcode),
                        active = 1,
                        updated_at = datetime('now', 'localtime')
                     WHERE id = ?5",
                    params![category, row.sell_price, row.cost_price, barcode, id],
                )?;
                if let Some(target) = row.stock_qty {
                    let delta = target - current_stock;
                    if delta != 0 {
                        tx.execute(
                            "UPDATE products SET stock_qty = ?1 WHERE id = ?2",
                            params![target, id],
                        )?;
                        tx.execute(
                            "INSERT INTO stock_movements (product_id, qty_delta, reason, note, user_id) VALUES (?1, ?2, 'count', 'Set by import', ?3)",
                            params![id, delta, owner.id],
                        )?;
                    }
                }
                result.updated += 1;
            }
            None => {
                let stock = row.stock_qty.unwrap_or(0).max(0);
                tx.execute(
                    "INSERT INTO products (barcode, name, category, cost_price, sell_price, stock_qty)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![barcode, name, category, row.cost_price.unwrap_or(0), row.sell_price.unwrap_or(0), stock],
                )?;
                let id = tx.last_insert_rowid();
                if stock > 0 {
                    tx.execute(
                        "INSERT INTO stock_movements (product_id, qty_delta, reason, note, user_id) VALUES (?1, ?2, 'purchase', 'Opening stock from import', ?3)",
                        params![id, stock, owner.id],
                    )?;
                }
                result.created += 1;
            }
        }
    }

    audit::log(
        &tx,
        Some(owner.id),
        "import",
        "product",
        None,
        serde_json::json!({ "created": result.created, "updated": result.updated, "skipped": result.skipped }),
    )?;
    tx.commit()?;
    Ok(result)
}
