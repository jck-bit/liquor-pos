//! Pull side of store-wide sync.
//!
//! Every computer logged into the same store receives what the other computers
//! wrote: users, product edits, sales, stock movements and audit entries. Rows
//! are matched by `uid`, so applying the same row twice changes nothing.
//!
//! Stock is never copied as a number. Each computer rebuilds it from the shared
//! movement history (`recompute_stock`), so every computer arrives at the same
//! figure no matter in which order the rows reached it.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use super::supabase::{Client, Tokens};
use super::{set_setting, setting, urlencode, PassDb};

const PAGE: usize = 1000;
const EPOCH: &str = "1970-01-01T00:00:00Z";
const REASONS: &[&str] = &["sale", "void", "purchase", "adjustment", "damage", "count"];

/// Pulled in this order so each table can find the rows it refers to.
pub const TABLES: &[&str] = &["users", "products", "sales", "sale_items", "stock_movements", "audit_log"];

/// Tables whose rows never change after they are written, so a computer never
/// needs its own rows from them back.
pub fn append_only(table: &str) -> bool {
    matches!(table, "sale_items" | "stock_movements" | "audit_log")
}

type R<T> = Result<T, String>;

fn err(e: rusqlite::Error) -> String {
    format!("Local database: {e}")
}

#[derive(Default)]
pub struct Applied {
    /// `synced_at` of the first row whose dependencies have not arrived yet.
    pub hold: Option<String>,
    /// Products whose stock must be rebuilt.
    pub touched: BTreeSet<i64>,
}

pub fn pull_all(db: &PassDb, client: &Client, tokens: &Tokens, device_id: &str) -> R<()> {
    pull_devices(db, client, tokens)?;
    for table in TABLES {
        pull_table(db, client, tokens, device_id, table)?;
    }
    rebuild_once(db)
}

/// First pull after upgrading: rebuild every product's stock once, so it follows
/// the shared history from here on.
fn rebuild_once(db: &PassDb) -> R<()> {
    let mut conn = db.lock()?;
    if setting(&conn, "stock_rebuilt_v2").is_some() {
        return Ok(());
    }
    let tx = conn.transaction().map_err(err)?;
    set_setting(&tx, "sync_pulling", "1").map_err(err)?;
    let all: BTreeSet<i64> = {
        let mut stmt = tx.prepare("SELECT DISTINCT product_id FROM stock_movements").map_err(err)?;
        let ids = stmt.query_map([], |r| r.get(0)).map_err(err)?;
        ids.collect::<Result<_, _>>().map_err(err)?
    };
    recompute_stock(&tx, &all)?;
    set_setting(&tx, "sync_pulling", "0").map_err(err)?;
    set_setting(&tx, "stock_rebuilt_v2", "1").map_err(err)?;
    tx.commit().map_err(err)
}

fn pull_devices(db: &PassDb, client: &Client, tokens: &Tokens) -> R<()> {
    let rows = client.select(
        &tokens.access,
        &format!("devices?select=device_id,name,last_seen&store_id=eq.{}", tokens.user_id),
    )?;
    let conn = db.lock()?;
    let mut stmt = conn
        .prepare_cached(
            "INSERT INTO devices (device_id, name, last_seen) VALUES (?1, ?2, ?3)
             ON CONFLICT(device_id) DO UPDATE SET name = excluded.name, last_seen = excluded.last_seen",
        )
        .map_err(err)?;
    for r in &rows {
        if let Some(id) = text(r, "device_id") {
            stmt.execute(params![id, text(r, "name"), stamp(text(r, "last_seen"))]).map_err(err)?;
        }
    }
    Ok(())
}

