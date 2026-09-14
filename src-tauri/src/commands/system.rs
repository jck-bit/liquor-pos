use std::collections::HashMap;

use rusqlite::params;
use tauri::{AppHandle, Manager, State};

use crate::commands::audit;
use crate::commands::auth::{require_owner, Session};
use crate::db::Db;
use crate::error::{bad, AppResult};

const EDITABLE_SETTINGS: &[&str] =
    &["store_name", "receipt_footer", "allow_negative_stock", "store_phone", "store_address", "device_name"];

/// Settings are public (needed for the login screen title and receipts).
#[tauri::command]
pub fn get_settings(db: State<Db>) -> AppResult<HashMap<String, String>> {
    let conn = db.lock();
    let mut stmt = conn.prepare_cached(
        "SELECT key, value FROM settings
         WHERE key NOT IN ('sync_password', 'sync_access_token', 'sync_refresh_token', 'sync_pulling')
           AND key NOT LIKE 'pull_cursor:%'",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[tauri::command]
pub fn update_settings(db: State<Db>, session: State<Session>, values: HashMap<String, String>) -> AppResult<()> {
    let owner = require_owner(&session)?;
    let mut conn = db.lock();
    let tx = conn.transaction()?;
    for (key, value) in &values {
        if !EDITABLE_SETTINGS.contains(&key.as_str()) {
            return Err(bad(format!("Unknown setting {key}")));
        }
        tx.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value.trim()],
        )?;
    }
    audit::log(&tx, Some(owner.id), "update", "settings", None, serde_json::json!(values))?;
    tx.commit()?;
    Ok(())
}

/// Consistent snapshot of the database into Documents/Liquor POS/backups.
/// VACUUM INTO is safe while the app is running, even in WAL mode.
#[tauri::command]
pub fn backup_database(app: AppHandle, db: State<Db>, session: State<Session>) -> AppResult<String> {
    let owner = require_owner(&session)?;
    let dir = app.path().document_dir()?.join("Liquor POS").join("backups");
    std::fs::create_dir_all(&dir)?;
    let conn = db.lock();
    let stamp: String = conn.query_row("SELECT strftime('%Y%m%d-%H%M%S', 'now', 'localtime')", [], |r| r.get(0))?;
    let path = dir.join(format!("liquorpos-{stamp}.db"));
    conn.execute("VACUUM INTO ?1", params![path.to_string_lossy()])?;
    audit::log(&conn, Some(owner.id), "backup", "database", None, serde_json::json!({ "path": path.to_string_lossy() }))?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn database_path(app: AppHandle) -> AppResult<String> {
    Ok(app.path().app_data_dir()?.join("liquorpos.db").to_string_lossy().into_owned())
}
