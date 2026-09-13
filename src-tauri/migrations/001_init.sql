-- All money is stored as integer cents (KES). Timestamps are local time text 'YYYY-MM-DD HH:MM:SS'.

CREATE TABLE users (
  id          INTEGER PRIMARY KEY,
  uid         TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
  username    TEXT NOT NULL UNIQUE COLLATE NOCASE,
  pin_hash    TEXT NOT NULL,
  role        TEXT NOT NULL CHECK (role IN ('owner', 'cashier')),
  active      INTEGER NOT NULL DEFAULT 1,
  created_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE products (
  id             INTEGER PRIMARY KEY,
  uid         TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
  barcode        TEXT UNIQUE,
  name           TEXT NOT NULL,
  category       TEXT,
  cost_price     INTEGER NOT NULL DEFAULT 0,
  sell_price     INTEGER NOT NULL,
  stock_qty      INTEGER NOT NULL DEFAULT 0,
  reorder_level  INTEGER NOT NULL DEFAULT 5,
  active         INTEGER NOT NULL DEFAULT 1,
  created_at     TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  updated_at     TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_products_name     ON products(name COLLATE NOCASE);
CREATE INDEX idx_products_category ON products(category);

CREATE TABLE sales (
  id              INTEGER PRIMARY KEY,
  uid         TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
  user_id         INTEGER NOT NULL REFERENCES users(id),
  subtotal        INTEGER NOT NULL,
  discount        INTEGER NOT NULL DEFAULT 0,
  total           INTEGER NOT NULL,
  payment_method  TEXT NOT NULL CHECK (payment_method IN ('cash', 'mpesa')),
  mpesa_code      TEXT,
  cash_tendered   INTEGER,
  change_given    INTEGER,
  status          TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('completed', 'voided')),
  voided_by       INTEGER REFERENCES users(id),
  voided_at       TEXT,
  void_reason     TEXT,
  created_at      TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_sales_created ON sales(created_at);
CREATE INDEX idx_sales_status_created ON sales(status, created_at);
CREATE UNIQUE INDEX idx_sales_mpesa ON sales(mpesa_code) WHERE mpesa_code IS NOT NULL;

CREATE TABLE sale_items (
  id            INTEGER PRIMARY KEY,
  uid         TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
  sale_id       INTEGER NOT NULL REFERENCES sales(id),
  product_id    INTEGER NOT NULL REFERENCES products(id),
  product_name  TEXT NOT NULL,
  qty           INTEGER NOT NULL CHECK (qty > 0),
  unit_price    INTEGER NOT NULL,
  unit_cost     INTEGER NOT NULL,
  line_total    INTEGER NOT NULL
);
CREATE INDEX idx_sale_items_sale    ON sale_items(sale_id);
CREATE INDEX idx_sale_items_product ON sale_items(product_id);

CREATE TABLE stock_movements (
  id           INTEGER PRIMARY KEY,
  uid         TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
  product_id   INTEGER NOT NULL REFERENCES products(id),
  qty_delta    INTEGER NOT NULL,
  reason       TEXT NOT NULL CHECK (reason IN ('sale', 'void', 'purchase', 'adjustment', 'damage', 'count')),
  ref_sale_id  INTEGER REFERENCES sales(id),
  note         TEXT,
  user_id      INTEGER NOT NULL REFERENCES users(id),
  created_at   TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_stock_moves_product ON stock_movements(product_id, created_at);
CREATE INDEX idx_stock_moves_created ON stock_movements(created_at);

CREATE TABLE audit_log (
  id          INTEGER PRIMARY KEY,
  uid         TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
  user_id     INTEGER REFERENCES users(id),
  action      TEXT NOT NULL,
  entity      TEXT NOT NULL,
  entity_id   INTEGER,
  details     TEXT NOT NULL DEFAULT '{}',
  created_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_audit_created ON audit_log(created_at);

CREATE TABLE settings (
  key    TEXT PRIMARY KEY,
  value  TEXT NOT NULL
);
INSERT INTO settings (key, value) VALUES
  ('store_name', 'My Liquor Store'),
  ('receipt_footer', 'Thank you, come again.'),
  ('allow_negative_stock', '0'),
  ('sync_pulling', '0');
