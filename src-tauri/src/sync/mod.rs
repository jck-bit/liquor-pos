//! Offline-first cloud sync.
//!
//! SQLite is the source of truth. Triggers (migrations/002_sync.sql) copy every
//! insert/update into `sync_outbox`; a background thread pushes that queue to
//! Supabase whenever it can, then pulls product edits made in the cloud.

pub mod supabase;

use std::collections::BTreeMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::db::Db;
use supabase::{Client, Tokens};

/// Push order matters only for readability in the cloud; there are no FKs there.
const PUSH_ORDER: &[&str] = &["users", "products", "sales", "sale_items", "stock_movements", "audit_log"];
const BATCH: i64 = 300;
const INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub configured: bool,
    pub connected: bool,
    pub syncing: bool,
    pub pending: i64,
    pub email: Option<String>,
    pub last_ok: Option<String>,
    pub last_error: Option<String>,
}

pub struct Sync {
    tx: Mutex<Sender<()>>,
    status: Mutex<SyncStatus>,
}

impl Sync {
    pub fn kick(&self) {
        let _ = self.tx.lock().unwrap_or_else(|e| e.into_inner()).send(());
    }
    pub fn status(&self) -> SyncStatus {
        self.status.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn update(&self, f: impl FnOnce(&mut SyncStatus)) {
        f(&mut self.status.lock().unwrap_or_else(|e| e.into_inner()));
    }
}

pub fn start(app: AppHandle) {
    let (tx, rx) = channel::<()>();
    app.manage(Sync { tx: Mutex::new(tx), status: Mutex::new(SyncStatus::default()) });
    std::thread::Builder::new()
        .name("cloud-sync".into())
        .spawn(move || worker(app, rx))
        .expect("spawn sync thread");
}

fn worker(app: AppHandle, rx: Receiver<()>) {
    // First run shortly after startup, then every INTERVAL or whenever kicked.
    let _ = rx.recv_timeout(Duration::from_secs(3));
    loop {
        run_once(&app);
        let _ = rx.recv_timeout(INTERVAL);
    }
}

// ---------- settings helpers ----------

pub fn setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
        .optional()
        .ok()
        .flatten()
        .filter(|v: &String| !v.is_empty())
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn pending_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM sync_outbox", [], |r| r.get(0)).unwrap_or(0)
}

struct Config {
    url: String,
    anon: String,
    email: String,
    password: String,
    tokens: Option<Tokens>,
    device_id: String,
    last_pull: String,
}

fn load_config(conn: &Connection) -> Option<Config> {
    let url = setting(conn, "supabase_url")?;
    let anon = setting(conn, "supabase_anon_key")?;
    let email = setting(conn, "sync_email")?;
    let password = setting(conn, "sync_password")?;
    let tokens = match (setting(conn, "sync_access_token"), setting(conn, "sync_refresh_token"), setting(conn, "sync_store_id")) {
        (Some(access), Some(refresh), Some(user_id)) => Some(Tokens {
            access,
            refresh,
            expires_at: setting(conn, "sync_expires_at").and_then(|v| v.parse().ok()).unwrap_or(0),
            user_id,
        }),
        _ => None,
    };
    Some(Config {
        url,
        anon,
        email,
        password,
        tokens,
        device_id: setting(conn, "device_id").unwrap_or_else(|| "unknown".into()),
        last_pull: setting(conn, "sync_last_pull").unwrap_or_else(|| "1970-01-01 00:00:00".into()),
    })
}

pub fn store_tokens(conn: &Connection, t: &Tokens) -> rusqlite::Result<()> {
    set_setting(conn, "sync_access_token", &t.access)?;
    set_setting(conn, "sync_refresh_token", &t.refresh)?;
    set_setting(conn, "sync_expires_at", &t.expires_at.to_string())?;
    set_setting(conn, "sync_store_id", &t.user_id)
}

// ---------- the sync pass ----------

