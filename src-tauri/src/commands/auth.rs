use std::sync::Mutex;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand_core::OsRng;
use argon2::Argon2;
use rusqlite::{params, OptionalExtension};
use tauri::State;

use crate::commands::audit;
use crate::db::Db;
use crate::error::{bad, AppResult};
use crate::models::User;

/// The currently logged-in user, held in app state so commands never trust
/// a user id sent from the UI.
#[derive(Default)]
pub struct Session(pub Mutex<Option<User>>);

pub fn require_user(session: &Session) -> AppResult<User> {
    session
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(|| bad("Please log in first"))
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

#[tauri::command]
pub fn login(db: State<Db>, session: State<Session>, username: String, pin: String) -> AppResult<User> {
    let conn = db.lock();
    let row: Option<(i64, String, String, String)> = conn
        .prepare_cached("SELECT id, username, pin_hash, role FROM users WHERE username = ?1 AND active = 1")?
        .query_row(params![username.trim()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()?;

    let Some((id, username, pin_hash, role)) = row else {
        return Err(bad("Wrong username or PIN"));
    };
    if !verify_pin(&pin, &pin_hash) {
        return Err(bad("Wrong username or PIN"));
    }

    let user = User { id, username, role, active: true };
    audit::log(&conn, Some(id), "login", "user", Some(id), serde_json::json!({}))?;
    *session.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(user.clone());
    Ok(user)
}

#[tauri::command]
pub fn logout(db: State<Db>, session: State<Session>) -> AppResult<()> {
    let mut guard = session.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(user) = guard.take() {
        let conn = db.lock();
        audit::log(&conn, Some(user.id), "logout", "user", Some(user.id), serde_json::json!({}))?;
    }
    Ok(())
}

#[tauri::command]
pub fn current_user(session: State<Session>) -> Option<User> {
    session.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