fn pull_table(db: &PassDb, client: &Client, tokens: &Tokens, device_id: &str, table: &str) -> R<()> {
    let key = format!("pull_cursor:{table}");
    // Start two minutes before the cursor: a push that was still committing
    // during the last read can carry an earlier synced_at than rows already seen.
    let lower = {
        let conn = db.lock()?;
        setting(&conn, &key)
            .and_then(|c| {
                conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', ?1, '-2 minutes')", params![c], |r| {
                    r.get::<_, Option<String>>(0)
                })
                .ok()
                .flatten()
            })
            .unwrap_or_else(|| EPOCH.to_string())
    };
    let own = if append_only(table) { format!("&device_id=neq.{device_id}") } else { String::new() };

    let mut hold: Option<String> = None;
    let mut offset = 0;
    loop {
        let query = format!(
            "{table}?select=*&store_id=eq.{store}&synced_at=gte.{lower}{own}&order=synced_at.asc,uid.asc&limit={PAGE}&offset={offset}",
            store = tokens.user_id,
            lower = urlencode(&lower),
        );
        let page = client.select(&tokens.access, &query)?;
        if page.is_empty() {
            break;
        }
        {
            let mut conn = db.lock()?;
            let tx = conn.transaction().map_err(err)?;
            set_setting(&tx, "sync_pulling", "1").map_err(err)?;
            let applied = apply(&tx, table, &page)?;
            recompute_stock(&tx, &applied.touched)?;
            set_setting(&tx, "sync_pulling", "0").map_err(err)?;
            if hold.is_none() {
                hold = applied.hold;
            }
            // Never move the cursor past a row that is still waiting on another row.
            if let Some(cursor) = hold.clone().or_else(|| page.last().and_then(|r| text(r, "synced_at"))) {
                set_setting(&tx, &key, &cursor).map_err(err)?;
            }
            tx.commit().map_err(err)?;
        }
        if page.len() < PAGE {
            break;
        }
        offset += PAGE;
    }
    Ok(())
}

// ---------- applying rows ----------

pub fn apply(conn: &Connection, table: &str, rows: &[Value]) -> R<Applied> {
    let mut out = Applied::default();
    for row in rows {
        let done = match table {
            "users" => apply_user(conn, row)?,
            "products" => apply_product(conn, row)?,
            "sales" => apply_sale(conn, row)?,
            "sale_items" => apply_sale_item(conn, row)?,
            "stock_movements" => apply_movement(conn, row, &mut out.touched)?,
            "audit_log" => apply_audit(conn, row)?,
            _ => true,
        };
        if !done && out.hold.is_none() {
            out.hold = Some(text(row, "synced_at").unwrap_or_else(|| EPOCH.to_string()));
        }
    }
    Ok(out)
}

fn apply_user(conn: &Connection, r: &Value) -> R<bool> {
    let Some(uid) = text(r, "uid") else { return Ok(true) };
    let role = text(r, "role").filter(|v| v == "owner" || v == "cashier").unwrap_or_else(|| "cashier".into());
    let active = flag(r, "active");
    match id_by_uid(conn, "users", &uid)? {
        // Role and active flag travel between computers. PINs never do.
        Some(id) => {
            if !has_pending(conn, "users", &uid)? {
                conn.execute(
                    "UPDATE users SET role = ?1, active = ?2 WHERE id = ?3 AND (role != ?1 OR active != ?2)",
                    params![role, active, id],
                )
                .map_err(err)?;
            }
        }
        None => {
            resolve_user(conn, Some(&uid), text(r, "username").as_deref(), &role, active)?;
        }
    }
    Ok(true)
}

