-- Store-wide sync (v0.2). Every computer logged into the same store receives
-- the other computers' sales, stock movements, users and audit entries.
-- Stock is rebuilt from the shared movement history; a count sets an absolute
-- level from the moment it was taken.

ALTER TABLE stock_movements ADD COLUMN count_to INTEGER;      -- counted level, for reason = 'count'
ALTER TABLE stock_movements ADD COLUMN origin_device TEXT;    -- NULL = made on this computer
ALTER TABLE sales ADD COLUMN origin_device TEXT;
ALTER TABLE sales ADD COLUMN origin_no INTEGER;               -- receipt number on the computer that made the sale
ALTER TABLE audit_log ADD COLUMN origin_device TEXT;

-- Names of the other computers in this store, for the Till column.
CREATE TABLE devices (
  device_id  TEXT PRIMARY KEY,
  name       TEXT,
  last_seen  TEXT
);

-- Another computer's account that is the same person as a local one (e.g. each till's 'admin').
CREATE TABLE user_aliases (
  uid      TEXT PRIMARY KEY,
  user_id  INTEGER NOT NULL REFERENCES users(id)
);

CREATE INDEX idx_outbox_uid ON sync_outbox(table_name, uid);
CREATE INDEX idx_stock_moves_count ON stock_movements(product_id, reason, created_at);

-- The same M-Pesa code can now arrive from another till; the sell screen still rejects reuse.
DROP INDEX idx_sales_mpesa;
CREATE INDEX idx_sales_mpesa ON sales(mpesa_code) WHERE mpesa_code IS NOT NULL;

DROP TRIGGER trg_sync_sales_insert;
DROP TRIGGER trg_sync_sales_update;
DROP TRIGGER trg_sync_stock_movements_insert;
DROP TRIGGER trg_sync_stock_movements_update;
DROP TRIGGER trg_sync_audit_log_insert;
DROP TRIGGER trg_sync_audit_log_update;

CREATE TRIGGER trg_sync_sales_insert AFTER INSERT ON sales
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('sales', NEW.uid, json_object('uid', NEW.uid, 'local_id', COALESCE(NEW.origin_no, NEW.id), 'device_id', NEW.origin_device, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'cashier', (SELECT username FROM users WHERE id = NEW.user_id), 'subtotal', NEW.subtotal, 'discount', NEW.discount, 'total', NEW.total, 'payment_method', NEW.payment_method, 'mpesa_code', NEW.mpesa_code, 'cash_tendered', NEW.cash_tendered, 'change_given', NEW.change_given, 'status', NEW.status, 'voided_by_uid', (SELECT uid FROM users WHERE id = NEW.voided_by), 'voided_at', NEW.voided_at, 'void_reason', NEW.void_reason, 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_sales_update AFTER UPDATE ON sales
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('sales', NEW.uid, json_object('uid', NEW.uid, 'local_id', COALESCE(NEW.origin_no, NEW.id), 'device_id', NEW.origin_device, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'cashier', (SELECT username FROM users WHERE id = NEW.user_id), 'subtotal', NEW.subtotal, 'discount', NEW.discount, 'total', NEW.total, 'payment_method', NEW.payment_method, 'mpesa_code', NEW.mpesa_code, 'cash_tendered', NEW.cash_tendered, 'change_given', NEW.change_given, 'status', NEW.status, 'voided_by_uid', (SELECT uid FROM users WHERE id = NEW.voided_by), 'voided_at', NEW.voided_at, 'void_reason', NEW.void_reason, 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_stock_movements_insert AFTER INSERT ON stock_movements
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('stock_movements', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'device_id', NEW.origin_device, 'product_uid', (SELECT uid FROM products WHERE id = NEW.product_id), 'qty_delta', NEW.qty_delta, 'count_to', NEW.count_to, 'reason', NEW.reason, 'sale_uid', (SELECT uid FROM sales WHERE id = NEW.ref_sale_id), 'note', NEW.note, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_stock_movements_update AFTER UPDATE ON stock_movements
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('stock_movements', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'device_id', NEW.origin_device, 'product_uid', (SELECT uid FROM products WHERE id = NEW.product_id), 'qty_delta', NEW.qty_delta, 'count_to', NEW.count_to, 'reason', NEW.reason, 'sale_uid', (SELECT uid FROM sales WHERE id = NEW.ref_sale_id), 'note', NEW.note, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_audit_log_insert AFTER INSERT ON audit_log
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('audit_log', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'device_id', NEW.origin_device, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'action', NEW.action, 'entity', NEW.entity, 'entity_id', NEW.entity_id, 'details', json(NEW.details), 'created_at', NEW.created_at));
END;

CREATE TRIGGER trg_sync_audit_log_update AFTER UPDATE ON audit_log
WHEN (SELECT value FROM settings WHERE key = 'sync_pulling') IS NOT '1'
BEGIN
  INSERT INTO sync_outbox (table_name, uid, payload)
  VALUES ('audit_log', NEW.uid, json_object('uid', NEW.uid, 'local_id', NEW.id, 'device_id', NEW.origin_device, 'user_uid', (SELECT uid FROM users WHERE id = NEW.user_id), 'action', NEW.action, 'entity', NEW.entity, 'entity_id', NEW.entity_id, 'details', json(NEW.details), 'created_at', NEW.created_at));
END;

-- Counts made before this version stored only the difference. The counted level
-- was written to the audit log in the same transaction, so recover it from there.
-- Updating these rows queues them for upload with the new count_to value.
UPDATE stock_movements SET count_to = (
  SELECT json_extract(a.details, '$.after') FROM audit_log a
  WHERE a.action = 'adjust_stock' AND a.entity = 'product' AND a.entity_id = stock_movements.product_id
    AND a.created_at = stock_movements.created_at AND json_extract(a.details, '$.reason') = 'count'
  ORDER BY a.id DESC LIMIT 1)
WHERE reason = 'count' AND count_to IS NULL AND EXISTS (
  SELECT 1 FROM audit_log a
  WHERE a.action = 'adjust_stock' AND a.entity = 'product' AND a.entity_id = stock_movements.product_id
    AND a.created_at = stock_movements.created_at AND json_extract(a.details, '$.reason') = 'count');
