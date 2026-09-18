use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Product {
    pub id: i64,
    pub barcode: Option<String>,
    pub name: String,
    pub category: Option<String>,
    pub cost_price: i64,
    pub sell_price: i64,
    pub stock_qty: i64,
    pub reorder_level: i64,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductInput {
    pub id: Option<i64>,
    pub barcode: Option<String>,
    pub name: String,
    pub category: Option<String>,
    pub cost_price: i64,
    pub sell_price: i64,
    pub reorder_level: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleItemInput {
    pub product_id: i64,
    pub qty: i64,
    pub unit_price: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleInput {
    pub items: Vec<SaleItemInput>,
    pub discount: i64,
    pub payment_method: String,
    pub mpesa_code: Option<String>,
    pub cash_tendered: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sale {
    pub id: i64,
    pub cashier: String,
    pub subtotal: i64,
    pub discount: i64,
    pub total: i64,
    pub payment_method: String,
    pub mpesa_code: Option<String>,
    pub cash_tendered: Option<i64>,
    pub change_given: Option<i64>,
    pub status: String,
    pub void_reason: Option<String>,
    pub item_count: i64,
    pub created_at: String,
    /// Receipt number on the computer that made the sale.
    pub receipt_no: i64,
    /// Name of the computer that made the sale.
    pub till: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleItem {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub qty: i64,
    pub unit_price: i64,
    pub line_total: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleDetail {
    pub sale: Sale,
    pub items: Vec<SaleItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiveLine {
    pub product_id: i64,
    pub qty: i64,
    pub unit_cost: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockMovement {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub qty_delta: i64,
    pub reason: String,
    pub ref_sale_id: Option<i64>,
    pub note: Option<String>,
    pub user: String,
    pub created_at: String,
    pub ref_receipt_no: Option<i64>,
    /// Set when the movement was made on another computer.
    pub till: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: i64,
    pub user: Option<String>,
    pub action: String,
    pub entity: String,
    pub entity_id: Option<i64>,
    pub details: String,
    pub created_at: String,
    /// Set when the entry was made on another computer.
    pub till: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SalesSummary {
    pub sales_count: i64,
    pub gross: i64,
    pub discounts: i64,
    pub net: i64,
    pub cost: i64,
    pub profit: i64,
    pub cash_total: i64,
    pub mpesa_total: i64,
    pub items_sold: i64,
    pub voided_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPoint {
    pub day: String,
    pub sales_count: i64,
    pub net: i64,
    pub profit: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopProduct {
    pub product_id: i64,
    pub name: String,
    pub qty: i64,
    pub revenue: i64,
    pub profit: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockValue {
    pub product_count: i64,
    pub units: i64,
    pub cost_value: i64,
    pub retail_value: i64,
    pub low_stock_count: i64,
}

/// One day on which stock was counted.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CountSession {
    pub day: String,
    pub products: i64,
    /// Products counted for the first time that day. They set a baseline and are
    /// left out of the short/over figures, which only compare count to count.
    pub baselines: i64,
    pub short_units: i64,
    pub over_units: i64,
    /// What the missing units would have sold for.
    pub short_value: i64,
    pub over_value: i64,
    /// The same at cost price; zero until cost prices are recorded.
    pub short_cost: i64,
    pub over_cost: i64,
    pub users: String,
}

/// One product on a count day: what the system expected before the first count
/// and what was found at the last count of that day.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VarianceRow {
    pub product_id: i64,
    pub name: String,
    pub category: Option<String>,
    /// The count before this one, if any, and when it was taken.
    pub previous_count: Option<i64>,
    pub previous_at: Option<String>,
    /// Movements between the previous count (or the start of records) and this count.
    pub sold: i64,
    pub received: i64,
    pub adjusted: i64,
    /// previous_count + received - sold + adjusted: what the system expected to find.
    pub expected: i64,
    pub counted: i64,
    pub difference: i64,
    pub sell_price: i64,
    pub cost_price: i64,
    pub note: Option<String>,
    pub user: String,
    pub at: String,
}