fn apply_product(conn: &Connection, r: &Value) -> R<bool> {
    let Some(uid) = text(r, "uid") else { return Ok(true) };
    let name = text(r, "name").unwrap_or_else(|| "Unnamed product".into());
    let updated_at = stamp(text(r, "updated_at")).unwrap_or_else(|| "1970-01-01 00:00:00".into());
    let barcode = text(r, "barcode").filter(|b| !b.trim().is_empty());
    // A barcode that a different local product already uses stays with that product.
    let clash = match &barcode {
        Some(code) => conn
            .query_row("SELECT 1 FROM products WHERE barcode = ?1 AND uid != ?2", params![code, uid], |_| Ok(()))
            .optional()
            .map_err(err)?
            .is_some(),
        None => false,
    };
    let (category, cost, price, reorder) = (
        text(r, "category"),
        int(r, "cost_price").unwrap_or(0),
        int(r, "sell_price").unwrap_or(0),
        int(r, "reorder_level").unwrap_or(5),
    );
    match id_by_uid(conn, "products", &uid)? {
        // Newest edit wins. Stock is not taken from here; it is rebuilt from movements.
        Some(id) => {
            conn.execute(
                "UPDATE products SET barcode = iif(?1, barcode, ?2), name = ?3, category = ?4, cost_price = ?5,
                    sell_price = ?6, reorder_level = ?7, active = ?8, updated_at = ?9
                 WHERE id = ?10 AND updated_at < ?9",
                params![clash, barcode, name, category, cost, price, reorder, flag(r, "active"), updated_at, id],
            )
            .map_err(err)?;
        }
        None => {
            conn.execute(
                "INSERT INTO products (uid, barcode, name, category, cost_price, sell_price, stock_qty, reorder_level,
                    active, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, COALESCE(?10, datetime('now', 'localtime')), ?11)",
                params![
                    uid,
                    if clash { None } else { barcode },
                    name,
                    category,
                    cost,
                    price,
                    int(r, "stock_qty").unwrap_or(0),
                    reorder,
                    flag(r, "active"),
                    stamp(text(r, "created_at")),
                    updated_at
                ],
            )
            .map_err(err)?;
        }
    }
    Ok(true)
}

fn apply_sale(conn: &Connection, r: &Value) -> R<bool> {
    let Some(uid) = text(r, "uid") else { return Ok(true) };
    let voided = text(r, "status").as_deref() == Some("voided");
    let voided_by = match text(r, "voided_by_uid") {
        Some(u) => Some(resolve_user(conn, Some(&u), None, "owner", 0)?),
        None => None,
    };
    if let Some(id) = id_by_uid(conn, "sales", &uid)? {
        // Voiding is the only change a sale can go through, and it cannot be undone.
        if voided {
            conn.execute(
                "UPDATE sales SET status = 'voided', voided_by = ?1, voided_at = ?2, void_reason = ?3
                 WHERE id = ?4 AND status = 'completed'",
                params![voided_by, stamp(text(r, "voided_at")), text(r, "void_reason"), id],
            )
            .map_err(err)?;
        }
        return Ok(true);
    }
    let user_id = resolve_user(conn, text(r, "user_uid").as_deref(), text(r, "cashier").as_deref(), "cashier", 0)?;
    let method = text(r, "payment_method").filter(|m| m == "cash" || m == "mpesa").unwrap_or_else(|| "cash".into());
    conn.execute(
        "INSERT INTO sales (uid, user_id, subtotal, discount, total, payment_method, mpesa_code, cash_tendered,
            change_given, status, voided_by, voided_at, void_reason, created_at, origin_device, origin_no)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
            COALESCE(?14, datetime('now', 'localtime')), ?15, ?16)",
        params![
            uid,
            user_id,
            int(r, "subtotal").unwrap_or(0),
            int(r, "discount").unwrap_or(0),
            int(r, "total").unwrap_or(0),
            method,
            text(r, "mpesa_code"),
            int(r, "cash_tendered"),
            int(r, "change_given"),
            if voided { "voided" } else { "completed" },
            voided_by,
            stamp(text(r, "voided_at")),
            text(r, "void_reason"),
            stamp(text(r, "created_at")),
            text(r, "device_id"),
            int(r, "local_id")
        ],
    )
    .map_err(err)?;
    Ok(true)
}

