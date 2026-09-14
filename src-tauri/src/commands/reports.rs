use std::io::Write;

use rusqlite::params;
use tauri::{AppHandle, Manager, State};

use crate::commands::auth::{require_owner, require_user, Session};
use crate::db::Db;
use crate::shops::{slug, Shops};
use crate::error::{bad, AppResult};
use crate::models::{DailyPoint, SalesSummary, StockValue, TopProduct};

fn check_range(from: &str, to: &str) -> AppResult<()> {
    let ok = |s: &str| s.len() == 10 && s.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
    if !ok(from) || !ok(to) {
        return Err(bad("Dates must be YYYY-MM-DD"));
    }
    Ok(())
}

#[tauri::command]
pub fn sales_summary(db: State<Db>, session: State<Session>, from: String, to: String) -> AppResult<SalesSummary> {
    require_user(&session)?;
    check_range(&from, &to)?;
    let conn = db.lock();

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
    require_user(&session)?;
    check_range(&from, &to)?;
    let conn = db.lock();
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
    require_user(&session)?;
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
    require_user(&session)?;
    let conn = db.lock();
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
