use tauri::State;

use crate::commands::audit;
use crate::commands::auth::{require_owner, require_user, Session};
use crate::db::Db;
use crate::error::{bad, AppResult};
use crate::sync::{pending_count, set_setting, store_tokens, supabase::Client, Sync, SyncStatus};

/// Owner enters the Supabase project and the store's login once. We verify the
/// login immediately so a typo is caught here, not silently in the background.
#[tauri::command]
pub fn configure_sync(
    db: State<Db>,
    session: State<Session>,
    sync: State<Sync>,
    url: String,
    anon_key: String,
    email: String,
    password: String,
) -> AppResult<SyncStatus> {
    let owner = require_owner(&session)?;
    let url = url.trim().trim_end_matches('/').to_string();
    if !url.starts_with("https://") {
        return Err(bad("Supabase URL should look like https://xxxx.supabase.co"));
    }
    let client = Client::new(&url, anon_key.trim())?;
    client.check_project()?;
    let tokens = client.password_login(email.trim(), &password).map_err(|e| bad(format!("Login failed: {e}")))?;

    {
        let conn = db.lock();
        set_setting(&conn, "supabase_url", &url)?;
        set_setting(&conn, "supabase_anon_key", anon_key.trim())?;
        set_setting(&conn, "sync_email", email.trim())?;
        set_setting(&conn, "sync_password", &password)?;
        conn.execute("DELETE FROM settings WHERE key LIKE 'pull_cursor:%' OR key = 'stock_rebuilt_v2'", [])?;
        store_tokens(&conn, &tokens)?;
        audit::log(&conn, Some(owner.id), "configure", "sync", None, serde_json::json!({ "url": url, "email": email.trim() }))?;
    }
    sync.kick();
    Ok(sync.status())
}

#[tauri::command]
pub fn disable_sync(db: State<Db>, session: State<Session>, sync: State<Sync>) -> AppResult<()> {
    let owner = require_owner(&session)?;
    let conn = db.lock();
    for key in ["supabase_url", "supabase_anon_key", "sync_email", "sync_password", "sync_access_token", "sync_refresh_token", "sync_expires_at", "sync_store_id"] {
        set_setting(&conn, key, "")?;
    }
    audit::log(&conn, Some(owner.id), "disable", "sync", None, serde_json::json!({}))?;
    drop(conn);
    sync.kick();
    Ok(())
}

#[tauri::command]
pub fn sync_now(session: State<Session>, sync: State<Sync>) -> AppResult<()> {
    require_user(&session)?;
    sync.kick();
    Ok(())
}

#[tauri::command]
pub fn sync_status(db: State<Db>, sync: State<Sync>) -> SyncStatus {
    let mut s = sync.status();
    s.pending = pending_count(&db.lock());
    s
}