fn apply_sale_item(conn: &Connection, r: &Value) -> R<bool> {
    let Some(uid) = text(r, "uid") else { return Ok(true) };
    if id_by_uid(conn, "sale_items", &uid)?.is_some() {
        return Ok(true);
    }
    let sale_id = match text(r, "sale_uid") {
        Some(su) => id_by_uid(conn, "sales", &su)?,
        None => None,
    };
    let Some(sale_id) = sale_id else { return Ok(false) };
    let name = text(r, "product_name").unwrap_or_else(|| "Unknown product".into());
    let price = int(r, "unit_price").unwrap_or(0);
    let product_id = match text(r, "product_uid") {
        Some(pu) => match id_by_uid(conn, "products", &pu)? {
            Some(id) => id,
            None => placeholder_product(conn, Some(&pu), &name, price)?,
        },
        None => placeholder_product(conn, None, &name, price)?,
    };
    let qty = int(r, "qty").unwrap_or(1).max(1);
    conn.execute(
        "INSERT INTO sale_items (uid, sale_id, product_id, product_name, qty, unit_price, unit_cost, line_total)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            uid,
            sale_id,
            product_id,
            name,
            qty,
            price,
            int(r, "unit_cost").unwrap_or(0),
            int(r, "line_total").unwrap_or(qty * price)
        ],
    )
    .map_err(err)?;
    Ok(true)
}

fn apply_movement(conn: &Connection, r: &Value, touched: &mut BTreeSet<i64>) -> R<bool> {
    let Some(uid) = text(r, "uid") else { return Ok(true) };
    let count_to = int(r, "count_to");
    let existing: Option<(i64, i64)> = conn
        .query_row("SELECT id, product_id FROM stock_movements WHERE uid = ?1", params![uid], |x| {
            Ok((x.get(0)?, x.get(1)?))
        })
        .optional()
        .map_err(err)?;
    if let Some((id, product_id)) = existing {
        // The only later change a movement gets is its counted level being filled in.
        if count_to.is_some()
            && conn
                .execute("UPDATE stock_movements SET count_to = ?1 WHERE id = ?2 AND count_to IS NOT ?1", params![count_to, id])
                .map_err(err)?
                > 0
        {
            touched.insert(product_id);
        }
        return Ok(true);
    }
    let Some(reason) = text(r, "reason").filter(|v| REASONS.contains(&v.as_str())) else { return Ok(true) };
    let Some(product_uid) = text(r, "product_uid") else { return Ok(true) };
    let Some(product_id) = id_by_uid(conn, "products", &product_uid)? else { return Ok(false) };
    let ref_sale_id = match text(r, "sale_uid") {
        Some(su) => match id_by_uid(conn, "sales", &su)? {
            Some(id) => Some(id),
            None => return Ok(false),
        },
        None => None,
    };
    let user_id = resolve_user(conn, text(r, "user_uid").as_deref(), None, "cashier", 0)?;
    conn.execute(
        "INSERT INTO stock_movements (uid, product_id, qty_delta, reason, ref_sale_id, note, user_id, created_at,
            count_to, origin_device)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE(?8, datetime('now', 'localtime')), ?9, ?10)",
        params![
            uid,
            product_id,
            int(r, "qty_delta").unwrap_or(0),
            reason,
            ref_sale_id,
            text(r, "note"),
            user_id,
            stamp(text(r, "created_at")),
            count_to,
            text(r, "device_id")
        ],
    )
    .map_err(err)?;
    touched.insert(product_id);
    Ok(true)
}

fn apply_audit(conn: &Connection, r: &Value) -> R<bool> {
    let Some(uid) = text(r, "uid") else { return Ok(true) };
    if id_by_uid(conn, "audit_log", &uid)?.is_some() {
        return Ok(true);
    }
    let user_id = match text(r, "user_uid") {
        Some(u) => find_user(conn, &u)?,
        None => None,
    };
    let details = match r.get("details") {
        Some(Value::String(t)) => t.clone(),
        None | Some(Value::Null) => "{}".to_string(),
        Some(v) => v.to_string(),
    };
    conn.execute(
        "INSERT INTO audit_log (uid, user_id, action, entity, entity_id, details, created_at, origin_device)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE(?7, datetime('now', 'localtime')), ?8)",
        params![
            uid,
            user_id,
            text(r, "action").unwrap_or_else(|| "unknown".into()),
            text(r, "entity").unwrap_or_else(|| "unknown".into()),
            int(r, "entity_id"),
            details,
            stamp(text(r, "created_at")),
            text(r, "device_id")
        ],
    )
    .map_err(err)?;
    Ok(true)
}

