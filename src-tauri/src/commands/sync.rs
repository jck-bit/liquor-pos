use rusqlite::Connection;
use tauri::State;

use crate::commands::audit;
use crate::commands::auth::{require_owner, require_user, Session};
use crate::db::Db;
use crate::error::{bad, AppResult};
use crate::shops::Shops;
use crate::sync::{pending_count, set_setting, setting, store_tokens, supabase::Client, Sync, SyncStatus};

/// Owner enters the shop's Supabase project and login once. The login is checked
/// immediately so a typo is caught here, not silently in the background.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn configure_sync(
    db: State<Db>,
    shops: State<Shops>,
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

    let shop = shops.current();
    let store = store_key(&url, &tokens.user_id);
    // Two shops on one computer must never share a cloud store.
    if let Some(other) = shops.store_owner(&store, &shop.id) {
        return Err(bad(format!(
            "{name} on this computer already uses this cloud account. Log out and choose {name} on the login screen instead.",
            name = other.name
        )));
    }

    {
        let conn = db.lock();
        // A shop's sales and stock must never be mixed into another shop's cloud, or the other way round.
        if binding_conflict(&conn, &store)? {
            return Err(bad(format!(
                "{} already holds another cloud account's sales and stock, so connecting it here would mix two shops. \
                 To run another shop on this computer, add it under Settings, Shops.",
                shop.name
            )));
        }
        set_setting(&conn, "supabase_url", &url)?;
        set_setting(&conn, "supabase_anon_key", anon_key.trim())?;
        set_setting(&conn, "sync_email", email.trim())?;
        set_setting(&conn, "sync_password", &password)?;
        set_setting(&conn, "bound_store", &store)?;
        conn.execute("DELETE FROM settings WHERE key LIKE 'pull_cursor:%' OR key = 'stock_rebuilt_v2'", [])?;
        store_tokens(&conn, &tokens)?;
        audit::log(&conn, Some(owner.id), "configure", "sync", None, serde_json::json!({ "url": url, "email": email.trim() }))?;
        shops.refresh(&shop.id, &conn)?;
    }
    sync.kick();
    Ok(sync.status())
}

/// Identifies one shop's cloud data: the Supabase project plus the store's login.
pub fn store_key(url: &str, store_id: &str) -> String {
    format!("{}|{}", url.trim().trim_end_matches('/').to_lowercase(), store_id)
}

/// True when this database already holds data from a different cloud store.
/// An empty database can be connected anywhere.
pub fn binding_conflict(conn: &Connection, store: &str) -> AppResult<bool> {
    let Some(bound) = setting(conn, "bound_store") else { return Ok(false) };
    if bound == store {
        return Ok(false);
    }
    let has_data: i64 = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM products) OR EXISTS (SELECT 1 FROM sales) OR EXISTS (SELECT 1 FROM stock_movements)",
        [],
        |r| r.get(0),
    )?;
    Ok(has_data == 1)
}

#[tauri::command]
pub fn disable_sync(db: State<Db>, shops: State<Shops>, session: State<Session>, sync: State<Sync>) -> AppResult<()> {
    let owner = require_owner(&session)?;
    let conn = db.lock();
    // The shop stays bound to its cloud store, so it can only ever be reconnected to that store.
    for key in ["supabase_url", "supabase_anon_key", "sync_email", "sync_password", "sync_access_token", "sync_refresh_token", "sync_expires_at", "sync_store_id"] {
        set_setting(&conn, key, "")?;
    }
    audit::log(&conn, Some(owner.id), "disable", "sync", None, serde_json::json!({}))?;
    shops.refresh(&shops.current().id, &conn)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shop_with_data_cannot_be_connected_to_another_shops_cloud() {
        let path = std::env::temp_dir().join(format!("liquorpos-conflict-{}.db", std::process::id()));
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
        let conn = crate::db::open_shop(&path).unwrap();
        let kimbo = store_key("https://kimbo.supabase.co/", "k");
        let vintage = store_key("https://vintage.supabase.co", "v");
        assert!(!binding_conflict(&conn, &kimbo).unwrap(), "a shop never connected can connect anywhere");

        set_setting(&conn, "bound_store", &kimbo).unwrap();
        assert!(!binding_conflict(&conn, &vintage).unwrap(), "an empty database can still be pointed elsewhere");

        conn.execute("INSERT INTO products (name, sell_price) VALUES ('Tusker Lager Can', 30000)", []).unwrap();
        assert!(binding_conflict(&conn, &vintage).unwrap(), "Kimbo's data must not go to Vintage");
        assert!(!binding_conflict(&conn, &kimbo).unwrap(), "reconnecting to its own store is fine");
    }
}
