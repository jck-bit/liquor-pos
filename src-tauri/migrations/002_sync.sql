-- Cloud sync queue. Rows are pushed to Supabase in id order and deleted once accepted.
-- The pull side sets settings.sync_pulling = '1' while it writes, so its writes are not re-queued.

CREATE TABLE sync_outbox (
  id          INTEGER PRIMARY KEY,
  table_name  TEXT NOT NULL,
  uid         TEXT NOT NULL,
  payload     TEXT NOT NULL,
  created_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_outbox_table ON sync_outbox(table_name, id);


CREATE TRIGGER trg_sync_users_insert AFTER INSERT ON users
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('users', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'username', NEW.username, 'role', NEW.role, 'active', json(iif(NEW.active, 'true', 'false')), 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_users_update AFTER UPDATE ON users
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('users', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'username', NEW.username, 'role', NEW.role, 'active', json(iif(NEW.active, 'true', 'false')), 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_products_insert AFTER INSERT ON products
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('products', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'barcode', NEW.barcode, 'name', NEW.name, 'category', NEW.category, 'cost_price', NEW.cost_price, 'sell_price', NEW.sell_price, 'stock_qty', NEW.stock_qty, 'reorder_level', NEW.reorder_level, 'active', json(iif(NEW.active, 'true', 'false')), 'created_at', NEW.created_at, 'updated_at', NEW.updated_at));
END;

CREATE TRIGGER trg_sync_products_update AFTER UPDATE ON products
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('products', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'barcode', NEW.barcode, 'name', NEW.name, 'category', NEW.category, 'cost_price', NEW.cost_price, 'sell_price', NEW.sell_price, 'stock_qty', NEW.stock_qty, 'reorder_level', NEW.reorder_level, 'active', json(iif(NEW.active, 'true', 'false')), 'created_at', NEW.created_at, 'updated_at', NEW.updated_at));
END;

CREATE TRIGGER trg_sync_sales_insert AFTER INSERT ON sales
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('sales', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'cashier', (SELECT username FROM users WHERE id = NEW.user_id), 'subtotal', NEW.subtotal, 'discount', NEW.discount, 'total', NEW.total, 'payment_method', NEW.payment_method, 'mpesa_code', NEW.mpesa_code, 'cash_tendered', NEW.cash_tendered, 'change_given', NEW.change_given, 'status', NEW.status, 'voided_by_uid', (SELECT uid FROM users WHERE id = NEW.voided_by), 'voided_at', NEW.voided_at, 'void_reason', NEW.void_reason, 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_sales_update AFTER UPDATE ON sales
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('sales', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'cashier', (SELECT username FROM users WHERE id = NEW.user_id), 'subtotal', NEW.subtotal, 'discount', NEW.discount, 'total', NEW.total, 'payment_method', NEW.payment_method, 'mpesa_code', NEW.mpesa_code, 'cash_tendered', NEW.cash_tendered, 'change_given', NEW.change_given, 'status', NEW.status, 'voided_by_uid', (SELECT uid FROM users WHERE id = NEW.voided_by), 'voided_at', NEW.voided_at, 'void_reason', NEW.void_reason, 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_sale_items_insert AFTER INSERT ON sale_items
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('sale_items', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'sale_uid', (SELECT uid FROM sales WHERE id = NEW.sale_id), 'product_uid', (SELECT uid FROM products WHERE id = NEW.product_id), 'product_name', NEW.product_name, 'qty', NEW.qty, 'unit_price', NEW.unit_price, 'unit_cost', NEW.unit_cost, 'line_total', NEW.line_total));
END;

CREATE TRIGGER trg_sync_sale_items_update AFTER UPDATE ON sale_items
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('sale_items', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'sale_uid', (SELECT uid FROM sales WHERE id = NEW.sale_id), 'product_uid', (SELECT uid FROM products WHERE id = NEW.product_id), 'product_name', NEW.product_name, 'qty', NEW.qty, 'unit_price', NEW.unit_price, 'unit_cost', NEW.unit_cost, 'line_total', NEW.line_total));
END;

CREATE TRIGGER trg_sync_stock_movements_insert AFTER INSERT ON stock_movements
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('stock_movements', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'product_uid', (SELECT uid FROM products WHERE id = NEW.product_id), 'qty_delta', NEW.qty_delta, 'reason', NEW.reason, 'sale_uid', (SELECT uid FROM sales WHERE id = NEW.ref_sale_id), 'note', NEW.note, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_stock_movements_update AFTER UPDATE ON stock_movements
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('stock_movements', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'product_uid', (SELECT uid FROM products WHERE id = NEW.product_id), 'qty_delta', NEW.qty_delta, 'reason', NEW.reason, 'sale_uid', (SELECT uid FROM sales WHERE id = NEW.ref_sale_id), 'note', NEW.note, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_audit_log_insert AFTER INSERT ON audit_log
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('audit_log', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'action', NEW.action, 'entity', NEW.entity, 'entity_id', NEW.entity_id, 'details', json(NEW.details), 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_audit_log_update AFTER UPDATE ON audit_log
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('audit_log', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'action', NEW.action, 'entity', NEW.entity, 'entity_id', NEW.entity_id, 'details', json(NEW.details), 'created_at', NEW.created_at));
END;
