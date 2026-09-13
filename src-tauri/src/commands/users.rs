use rusqlite::params;
use tauri::State;

use crate::commands::audit;
use crate::commands::auth::{hash_pin, require_owner, require_user, validate_pin, Session};
use crate::db::Db;
use crate::error::{bad, AppResult};
use crate::models::User;

#[tauri::command]
pub fn list_users(db: State<Db>, session: State<Session>) -> AppResult<Vec<User>> {
    require_owner(&session)?;
    let conn = db.lock();
    let mut stmt = conn.prepare_cached("SELECT id, username, role, active FROM users ORDER BY username")?;
    let rows = stmt.query_map([], |r| {
        Ok(User { id: r.get(0)?, username: r.get(1)?, role: r.get(2)?, active: r.get::<_, i64>(3)? != 0 })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[tauri::command]
pub fn create_user(
    db: State<Db>,
    session: State<Session>,
    username: String,
    pin: String,
    role: String,
) -> AppResult<User> {
    let owner = require_owner(&session)?;
    let username = username.trim().to_string();
    if username.len() < 2 {
        return Err(bad("Username is too short"));
    }
    if role != "owner" && role != "cashier" {
        return Err(bad("Role must be owner or cashier"));
    }
    validate_pin(&pin)?;
    let hash = hash_pin(&pin)?;

    let conn = db.lock();
    let exists: i64 = conn.query_row("SELECT COUNT(*) FROM users WHERE username = ?1", params![username], |r| r.get(0))?;
    if exists > 0 {
        return Err(bad("That username is already taken"));
    }
    conn.execute(
        "INSERT INTO users (username, pin_hash, role) VALUES (?1, ?2, ?3)",
        params![username, hash, role],
    )?;
    let id = conn.last_insert_rowid();
    audit::log(&conn, Some(owner.id), "create", "user", Some(id), serde_json::json!({ "username": username, "role": role }))?;
    Ok(User { id, username, role, active: true })
}

#[tauri::command]
pub fn set_user_active(db: State<Db>, session: State<Session>, user_id: i64, active: bool) -> AppResult<()> {
    let owner = require_owner(&session)?;
    if owner.id == user_id && !active {
        return Err(bad("You cannot deactivate yourself"));
    }
    let conn = db.lock();
    conn.execute("UPDATE users SET active = ?1 WHERE id = ?2", params![active as i64, user_id])?;
    audit::log(&conn, Some(owner.id), if active { "activate" } else { "deactivate" }, "user", Some(user_id), serde_json::json!({}))?;
    Ok(())
}

#[tauri::command]
pub fn change_pin(db: State<Db>, session: State<Session>, user_id: i64, new_pin: String) -> AppResult<()> {
    let me = require_user(&session)?;
    if me.id != user_id && me.role != "owner" {
        return Err(bad("Only the owner can change another user's PIN"));
    }
    validate_pin(&new_pin)?;
    let hash = hash_pin(&new_pin)?;
    let conn = db.lock();
    conn.execute("UPDATE users SET pin_hash = ?1 WHERE id = ?2", params![hash, user_id])?;
    audit::log(&conn, Some(me.id), "change_pin", "user", Some(user_id), serde_json::json!({}))?;
    Ok(())
}