fn run_once(app: &AppHandle) {
    let db = app.state::<Db>();
    let sync = app.state::<Sync>();

    let cfg = {
        let conn = db.lock();
        let cfg = load_config(&conn);
        let pending = pending_count(&conn);
        sync.update(|s| {
            s.configured = cfg.is_some();
            s.pending = pending;
            s.email = cfg.as_ref().map(|c| c.email.clone());
        });
        match cfg {
            Some(c) => c,
            None => return,
        }
    };

    sync.update(|s| s.syncing = true);
    let version = app.package_info().version.to_string();
    let result = sync_pass(&db, &cfg, &version);
    let pending = pending_count(&db.lock());
    sync.update(|s| {
        s.syncing = false;
        s.pending = pending;
        match &result {
            Ok(()) => {
                s.connected = true;
                s.last_error = None;
                s.last_ok = Some(local_now(&db.lock()));
            }
            Err(e) => {
                s.connected = false;
                s.last_error = Some(e.clone());
            }
        }
    });
    if let Ok(()) = result {
        let conn = db.lock();
        let _ = set_setting(&conn, "sync_last_ok", &local_now(&conn));
    }
}

fn local_now(conn: &Connection) -> String {
    conn.query_row("SELECT datetime('now', 'localtime')", [], |r| r.get(0)).unwrap_or_default()
}

fn sync_pass(db: &Db, cfg: &Config, version: &str) -> Result<(), String> {
    let client = Client::new(&cfg.url, &cfg.anon)?;
    let tokens = ensure_tokens(db, &client, cfg)?;
    let pushed = push(db, &client, &tokens, cfg);
    // Heartbeat goes out even if the push failed, so the owner can see a till
    // that is online but stuck. Its own failure never blocks the sync.
    let _ = heartbeat(db, &client, &tokens, cfg, version, pushed.is_ok());
    pushed?;
    pull_products(db, &client, &tokens, cfg)?;
    Ok(())
}

