use std::io::Write;

use rusqlite::{params, Connection};
use tauri::{AppHandle, Manager, State};

use crate::commands::auth::{require_owner, Session};
use crate::db::Db;
use crate::shops::{slug, Shops};
use crate::error::{bad, AppResult};
use crate::models::{CountSession, DailyPoint, SalesSummary, StockValue, TopProduct, VarianceRow};

pub(crate) fn check_range(from: &str, to: &str) -> AppResult<()> {
    let ok = |s: &str| s.len() == 10 && s.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
    if !ok(from) || !ok(to) {
        return Err(bad("Dates must be YYYY-MM-DD"));
    }
    Ok(())
}

#[tauri::command]
pub fn sales_summary(db: State<Db>, session: State<Session>, from: String, to: String) -> AppResult<SalesSummary> {
    require_owner(&session)?;
    check_range(&from, &to)?;
    sales_summary_for(&db.lock(), &from, &to)
}

pub(crate) fn sales_summary_for(conn: &Connection, from: &str, to: &str) -> AppResult<SalesSummary> {
    let (sales_count, gross, discounts, net, cash_total, mpesa_total): (i64, i64, i64, i64, i64, i64) = conn
        .prepare_cached(
            "SELECT COUNT(*), COALESCE(SUM(subtotal), 0), COALESCE(SUM(discount), 0), COALESCE(SUM(total), 0),
                    COALESCE(SUM(CASE WHEN payment_method = 'cash' THEN total END), 0),
                    COALESCE(SUM(CASE WHEN payment_method = 'mpesa' THEN total END), 0)
             FROM sales WHERE status = 'completed' AND created_at >= ?1 AND created_at < date(?2, '+1 day')",
        )?
        .query_row(params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;

    let (items_sold, cost): (i64, i64) = conn
        .prepare_cached(
            "SELECT COALESCE(SUM(si.qty), 0), COALESCE(SUM(si.qty * si.unit_cost), 0)
             FROM sales s JOIN sale_items si ON si.sale_id = s.id
             WHERE s.status = 'completed' AND s.created_at >= ?1 AND s.created_at < date(?2, '+1 day')",
        )?
        .query_row(params![from, to], |r| Ok((r.get(0)?, r.get(1)?)))?;

    let voided_count: i64 = conn
        .prepare_cached(
            "SELECT COUNT(*) FROM sales WHERE status = 'voided' AND created_at >= ?1 AND created_at < date(?2, '+1 day')",
        )?
        .query_row(params![from, to], |r| r.get(0))?;

    Ok(SalesSummary {
        sales_count,
        gross,
        discounts,
        net,
        cost,
        profit: net - cost,
        cash_total,
        mpesa_total,
        items_sold,
        voided_count,
    })
}

#[tauri::command]
pub fn daily_sales(db: State<Db>, session: State<Session>, from: String, to: String) -> AppResult<Vec<DailyPoint>> {
    require_owner(&session)?;
    check_range(&from, &to)?;
    daily_sales_for(&db.lock(), &from, &to)
}

pub(crate) fn daily_sales_for(conn: &Connection, from: &str, to: &str) -> AppResult<Vec<DailyPoint>> {
    let mut stmt = conn.prepare_cached(
        "SELECT day, COUNT(*), SUM(total), SUM(total - cost) FROM (
            SELECT date(s.created_at) AS day, s.total,
                   (SELECT COALESCE(SUM(qty * unit_cost), 0) FROM sale_items WHERE sale_id = s.id) AS cost
            FROM sales s
            WHERE s.status = 'completed' AND s.created_at >= ?1 AND s.created_at < date(?2, '+1 day')
         ) GROUP BY day ORDER BY day",
    )?;
    let rows = stmt.query_map(params![from, to], |r| {
        Ok(DailyPoint { day: r.get(0)?, sales_count: r.get(1)?, net: r.get(2)?, profit: r.get(3)? })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[tauri::command]
pub fn top_products(
    db: State<Db>,
    session: State<Session>,
    from: String,
    to: String,
    limit: Option<i64>,
) -> AppResult<Vec<TopProduct>> {
    require_owner(&session)?;
    check_range(&from, &to)?;
    let conn = db.lock();
    let mut stmt = conn.prepare_cached(
        "SELECT si.product_id, si.product_name, SUM(si.qty), SUM(si.line_total), SUM(si.line_total - si.qty * si.unit_cost)
         FROM sale_items si JOIN sales s ON s.id = si.sale_id
         WHERE s.status = 'completed' AND s.created_at >= ?1 AND s.created_at < date(?2, '+1 day')
         GROUP BY si.product_id ORDER BY 4 DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![from, to, limit.unwrap_or(20)], |r| {
        Ok(TopProduct { product_id: r.get(0)?, name: r.get(1)?, qty: r.get(2)?, revenue: r.get(3)?, profit: r.get(4)? })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[tauri::command]
pub fn stock_value(db: State<Db>, session: State<Session>) -> AppResult<StockValue> {
    require_owner(&session)?;
    stock_value_for(&db.lock())
}

pub(crate) fn stock_value_for(conn: &Connection) -> AppResult<StockValue> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT COUNT(*), COALESCE(SUM(stock_qty), 0),
                    COALESCE(SUM(MAX(stock_qty, 0) * cost_price), 0),
                    COALESCE(SUM(MAX(stock_qty, 0) * sell_price), 0),
                    COALESCE(SUM(stock_qty <= reorder_level), 0)
             FROM products WHERE active = 1",
        )?;
    let value = stmt.query_row([], |r| {
        Ok(StockValue {
            product_count: r.get(0)?,
            units: r.get(1)?,
            cost_value: r.get(2)?,
            retail_value: r.get(3)?,
            low_stock_count: r.get(4)?,
        })
    })?;
    Ok(value)
}

// ---------- stock counts ----------

/// Count days, newest first. Differences are netted per product within the day,
/// so a product counted twice the same day is not reported twice. A product's
/// first ever count only sets its baseline: whatever the system held before it
/// was never a real count, so it is not reported as missing or found.
#[tauri::command]
pub fn count_sessions(db: State<Db>, session: State<Session>) -> AppResult<Vec<CountSession>> {
    require_owner(&session)?;
    count_sessions_for(&db.lock())
}

pub(crate) fn count_sessions_for(conn: &Connection) -> AppResult<Vec<CountSession>> {
    let mut stmt = conn.prepare_cached(
        "SELECT day, COUNT(*), COALESCE(SUM(baseline), 0),
                COALESCE(SUM(CASE WHEN NOT baseline AND d < 0 THEN -d END), 0), COALESCE(SUM(CASE WHEN NOT baseline AND d > 0 THEN d END), 0),
                COALESCE(SUM(CASE WHEN NOT baseline AND d < 0 THEN -d * sell END), 0), COALESCE(SUM(CASE WHEN NOT baseline AND d > 0 THEN d * sell END), 0),
                COALESCE(SUM(CASE WHEN NOT baseline AND d < 0 THEN -d * cost END), 0), COALESCE(SUM(CASE WHEN NOT baseline AND d > 0 THEN d * cost END), 0),
                (SELECT GROUP_CONCAT(DISTINCT u.username) FROM stock_movements x JOIN users u ON u.id = x.user_id
                  WHERE x.reason = 'count' AND x.count_to IS NOT NULL AND date(x.created_at) = day)
         FROM (
            SELECT date(m.created_at) AS day, m.product_id, SUM(m.qty_delta) AS d, MAX(p.sell_price) AS sell, MAX(p.cost_price) AS cost,
                   NOT EXISTS (SELECT 1 FROM stock_movements c WHERE c.product_id = m.product_id AND c.reason = 'count'
                               AND c.count_to IS NOT NULL AND date(c.created_at) < date(m.created_at)) AS baseline
            FROM stock_movements m JOIN products p ON p.id = m.product_id
            WHERE m.reason = 'count' AND m.count_to IS NOT NULL
            GROUP BY day, m.product_id
         )
         GROUP BY day ORDER BY day DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(CountSession {
            day: r.get(0)?,
            products: r.get(1)?,
            baselines: r.get(2)?,
            short_units: r.get(3)?,
            over_units: r.get(4)?,
            short_value: r.get(5)?,
            over_value: r.get(6)?,
            short_cost: r.get(7)?,
            over_cost: r.get(8)?,
            users: r.get::<_, Option<String>>(9)?.unwrap_or_default(),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[tauri::command]
pub fn count_variance(db: State<Db>, session: State<Session>, day: String) -> AppResult<Vec<VarianceRow>> {
    require_owner(&session)?;
    check_range(&day, &day)?;
    count_variance_for(&db.lock(), &day)
}

pub(crate) fn count_variance_for(conn: &Connection, day: &str) -> AppResult<Vec<VarianceRow>> {
    // For each product counted that day: `t1` is its first count of the day and
    // `prev` the last count before that. The working covers everything between.
    let mut stmt = conn.prepare_cached(
        "WITH first AS (
            SELECT product_id, MIN(created_at) AS t1 FROM stock_movements
            WHERE reason = 'count' AND count_to IS NOT NULL AND date(created_at) = ?1 GROUP BY product_id
         ), prev AS (
            SELECT f.product_id, f.t1,
              (SELECT c.count_to FROM stock_movements c WHERE c.product_id = f.product_id AND c.reason = 'count'
                 AND c.count_to IS NOT NULL AND c.created_at < f.t1 ORDER BY c.created_at DESC, c.id DESC LIMIT 1) AS level,
              (SELECT c.created_at FROM stock_movements c WHERE c.product_id = f.product_id AND c.reason = 'count'
                 AND c.count_to IS NOT NULL AND c.created_at < f.t1 ORDER BY c.created_at DESC, c.id DESC LIMIT 1) AS at
            FROM first f
         )
         SELECT p.id, p.name, p.category, prev.level, prev.at,
                COALESCE(-(SELECT SUM(s.qty_delta) FROM stock_movements s WHERE s.product_id = p.id AND s.reason = 'sale'
                    AND s.created_at < prev.t1 AND (prev.at IS NULL OR s.created_at > prev.at)), 0),
                COALESCE((SELECT SUM(s.qty_delta) FROM stock_movements s WHERE s.product_id = p.id AND s.reason IN ('purchase', 'void')
                    AND s.created_at < prev.t1 AND (prev.at IS NULL OR s.created_at > prev.at)), 0),
                COALESCE((SELECT SUM(s.qty_delta) FROM stock_movements s WHERE s.product_id = p.id
                    AND (s.reason IN ('damage', 'adjustment') OR (s.reason = 'count' AND s.count_to IS NULL))
                    AND s.created_at < prev.t1 AND (prev.at IS NULL OR s.created_at > prev.at)), 0),
                (SELECT f.count_to - f.qty_delta FROM stock_movements f WHERE f.product_id = p.id AND f.reason = 'count'
                    AND f.count_to IS NOT NULL AND date(f.created_at) = ?1 ORDER BY f.created_at, f.id LIMIT 1),
                (SELECT l.count_to FROM stock_movements l WHERE l.product_id = p.id AND l.reason = 'count'
                    AND l.count_to IS NOT NULL AND date(l.created_at) = ?1 ORDER BY l.created_at DESC, l.id DESC LIMIT 1),
                SUM(m.qty_delta), p.sell_price, p.cost_price,
                (SELECT GROUP_CONCAT(n, '; ') FROM (SELECT DISTINCT x.note AS n FROM stock_movements x
                    WHERE x.product_id = p.id AND x.reason = 'count' AND x.count_to IS NOT NULL AND date(x.created_at) = ?1
                      AND x.note IS NOT NULL AND x.note != '')),
                (SELECT GROUP_CONCAT(DISTINCT u.username) FROM stock_movements x JOIN users u ON u.id = x.user_id
                    WHERE x.product_id = p.id AND x.reason = 'count' AND x.count_to IS NOT NULL AND date(x.created_at) = ?1),
                MAX(m.created_at)
         FROM stock_movements m
         JOIN products p ON p.id = m.product_id
         JOIN prev ON prev.product_id = p.id
         WHERE m.reason = 'count' AND m.count_to IS NOT NULL AND date(m.created_at) = ?1
         GROUP BY p.id ORDER BY p.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![day], |r| {
        Ok(VarianceRow {
            product_id: r.get(0)?,
            name: r.get(1)?,
            category: r.get(2)?,
            previous_count: r.get(3)?,
            previous_at: r.get(4)?,
            sold: r.get(5)?,
            received: r.get(6)?,
            adjusted: r.get(7)?,
            expected: r.get(8)?,
            counted: r.get(9)?,
            difference: r.get(10)?,
            sell_price: r.get(11)?,
            cost_price: r.get(12)?,
            note: r.get(13)?,
            user: r.get::<_, Option<String>>(14)?.unwrap_or_default(),
            at: r.get(15)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// A sheet to count against, saved to Documents/Liquor POS/exports. Fill the
/// blank `stock` column and bring it back through Products > Import CSV; rows
/// left blank are not touched.
#[tauri::command]
pub fn export_count_sheet_csv(app: AppHandle, db: State<Db>, shops: State<Shops>, session: State<Session>) -> AppResult<String> {
    require_owner(&session)?;
    let dir = app.path().document_dir()?.join("Liquor POS").join("exports");
    std::fs::create_dir_all(&dir)?;
    let conn = db.lock();
    let today: String = conn.query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))?;
    let path = dir.join(format!("count-sheet-{}-{today}.csv", slug(&shops.current().name)));

    let mut stmt = conn.prepare_cached(
        "SELECT name, COALESCE(category, ''), COALESCE(barcode, ''), stock_qty FROM products WHERE active = 1
         ORDER BY category COLLATE NOCASE, name COLLATE NOCASE",
    )?;
    let mut file = std::io::BufWriter::new(std::fs::File::create(&path)?);
    writeln!(file, "name,category,barcode,expected,stock")?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        writeln!(
            file,
            "{},{},{},{},",
            csv_cell(&r.get::<_, String>(0)?),
            csv_cell(&r.get::<_, String>(1)?),
            csv_cell(&r.get::<_, String>(2)?),
            r.get::<_, i64>(3)?
        )?;
    }
    file.flush()?;
    Ok(path.to_string_lossy().into_owned())
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn money(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, (cents % 100).abs())
}

/// Writes every sale line in the range to a CSV in Documents/Liquor POS/exports
/// and returns the path. Opens directly in Excel.
#[tauri::command]
pub fn export_sales_csv(
    app: AppHandle,
    db: State<Db>,
    shops: State<Shops>,
    session: State<Session>,
    from: String,
    to: String,
) -> AppResult<String> {
    require_owner(&session)?;
    check_range(&from, &to)?;
    let dir = app.path().document_dir()?.join("Liquor POS").join("exports");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("sales-{}-{from}-to-{to}.csv", slug(&shops.current().name)));

    let conn = db.lock();
    let mut stmt = conn.prepare_cached(
        "SELECT COALESCE(s.origin_no, s.id), s.created_at, u.username, s.status, s.payment_method, COALESCE(s.mpesa_code, ''),
                si.product_name, si.qty, si.unit_price, si.line_total, si.unit_cost, s.discount, s.total
         FROM sales s JOIN users u ON u.id = s.user_id JOIN sale_items si ON si.sale_id = s.id
         WHERE s.created_at >= ?1 AND s.created_at < date(?2, '+1 day')
         ORDER BY s.id, si.id",
    )?;
    let mut file = std::io::BufWriter::new(std::fs::File::create(&path)?);
    writeln!(
        file,
        "Sale No,Date,Cashier,Status,Payment,M-Pesa Code,Product,Qty,Unit Price,Line Total,Unit Cost,Sale Discount,Sale Total"
    )?;
    let mut rows = stmt.query(params![from, to])?;
    while let Some(r) = rows.next()? {
        let line = [
            r.get::<_, i64>(0)?.to_string(),
            r.get::<_, String>(1)?,
            csv_cell(&r.get::<_, String>(2)?),
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            csv_cell(&r.get::<_, String>(6)?),
            r.get::<_, i64>(7)?.to_string(),
            money(r.get(8)?),
            money(r.get(9)?),
            money(r.get(10)?),
            money(r.get(11)?),
            money(r.get(12)?),
        ];
        writeln!(file, "{}", line.join(","))?;
    }
    file.flush()?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_day_reports_each_product_once_with_its_net_difference() {
        let path = std::env::temp_dir().join(format!("liquorpos-counts-{}.db", std::process::id()));
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
        let conn = crate::db::open_shop(&path).unwrap();
        conn.execute_batch(
            "INSERT INTO products (name, category, sell_price, cost_price, stock_qty) VALUES
               ('Product A', 'Cat 1', 30000, 20000, 2), ('Product B', 'Cat 2', 45000, 0, 12), ('Product C', 'Cat 1', 10000, 0, 9);
             -- Before day one: A was delivered (31) and never counted; B delivered 30 and sold 6.
             INSERT INTO stock_movements (product_id, qty_delta, reason, note, user_id, count_to, created_at) VALUES
               (1, 31, 'purchase', 'opening', 1, NULL, '2026-09-10 09:00:00'),
               (2, 30, 'purchase', 'opening', 1, NULL, '2026-09-10 09:00:00'),
               (2, -6, 'sale', NULL, 1, NULL, '2026-09-11 19:00:00'),
             -- Day one: A counted twice (31 -> 10 -> 2), B once (24 -> 12), C once but unchanged.
               (1, -21, 'count', 'weekly', 1, 10, '2026-09-13 22:32:00'),
               (1, -8, 'count', 'recount', 1, 2, '2026-09-13 22:33:00'),
               (2, -12, 'count', 'weekly', 1, 12, '2026-09-13 22:35:00'),
               (3, 0, 'count', NULL, 1, 9, '2026-09-13 22:36:00'),
             -- Between the counts B sold 2, took a delivery of 4 and had 1 logged as damaged: expected 13.
               (2, -2, 'sale', NULL, 1, NULL, '2026-09-15 20:00:00'),
               (2, 4, 'purchase', 'top up', 1, NULL, '2026-09-16 10:00:00'),
               (2, -1, 'damage', 'broken', 1, NULL, '2026-09-17 12:00:00'),
             -- Day two: B found 2 over.
               (2, 2, 'count', 'found extra', 1, 15, '2026-09-18 09:00:00'),
             -- A plain sale must not appear in count results.
               (2, -1, 'sale', NULL, 1, NULL, '2026-09-18 10:00:00');",
        )
        .unwrap();

        let days = count_sessions_for(&conn).unwrap();
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].day, "2026-09-18");
        assert_eq!((days[0].products, days[0].baselines, days[0].short_units, days[0].over_units, days[0].over_value), (1, 0, 0, 2, 90_000));
        let d1 = &days[1];
        assert_eq!((d1.products, d1.baselines), (3, 3), "every product was counted for the first time");
        assert_eq!((d1.short_units, d1.over_units, d1.short_value, d1.short_cost), (0, 0, 0, 0), "a first count sets a baseline, it is not a shortage");
        assert_eq!(d1.users, "admin");

        let rows = count_variance_for(&conn, "2026-09-13").unwrap();
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Product A", "Product B", "Product C"]);
        let a = &rows[0];
        assert_eq!((a.expected, a.counted, a.difference), (31, 2, -29), "first expected, last counted, net difference");
        assert_eq!(a.note.as_deref(), Some("weekly; recount"));
        assert_eq!((rows[2].expected, rows[2].counted, rows[2].difference), (9, 9, 0));
        assert_eq!((a.previous_count, a.sold, a.received, a.adjusted), (None, 0, 31, 0), "never counted before: the working starts from the first delivery");
        let b = &rows[1];
        assert_eq!((b.previous_count, b.sold, b.received, b.expected, b.counted), (None, 6, 30, 24, 12));
        assert!(count_variance_for(&conn, "2026-09-14").unwrap().is_empty());

        // Day two shows the working since the previous count: 12 counted, sold 2, received 4, 1 damaged -> expected 13, found 15.
        let rows = count_variance_for(&conn, "2026-09-18").unwrap();
        assert_eq!(rows.len(), 1);
        let b = &rows[0];
        assert_eq!(b.previous_count, Some(12));
        assert_eq!(b.previous_at.as_deref(), Some("2026-09-13 22:35:00"));
        assert_eq!((b.sold, b.received, b.adjusted), (2, 4, -1));
        assert_eq!(b.previous_count.unwrap() + b.received - b.sold + b.adjusted, b.expected, "the working adds up to the expected number");
        assert_eq!((b.expected, b.counted, b.difference), (13, 15, 2));
    }
}
