use rusqlite::params;
use tauri::State;

use crate::commands::audit;
use crate::commands::auth::{active_owner, require_owner, require_user, Login, Session};
use crate::db::{self, Db};
use crate::error::{bad, AppResult};
use crate::shops::{Shop, ShopList, Shops};
use crate::sync::Sync;

/// What the logged-in person may know about the shops on this computer.
/// Before login: nothing but whether there is more than one. A cashier: only
/// their own shop. An owner: every shop, marked with whether their login opens it.
#[tauri::command]
pub fn list_shops(shops: State<Shops>, session: State<Session>) -> ShopList {
    visible_shops(&shops, &session)
}

fn visible_shops(shops: &Shops, session: &Session) -> ShopList {
    let mut list = shops.list();
    match require_user(session) {
        Err(_) => list.shops.clear(),
        Ok(user) if user.role != "owner" => list.shops.retain(|s| s.id == list.current),
        Ok(_) => {
            let unlocked = session.unlocked();
            for shop in &mut list.shops {
                shop.unlocked = unlocked.contains(&shop.id);
            }
        }
    }
    list
}

/// An owner moves to another of their shops without logging out. Only shops
/// that the login unlocked (same username and PIN, owner there too) can be entered.
#[tauri::command]
pub fn enter_shop(
    db: State<Db>,
    shops: State<Shops>,
    session: State<Session>,
    sync: State<Sync>,
    shop_id: String,
) -> AppResult<ShopList> {
    let me = require_owner(&session)?;
    if shops.is_current(&shop_id) {
        return Ok(visible_shops(&shops, &session));
    }
    let shop = shops.get(&shop_id)?;
    let unlocked = session.unlocked();
    let locked = || bad(format!("Your login does not open {0}. Log out and sign in with {0}'s owner username and PIN.", shop.name));
    if !unlocked.contains(&shop.id) {
        return Err(locked());
    }
    // Find the account before switching, so a failure leaves the current shop open and logged in.
    let user = active_owner(&db::open_existing(&shops.path_of(&shop))?, &me.username)?.ok_or_else(locked)?;

    switch_to(&db, &shops, &session, &sync, &shop, None)?;
    audit::log(&db.lock(), Some(user.id), "login", "user", Some(user.id), serde_json::json!({ "from": "switch shop" }))?;
    session.set(Some(Login { user, unlocked }));
    Ok(visible_shops(&shops, &session))
}

/// Owner-only. Adds a shop with its own empty database, carries the owner's
/// login into it, and opens it. No code or configuration change is needed for a
/// new shop: it appears in Switch shop and on the All shops screen straight away.
#[tauri::command]
pub fn add_shop(
    db: State<Db>,
    shops: State<Shops>,
    session: State<Session>,
    sync: State<Sync>,
    name: String,
) -> AppResult<ShopList> {
    let owner = require_owner(&session)?;
    let mut unlocked = session.unlocked();
    let pin_hash: String =
        db.lock().query_row("SELECT pin_hash FROM users WHERE id = ?1", params![owner.id], |r| r.get(0))?;

    let shop = shops.add(&name)?;
    audit::log(&db.lock(), Some(owner.id), "add_shop", "shop", None, serde_json::json!({ "name": shop.name }))?;
    switch_to(&db, &shops, &session, &sync, &shop, Some(&shop.name))?;

    // The new database starts with a default admin account. Replace it with this
    // owner's own username and PIN, so there is never a shop with a well-known PIN.
    let user = {
        let conn = db.lock();
        conn.execute(
            "UPDATE users SET username = ?1, pin_hash = ?2, role = 'owner', active = 1 WHERE id = (SELECT MIN(id) FROM users)",
            params![owner.username, pin_hash],
        )?;
        active_owner(&conn, &owner.username)?.ok_or_else(|| bad("Could not set up the owner in the new shop"))?
    };
    audit::log(&db.lock(), Some(user.id), "login", "user", Some(user.id), serde_json::json!({ "from": "add shop" }))?;
    unlocked.push(shop.id.clone());
    session.set(Some(Login { user, unlocked }));
    Ok(visible_shops(&shops, &session))
}

/// Close the open shop and open another. Whoever was logged in is logged out;
/// callers that carry a login across set it again afterwards.
pub(crate) fn switch_to(
    db: &Db,
    shops: &Shops,
    session: &Session,
    sync: &Sync,
    shop: &Shop,
    new_name: Option<&str>,
) -> AppResult<()> {
    let conn = db::open_shop(&shops.path_of(shop))?;
    if let Some(name) = new_name {
        conn.execute("UPDATE settings SET value = ?1 WHERE key = 'store_name'", params![name])?;
    }
    audit::log(&conn, None, "open_shop", "shop", None, serde_json::json!({ "name": shop.name }))?;
    shops.refresh(&shop.id, &conn)?;
    session.set(None);
    // Replacing the connection stops any sync pass still running for the previous shop.
    db.replace(conn);
    shops.set_current(&shop.id)?;
    sync.reset();
    sync.kick();
    Ok(())
}