// ---------- stock ----------

/// Rebuild stock for these products from the shared movement history.
///
/// The latest count sets the level at the moment it was taken, and everything
/// after it (sales, deliveries, voids, damage) moves it from there. Products
/// that were never counted are the plain sum of their movements. Every computer
/// applies the same rule to the same rows, so they agree.
pub fn recompute_stock(conn: &Connection, products: &BTreeSet<i64>) -> R<()> {
    if products.is_empty() {
        return Ok(());
    }
    let mut last_count = conn
        .prepare_cached(
            "SELECT count_to, created_at, id FROM stock_movements
             WHERE product_id = ?1 AND reason = 'count' AND count_to IS NOT NULL
             ORDER BY created_at DESC, id DESC LIMIT 1",
        )
        .map_err(err)?;
    let mut since = conn
        .prepare_cached(
            "SELECT COALESCE(SUM(qty_delta), 0) FROM stock_movements
             WHERE product_id = ?1 AND created_at > ?2 AND id != ?3
               AND NOT (reason = 'count' AND count_to IS NOT NULL)",
        )
        .map_err(err)?;
    let mut total = conn
        .prepare_cached("SELECT COUNT(*), COALESCE(SUM(qty_delta), 0) FROM stock_movements WHERE product_id = ?1")
        .map_err(err)?;
    let mut set = conn
        .prepare_cached("UPDATE products SET stock_qty = ?1 WHERE id = ?2 AND stock_qty != ?1")
        .map_err(err)?;
    for &id in products {
        let counted: Option<(i64, String, i64)> = last_count
            .query_row(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()
            .map_err(err)?;
        let stock = match counted {
            Some((level, at, count_id)) => {
                level + since.query_row(params![id, at, count_id], |r| r.get::<_, i64>(0)).map_err(err)?
            }
            None => {
                let (n, sum): (i64, i64) = total.query_row(params![id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?;
                if n == 0 {
                    continue;
                }
                sum
            }
        };
        set.execute(params![stock, id]).map_err(err)?;
    }
    Ok(())
}

// ---------- helpers ----------

fn id_by_uid(conn: &Connection, table: &str, uid: &str) -> R<Option<i64>> {
    let mut stmt = conn.prepare_cached(&format!("SELECT id FROM {table} WHERE uid = ?1")).map_err(err)?;
    let id = stmt.query_row(params![uid], |r| r.get(0)).optional().map_err(err)?;
    Ok(id)
}

fn has_pending(conn: &Connection, table: &str, uid: &str) -> R<bool> {
    let mut stmt = conn
        .prepare_cached("SELECT EXISTS (SELECT 1 FROM sync_outbox WHERE table_name = ?1 AND uid = ?2)")
        .map_err(err)?;
    let found: i64 = stmt.query_row(params![table, uid], |r| r.get(0)).map_err(err)?;
    Ok(found == 1)
}

fn find_user(conn: &Connection, uid: &str) -> R<Option<i64>> {
    if let Some(id) = id_by_uid(conn, "users", uid)? {
        return Ok(Some(id));
    }
    let mut stmt = conn.prepare_cached("SELECT user_id FROM user_aliases WHERE uid = ?1").map_err(err)?;
    let id = stmt.query_row(params![uid], |r| r.get(0)).optional().map_err(err)?;
    Ok(id)
}

/// Local id for another computer's user: same uid, a known alias, or the same
/// username (each computer's 'admin' is one person). Anyone new is added without
/// a PIN, so they cannot log in here until an owner sets one.
fn resolve_user(conn: &Connection, uid: Option<&str>, username: Option<&str>, role: &str, active: i64) -> R<i64> {
    if let Some(u) = uid {
        if let Some(id) = find_user(conn, u)? {
            return Ok(id);
        }
    }
    let name = username
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("user-{}", uid.unwrap_or("unknown").chars().take(6).collect::<String>()));
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM users WHERE username = ?1", params![name], |r| r.get(0))
        .optional()
        .map_err(err)?;
    if let Some(id) = existing {
        if let Some(u) = uid {
            conn.execute("INSERT OR IGNORE INTO user_aliases (uid, user_id) VALUES (?1, ?2)", params![u, id])
                .map_err(err)?;
        }
        return Ok(id);
    }
    match uid {
        Some(u) => conn.execute(
            "INSERT INTO users (uid, username, pin_hash, role, active) VALUES (?1, ?2, '', ?3, ?4)",
            params![u, name, role, active],
        ),
        None => conn.execute(
            "INSERT INTO users (username, pin_hash, role, active) VALUES (?1, '', ?2, ?3)",
            params![name, role, active],
        ),
    }
    .map_err(err)?;
    Ok(conn.last_insert_rowid())
}

/// Stand-in for a product this computer has not received yet. The real row
/// replaces its details when it arrives, because its updated_at is always newer.
fn placeholder_product(conn: &Connection, uid: Option<&str>, name: &str, price: i64) -> R<i64> {
    conn.execute(
        "INSERT INTO products (uid, name, sell_price, stock_qty, active, updated_at)
         VALUES (COALESCE(?1, lower(hex(randomblob(16)))), ?2, ?3, 0, 0, '1970-01-01 00:00:00')",
        params![uid, name, price],
    )
    .map_err(err)?;
    Ok(conn.last_insert_rowid())
}

fn text(r: &Value, k: &str) -> Option<String> {
    r.get(k).and_then(Value::as_str).map(str::to_string)
}

fn int(r: &Value, k: &str) -> Option<i64> {
    r.get(k).and_then(Value::as_i64)
}

fn flag(r: &Value, k: &str) -> i64 {
    r.get(k).and_then(Value::as_bool).unwrap_or(true) as i64
}

/// Cloud `timestamp` ("2026-09-13T17:20:30.123") to local text ("2026-09-13 17:20:30").
fn stamp(v: Option<String>) -> Option<String> {
    v.map(|t| t.replace('T', " ").chars().take(19).collect())
}

#[cfg(test)]
mod tests {
    //! Replays real cloud data against a copy of a real database.
    //! Run with LIQUORPOS_FIXTURES set to a folder holding mac.db and <table>.json files.

    use super::*;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    /// Optional real-data snapshot. Absent (or cleaned up) means the replay test is skipped.
    fn fixtures() -> Option<PathBuf> {
        std::env::var_os("LIQUORPOS_FIXTURES").map(PathBuf::from).filter(|dir| dir.join("mac.db").is_file())
    }

    fn rows(dir: &Path, table: &str) -> Vec<Value> {
        serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{table}.json"))).unwrap()).unwrap()
    }

    fn temp_db(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("liquorpos-test-{name}-{}.db", std::process::id()));
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
        path
    }

    fn scalar(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    fn stock(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT stock_qty FROM products WHERE name = ?1", params![name], |r| r.get(0)).unwrap()
    }

    fn stock_by_uid(conn: &Connection) -> BTreeMap<String, (String, i64)> {
        let mut stmt = conn.prepare("SELECT uid, name, stock_qty FROM products").unwrap();
        let map = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, String>(1)?, r.get::<_, i64>(2)?))))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        map
    }

    fn pull_into(conn: &Connection, dir: &Path, me: Option<&str>, patch: &dyn Fn(&str, &mut Value)) {
        set_setting(conn, "sync_pulling", "1").unwrap();
        let mut touched = BTreeSet::new();
        for table in TABLES {
            let mut data = rows(dir, table);
            if let Some(me) = me {
                if append_only(table) {
                    data.retain(|r| text(r, "device_id").as_deref() != Some(me));
                }
            }
            for r in data.iter_mut() {
                patch(table, r);
            }
            let applied = apply(conn, table, &data).unwrap();
            assert!(applied.hold.is_none(), "{table}: a row was held back waiting for another row");
            touched.extend(applied.touched);
        }
        recompute_stock(conn, &touched).unwrap();
        set_setting(conn, "sync_pulling", "0").unwrap();
    }

    /// Needs no snapshot: the rules every computer relies on to agree on stock.
    #[test]
    fn stock_follows_the_latest_count_whatever_order_rows_arrive_in() {
        let conn = crate::db::open_shop(&temp_db("rules")).unwrap();
        // The new shop's own admin account is already waiting to upload; pulled rows must add nothing.
        let queued = scalar(&conn, "SELECT COUNT(*) FROM sync_outbox");
        let product = |uid: &str, name: &str| serde_json::json!({
            "uid": uid, "name": name, "sell_price": 30000, "stock_qty": 0, "active": true, "updated_at": "2026-01-01T09:00:00"
        });
        let sale = |uid: &str, at: &str| serde_json::json!({
            "uid": uid, "user_uid": "u-cashier", "cashier": "cashier one", "subtotal": 30000, "total": 30000,
            "payment_method": "cash", "status": "completed", "created_at": at, "device_id": "till-1", "local_id": 7
        });
        let mv = |uid: &str, product: &str, delta: i64, reason: &str, at: &str, count_to: Option<i64>, sale: Option<&str>| serde_json::json!({
            "uid": uid, "product_uid": product, "qty_delta": delta, "reason": reason, "created_at": at,
            "count_to": count_to, "sale_uid": sale, "user_uid": "u-cashier", "device_id": "till-1"
        });
        let movements = vec![
            mv("m1", "p-counted", 31, "purchase", "2026-01-01T12:00:00", None, None),
            mv("m2", "p-counted", -1, "sale", "2026-01-01T17:00:00", None, Some("s1")),
            mv("m3", "p-counted", -21, "count", "2026-01-01T22:30:00", Some(10), None),
            mv("m4", "p-counted", -2, "sale", "2026-01-01T23:00:00", None, Some("s2")),
            mv("m5", "p-plain", 30, "purchase", "2026-01-01T12:00:00", None, None),
            mv("m6", "p-plain", -4, "sale", "2026-01-01T18:00:00", None, Some("s1")),
        ];

        set_setting(&conn, "sync_pulling", "1").unwrap();
        // A sale line that arrives before its sale is held back, not lost.
        let early = serde_json::json!({ "uid": "i1", "sale_uid": "s1", "product_uid": "p-plain", "product_name": "Plain", "qty": 4,
            "unit_price": 30000, "line_total": 120000, "synced_at": "2026-01-01T18:00:05Z" });
        assert_eq!(apply(&conn, "sale_items", &[early.clone()]).unwrap().hold.as_deref(), Some("2026-01-01T18:00:05Z"));

        apply(&conn, "products", &[product("p-counted", "Counted"), product("p-plain", "Plain")]).unwrap();
        apply(&conn, "sales", &[sale("s1", "2026-01-01T17:00:00"), sale("s2", "2026-01-01T23:00:00")]).unwrap();
        assert!(apply(&conn, "sale_items", &[early]).unwrap().hold.is_none());

        // Newest first, then again: order and repetition must not matter.
        let mut touched = BTreeSet::new();
        for batch in [movements.iter().rev().cloned().collect::<Vec<_>>(), movements.clone()] {
            let applied = apply(&conn, "stock_movements", &batch).unwrap();
            assert!(applied.hold.is_none());
            touched.extend(applied.touched);
        }
        recompute_stock(&conn, &touched).unwrap();
        set_setting(&conn, "sync_pulling", "0").unwrap();

        assert_eq!(stock(&conn, "Counted"), 8, "the count of 10 at 22:30, minus the 2 sold after it");
        assert_eq!(stock(&conn, "Plain"), 26, "never counted: deliveries minus sales");
        assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM stock_movements"), 6, "applying rows twice adds nothing");
        assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM sync_outbox"), queued, "received rows are never queued for upload");
        assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM users WHERE username = 'cashier one' AND pin_hash = ''"), 1);
        assert_eq!(scalar(&conn, "SELECT origin_no FROM sales WHERE uid = 's1'"), 7, "the till's receipt number is kept");
    }

    #[test]
    fn store_wide_pull_replays_real_data() {
        let Some(dir) = fixtures() else {
            eprintln!("no LIQUORPOS_FIXTURES snapshot; skipping the real-data replay");
            return;
        };

        // The Mac after upgrading: its counts regain their counted level and are queued for upload.
        let mac_path = temp_db("mac");
        std::fs::copy(dir.join("mac.db"), &mac_path).unwrap();
        let mac = crate::db::open(&mac_path).unwrap();
        assert_eq!(scalar(&mac, "SELECT COUNT(*) FROM stock_movements WHERE reason = 'count' AND count_to IS NOT NULL"), 60);
        let queued = scalar(&mac, "SELECT COUNT(*) FROM sync_outbox");
        assert_eq!(queued, 60);
        let me: String = mac.query_row("SELECT value FROM settings WHERE key = 'device_id'", [], |r| r.get(0)).unwrap();

        // The Mac receives the till's day.
        pull_into(&mac, &dir, Some(&me), &|_, _| {});
        assert_eq!(scalar(&mac, "SELECT COUNT(*) FROM sync_outbox"), queued, "pulled rows must not be sent back");
        assert_eq!(scalar(&mac, "SELECT COUNT(*) FROM sales WHERE created_at >= '2026-09-13' AND created_at < '2026-09-14'"), 40);
        assert_eq!(scalar(&mac, "SELECT SUM(total) FROM sales WHERE status = 'completed'"), 1_549_000);
        assert_eq!(scalar(&mac, "SELECT COUNT(*) FROM sale_items"), 40);
        assert_eq!(stock(&mac, "BALOZI CAN"), 2);
        assert_eq!(stock(&mac, "TUSKER MALT CAN"), 15);
        assert_eq!(stock(&mac, "WHITE CAP CAN"), 24);
        assert_eq!(stock(&mac, "MANYATTA PINEAPPLE"), 72);
        assert_eq!(stock(&mac, "SAFARI LEMONADE 300ML"), 30);
        assert_eq!(scalar(&mac, "SELECT COUNT(*) FROM users WHERE username = 'metty' AND pin_hash = ''"), 1);
        assert_eq!(scalar(&mac, "SELECT COUNT(*) FROM users WHERE username = 'admin'"), 1);

        // Receiving the same rows again changes nothing.
        let before = stock_by_uid(&mac);
        let sales = scalar(&mac, "SELECT COUNT(*) FROM sales");
        pull_into(&mac, &dir, Some(&me), &|_, _| {});
        assert_eq!(stock_by_uid(&mac), before);
        assert_eq!(scalar(&mac, "SELECT COUNT(*) FROM sales"), sales);

        // Another computer on the same store, receiving everything, lands on identical stock.
        // Its cloud rows carry the counted levels the Mac uploads after upgrading.
        let levels: BTreeMap<String, i64> = {
            let mut stmt = mac.prepare("SELECT uid, count_to FROM stock_movements WHERE count_to IS NOT NULL").unwrap();
            let map = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
            map
        };
        let other = crate::db::open(&temp_db("other")).unwrap();
        pull_into(&other, &dir, None, &|table, r| {
            if table == "stock_movements" {
                if let Some(level) = text(r, "uid").and_then(|u| levels.get(&u).copied()) {
                    r["count_to"] = Value::from(level);
                }
            }
        });
        let (a, b) = (stock_by_uid(&mac), stock_by_uid(&other));
        let diffs: Vec<_> = a.iter().filter(|(uid, v)| b.get(*uid) != Some(*v)).collect();
        assert!(diffs.is_empty(), "stock differs between computers: {diffs:?}");
        assert_eq!(scalar(&other, "SELECT COUNT(*) FROM sales"), 40);
        eprintln!("OK: {} products agree on both computers; Balozi {}, Safari Lemonade {}", b.len(), stock(&other, "BALOZI CAN"), stock(&other, "SAFARI LEMONADE 300ML"));
    }
}