/// One row per till in `devices`: last seen, queue length, last receipt, app version.
fn heartbeat(db: &Db, client: &Client, tokens: &Tokens, cfg: &Config, version: &str, push_ok: bool) -> Result<(), String> {
    let (now, pending, last_sale, last_user): (String, i64, i64, Option<String>) = {
        let conn = db.lock();
        let now = local_now(&conn);
        let pending = pending_count(&conn);
        let last_sale: i64 = conn.query_row("SELECT COALESCE(MAX(id), 0) FROM sales", [], |r| r.get(0)).unwrap_or(0);
        let last_user: Option<String> = conn
            .query_row(
                "SELECT u.username FROM audit_log a JOIN users u ON u.id = a.user_id WHERE a.action = 'login' ORDER BY a.id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten();
        (now, pending, last_sale, last_user)
    };
    let row = serde_json::json!({
        "device_id": cfg.device_id,
        "store_id": tokens.user_id,
        "last_seen": now,
        "pending": pending,
        "push_ok": push_ok,
        "last_sale_local_id": last_sale,
        "last_user": last_user,
        "app_version": version,
    });
    client.upsert_on(&tokens.access, "devices", "device_id", &[row])
}

fn ensure_tokens(db: &Db, client: &Client, cfg: &Config) -> Result<Tokens, String> {
    if let Some(t) = &cfg.tokens {
        if !supabase::expiring_soon(t.expires_at) {
            return Ok(t.clone());
        }
        if let Ok(fresh) = client.refresh(&t.refresh) {
            store_tokens(&db.lock(), &fresh).map_err(|e| e.to_string())?;
            return Ok(fresh);
        }
    }
    let fresh = client.password_login(&cfg.email, &cfg.password)?;
    store_tokens(&db.lock(), &fresh).map_err(|e| e.to_string())?;
    Ok(fresh)
}

fn push(db: &Db, client: &Client, tokens: &Tokens, cfg: &Config) -> Result<(), String> {
    loop {
        // Read a batch, then release the lock before any network call.
        let rows: Vec<(i64, String, String, String)> = {
            let conn = db.lock();
            let mut stmt = conn
                .prepare_cached("SELECT id, table_name, uid, payload FROM sync_outbox ORDER BY id LIMIT ?1")
                .map_err(|e| e.to_string())?;
            let it = stmt
                .query_map(params![BATCH], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .map_err(|e| e.to_string())?;
            it.collect::<Result<_, _>>().map_err(|e| e.to_string())?
        };
        if rows.is_empty() {
            return Ok(());
        }

        // Latest payload wins per (table, uid); PostgREST rejects duplicate keys in one upsert.
        let mut by_table: BTreeMap<&str, BTreeMap<String, Value>> = BTreeMap::new();
        for (_, table, uid, payload) in &rows {
            let mut v: Value = serde_json::from_str(payload).map_err(|e| format!("Bad outbox payload: {e}"))?;
            if let Value::Object(map) = &mut v {
                map.insert("store_id".into(), Value::String(tokens.user_id.clone()));
                map.insert("device_id".into(), Value::String(cfg.device_id.clone()));
            }
            by_table.entry(table.as_str()).or_default().insert(uid.clone(), v);
        }
        for table in PUSH_ORDER {
            if let Some(items) = by_table.get(table) {
                let batch: Vec<Value> = items.values().cloned().collect();
                client.upsert(&tokens.access, table, &batch)?;
            }
        }

        let max_id = rows.iter().map(|r| r.0).max().unwrap_or(0);
        db.lock()
            .execute("DELETE FROM sync_outbox WHERE id <= ?1", params![max_id])
            .map_err(|e| e.to_string())?;
    }
}

/// Bring down product edits made in the cloud (name, prices, barcode, category,
/// reorder level, active). Stock quantity is owned by the till and never pulled
/// for products the till already knows.
fn pull_products(db: &Db, client: &Client, tokens: &Tokens, cfg: &Config) -> Result<(), String> {
    let query = format!(
        "products?select=uid,barcode,name,category,cost_price,sell_price,stock_qty,reorder_level,active,updated_at\
         &store_id=eq.{}&updated_at=gt.{}&order=updated_at.asc&limit=500",
        tokens.user_id,
        urlencode(&cfg.last_pull)
    );
    let rows = client.select(&tokens.access, &query)?;
    if rows.is_empty() {
        return Ok(());
    }

    let mut conn = db.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    set_setting(&tx, "sync_pulling", "1").map_err(|e| e.to_string())?;
    let mut newest = cfg.last_pull.clone();
    let result: Result<(), String> = (|| {
        for r in &rows {
            let s = |k: &str| r.get(k).and_then(Value::as_str).map(str::to_string);
            let n = |k: &str| r.get(k).and_then(Value::as_i64).unwrap_or(0);
            let uid = s("uid").ok_or("product without uid")?;
            let updated_at = s("updated_at").unwrap_or_default().replace('T', " ");
            let updated_at = updated_at.split('.').next().unwrap_or(&updated_at).to_string();
            let active = r.get("active").and_then(Value::as_bool).unwrap_or(true) as i64;
            let changed = tx
                .execute(
                    "UPDATE products SET barcode = ?1, name = ?2, category = ?3, cost_price = ?4, sell_price = ?5,
                        reorder_level = ?6, active = ?7, updated_at = ?8
                     WHERE uid = ?9 AND updated_at < ?8",
                    params![s("barcode"), s("name"), s("category"), n("cost_price"), n("sell_price"), n("reorder_level"), active, updated_at, uid],
                )
                .map_err(|e| e.to_string())?;
            if changed == 0 {
                let exists: bool = tx
                    .query_row("SELECT 1 FROM products WHERE uid = ?1", params![uid], |_| Ok(true))
                    .optional()
                    .map_err(|e| e.to_string())?
                    .unwrap_or(false);
                if !exists {
                    tx.execute(
                        "INSERT INTO products (uid, barcode, name, category, cost_price, sell_price, stock_qty, reorder_level, active, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![uid, s("barcode"), s("name"), s("category"), n("cost_price"), n("sell_price"), n("stock_qty"), n("reorder_level"), active, updated_at],
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
            if updated_at > newest {
                newest = updated_at;
            }
        }
        Ok(())
    })();
    set_setting(&tx, "sync_pulling", "0").map_err(|e| e.to_string())?;
    result?;
    set_setting(&tx, "sync_last_pull", &newest).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

fn urlencode(s: &str) -> String {
    s.replace(' ', "%20").replace(':', "%3A").replace('+', "%2B")
}
