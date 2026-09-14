use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection};

use crate::error::AppResult;

/// The open shop's database. A single SQLite connection with WAL mode is the
/// fastest safe setup for one machine; switching shops swaps the connection.
pub struct Db {
    conn: Mutex<Connection>,
    /// Bumped on every shop switch, so a sync pass that started for the previous
    /// shop can tell and stop before it writes anything.
    generation: AtomicU64,
}

impl Db {
    pub fn new(conn: Connection) -> Self {
        Self { conn: Mutex::new(conn), generation: AtomicU64::new(0) }
    }

    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Lock only if this is still the shop that was open at `generation`.
    pub fn lock_if(&self, generation: u64) -> Result<MutexGuard<'_, Connection>, String> {
        let guard = self.lock();
        if self.generation() != generation {
            return Err("The shop was switched, so this sync pass stopped.".into());
        }
        Ok(guard)
    }

    /// Close the open shop and use another shop's database from now on.
    pub fn replace(&self, conn: Connection) {
        let mut guard = self.lock();
        *guard = conn;
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
}

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_init.sql"),
    include_str!("../migrations/002_sync.sql"),
    include_str!("../migrations/003_store_sync.sql"),
];

pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "cache_size", -16000)?; // 16 MB page cache
    migrate(&conn)?;
    Ok(conn)
}

/// Open a shop's database, creating and preparing it on first use.
pub fn open_shop(path: &Path) -> AppResult<Connection> {
    let conn = open(path)?;
    seed(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

/// First use of a database: the owner account and this computer's identity.
fn seed(conn: &Connection) -> AppResult<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
    if count == 0 {
        let hash = crate::commands::auth::hash_pin("1234")?;
        conn.execute("INSERT INTO users (username, pin_hash, role) VALUES ('admin', ?1, 'owner')", params![hash])?;
    }
    conn.execute("INSERT OR IGNORE INTO settings (key, value) VALUES ('device_id', lower(hex(randomblob(8))))", [])?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('device_name', ?1)",
        params![computer_name().unwrap_or_else(|| "This computer".into())],
    )?;
    // A shop already connected to the cloud belongs to that cloud store from now on.
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value)
         SELECT 'bound_store', lower(rtrim(u.value, '/')) || '|' || s.value FROM settings u, settings s
         WHERE u.key = 'supabase_url' AND s.key = 'sync_store_id' AND u.value != '' AND s.value != ''",
        [],
    )?;
    Ok(())
}

/// Default name shown in the Till column on other computers. Owners can rename it in Settings.
#[cfg(target_os = "macos")]
fn computer_name() -> Option<String> {
    let out = std::process::Command::new("scutil").args(["--get", "ComputerName"]).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|n| !n.is_empty())
}

#[cfg(not(target_os = "macos"))]
fn computer_name() -> Option<String> {
    std::env::var("COMPUTERNAME").ok().filter(|n| !n.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_shops_stops_a_sync_pass_for_the_old_shop() {
        let db = Db::new(Connection::open_in_memory().unwrap());
        let started = db.generation();
        assert!(db.lock_if(started).is_ok());
        db.replace(Connection::open_in_memory().unwrap());
        assert!(db.lock_if(started).is_err(), "a pass for the previous shop must not get the new database");
        assert!(db.lock_if(db.generation()).is_ok());
    }

    #[test]
    fn an_already_connected_shop_is_bound_to_its_cloud_store() {
        let path = std::env::temp_dir().join(format!("liquorpos-bind-{}.db", std::process::id()));
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
        {
            let conn = open_shop(&path).unwrap();
            conn.execute_batch(
                "INSERT INTO settings (key, value) VALUES ('supabase_url', 'https://Abc.supabase.co/'), ('sync_store_id', 'store-1');",
            )
            .unwrap();
        }
        let conn = open_shop(&path).unwrap();
        let bound: String = conn.query_row("SELECT value FROM settings WHERE key = 'bound_store'", [], |r| r.get(0)).unwrap();
        assert_eq!(bound, "https://abc.supabase.co|store-1");
    }
}
