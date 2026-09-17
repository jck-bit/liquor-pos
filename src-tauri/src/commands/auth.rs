use std::sync::Mutex;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand_core::OsRng;
use argon2::Argon2;
use rusqlite::{params, Connection, OptionalExtension};
use tauri::State;

use crate::commands::audit;
use crate::db::Db;
use crate::error::{bad, AppResult};
use crate::models::User;
use crate::shops::{Shop, Shops};
use crate::sync::Sync;

/// Who is logged in, held in app state so commands never trust a user id sent
/// from the UI, plus the shops on this computer that this login has proved it owns.
pub struct Login {
    pub user: User,
    /// Shop ids where this username and PIN belong to an active owner (and the shop logged into).
    pub unlocked: Vec<String>,
}

#[derive(Default)]
pub struct Session(pub Mutex<Option<Login>>);

impl Session {
    fn guard(&self) -> std::sync::MutexGuard<'_, Option<Login>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn set(&self, login: Option<Login>) {
        *self.guard() = login;
    }
    pub fn unlocked(&self) -> Vec<String> {
        self.guard().as_ref().map(|l| l.unlocked.clone()).unwrap_or_default()
    }
}

pub fn require_user(session: &Session) -> AppResult<User> {
    session.guard().as_ref().map(|l| l.user.clone()).ok_or_else(|| bad("Please log in first"))
}

pub fn require_owner(session: &Session) -> AppResult<User> {
    let user = require_user(session)?;
    if user.role != "owner" {
        return Err(bad("Only the owner can do this"));
    }
    Ok(user)
}

pub fn hash_pin(pin: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| bad(format!("Could not hash PIN: {e}")))
}

