use rusqlite::params;
use tauri::State;

use crate::commands::audit;
use crate::commands::auth::{require_owner, Session};
use crate::db::{self, Db};
use crate::error::AppResult;
use crate::shops::{Shop, ShopList, Shops};
use crate::sync::Sync;

/// Public: the login screen shows the shop picker before anyone logs in.
#[tauri::command]
pub fn list_shops(shops: State<Shops>) -> ShopList {
    shops.list()
}

/// Open another of this computer's shops. Users belong to one shop, so whoever
/// was logged in is logged out.
#[tauri::command]
pub fn open_shop(
    db: State<Db>,
    shops: State<Shops>,
    session: State<Session>,
    sync: State<Sync>,
    shop_id: String,
) -> AppResult<ShopList> {
    if shops.current().id != shop_id {
        let shop = shops.get(&shop_id)?;
        switch_to(&db, &shops, &session, &sync, &shop, None)?;
    }
    Ok(shops.list())
}

/// Owner-only. Adds a shop with its own empty database and opens it.
#[tauri::command]
pub fn add_shop(
    db: State<Db>,
    shops: State<Shops>,
    session: State<Session>,
    sync: State<Sync>,
    name: String,
) -> AppResult<ShopList> {
    let owner = require_owner(&session)?;
    let shop = shops.add(&name)?;
    audit::log(&db.lock(), Some(owner.id), "add_shop", "shop", None, serde_json::json!({ "name": shop.name }))?;
    switch_to(&db, &shops, &session, &sync, &shop, Some(&shop.name))?;
    Ok(shops.list())
}

fn switch_to(db: &Db, shops: &Shops, session: &Session, sync: &Sync, shop: &Shop, new_name: Option<&str>) -> AppResult<()> {
    let conn = db::open_shop(&shops.path_of(shop))?;
    if let Some(name) = new_name {
        conn.execute("UPDATE settings SET value = ?1 WHERE key = 'store_name'", params![name])?;
    }
    audit::log(&conn, None, "open_shop", "shop", None, serde_json::json!({ "name": shop.name }))?;
    shops.refresh(&shop.id, &conn)?;
    *session.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
    // Replacing the connection stops any sync pass still running for the previous shop.
    db.replace(conn);
    shops.set_current(&shop.id)?;
    sync.reset();
    sync.kick();
    Ok(())
}