pub fn verify_pin(pin: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(pin.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

pub fn validate_pin(pin: &str) -> AppResult<()> {
    if pin.len() < 4 || pin.len() > 8 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(bad("PIN must be 4 to 8 digits"));
    }
    Ok(())
}

/// The active user with this name in one shop's database, if the PIN is right.
fn check(conn: &Connection, username: &str, pin: &str) -> AppResult<Option<User>> {
    let row: Option<(i64, String, String, String)> = conn
        .prepare_cached("SELECT id, username, pin_hash, role FROM users WHERE username = ?1 AND active = 1")?
        .query_row(params![username], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()?;
    Ok(row
        .filter(|(_, _, hash, _)| verify_pin(pin, hash))
        .map(|(id, username, _, role)| User { id, username, role, active: true }))
}

/// The active owner with this name in a shop, used when an owner moves between their shops.
pub fn active_owner(conn: &Connection, username: &str) -> AppResult<Option<User>> {
    let row: Option<(i64, String)> = conn
        .prepare_cached("SELECT id, username FROM users WHERE username = ?1 AND role = 'owner' AND active = 1")?
        .query_row(params![username], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    Ok(row.map(|(id, username)| User { id, username, role: "owner".into(), active: true }))
}

/// Every shop on this computer where this username and PIN are valid. `open` is
/// the connection of the shop that is open now; the others are opened briefly.
fn matching_shops(shops: &Shops, current_id: &str, open: &Connection, username: &str, pin: &str) -> AppResult<Vec<(Shop, User)>> {
    let mut matches = Vec::new();
    for shop in shops.all() {
        let found = if shop.id == current_id {
            check(open, username, pin)?
        } else {
            // A shop whose file cannot be opened is skipped, not an error for everyone else.
            match crate::db::open_existing(&shops.path_of(&shop)) {
                Ok(conn) => check(&conn, username, pin)?,
                Err(_) => None,
            }
        };
        if let Some(user) = found {
            matches.push((shop, user));
        }
    }
    Ok(matches)
}

/// Which shop to open, as whom, and which shops the login unlocks: the shop
/// used last if the login is valid there, otherwise the first shop it is valid in.
fn choose(matches: &[(Shop, User)], current_id: &str) -> Option<(Shop, User, Vec<String>)> {
    let (shop, user) = matches.iter().find(|(s, _)| s.id == current_id).or_else(|| matches.first())?.clone();
    let mut unlocked: Vec<String> = matches.iter().filter(|(_, u)| u.role == "owner").map(|(s, _)| s.id.clone()).collect();
    if !unlocked.contains(&shop.id) {
        unlocked.push(shop.id.clone());
    }
    Some((shop, user, unlocked))
}

/// Nobody chooses a shop on the login screen. The username and PIN are tried in
/// every shop on this computer and the shop they belong to is opened. A cashier
/// therefore only ever reaches their own shop; an owner with the same login in
/// several shops lands in the one used last and may then move between them.
#[tauri::command(async)]
pub fn login(
    db: State<Db>,
    shops: State<Shops>,
    session: State<Session>,
    sync: State<Sync>,
    username: String,
    pin: String,
) -> AppResult<User> {
    let current = shops.current();
    let matches = matching_shops(&shops, &current.id, &db.lock(), username.trim(), &pin)?;
    let Some((shop, user, unlocked)) = choose(&matches, &current.id) else {
        return Err(bad("Wrong username or PIN"));
    };

    if shop.id != current.id {
        crate::commands::shops::switch_to(&db, &shops, &session, &sync, &shop, None)?;
    }
    audit::log(&db.lock(), Some(user.id), "login", "user", Some(user.id), serde_json::json!({}))?;
    session.set(Some(Login { user: user.clone(), unlocked }));
    Ok(user)
}

#[tauri::command]
pub fn logout(db: State<Db>, session: State<Session>) -> AppResult<()> {
    let login = session.guard().take();
    if let Some(Login { user, .. }) = login {
        let conn = db.lock();
        audit::log(&conn, Some(user.id), "logout", "user", Some(user.id), serde_json::json!({}))?;
    }
    Ok(())
}

#[tauri::command]
pub fn current_user(session: State<Session>) -> Option<User> {
    require_user(&session).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_login_decides_the_shop() {
        let dir = std::env::temp_dir().join(format!("liquorpos-login-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let shops = Shops::load(&dir).unwrap();
        let first = shops.current();
        let second = shops.add("Second shop").unwrap();

        // Both shops start with admin / 1234. The second also has a cashier, and later its own admin PIN.
        let first_db = crate::db::open_shop(&shops.path_of(&first)).unwrap();
        let second_db = crate::db::open_shop(&shops.path_of(&second)).unwrap();
        second_db
            .execute("INSERT INTO users (username, pin_hash, role) VALUES ('cashier two', ?1, 'cashier')", params![hash_pin("2468").unwrap()])
            .unwrap();
        let login = |name: &str, pin: &str| choose(&matching_shops(&shops, &first.id, &first_db, name, pin).unwrap(), &first.id);

        let (shop, user, unlocked) = login("Admin", "1234").unwrap();
        assert_eq!((shop.id.as_str(), user.role.as_str()), (first.id.as_str(), "owner"), "an owner lands in the shop used last");
        assert_eq!(unlocked, vec![first.id.clone(), second.id.clone()], "the same login in both shops unlocks both");

        let (shop, user, unlocked) = login("cashier two", "2468").unwrap();
        assert_eq!((shop.id.as_str(), user.role.as_str()), (second.id.as_str(), "cashier"), "a cashier reaches only their own shop");
        assert_eq!(unlocked, vec![second.id.clone()]);

        assert!(login("admin", "0000").is_none());
        assert!(login("nobody", "1234").is_none());

        // A different owner PIN in the second shop: each PIN opens its own shop and nothing else.
        second_db.execute("UPDATE users SET pin_hash = ?1 WHERE username = 'admin'", params![hash_pin("9999").unwrap()]).unwrap();
        let (shop, _, unlocked) = login("admin", "9999").unwrap();
        assert_eq!(shop.id, second.id);
        assert_eq!(unlocked, vec![second.id.clone()]);
        assert_eq!(login("admin", "1234").unwrap().2, vec![first.id.clone()]);

        // An account without a PIN on this computer (synced from a till) cannot log in here.
        second_db.execute("INSERT INTO users (username, pin_hash, role) VALUES ('from a till', '', 'cashier')", []).unwrap();
        assert!(login("from a till", "").is_none());
    }
}
