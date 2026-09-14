use rusqlite::{params, Connection};
use tauri::State;

use crate::commands::auth::{require_owner, Session};
use crate::db::Db;
use crate::error::AppResult;
use crate::models::AuditEntry;

/// Append one row to the audit log. Called inside the same transaction as the
/// change it describes, so the log can never disagree with the data.
pub fn log(
    conn: &Connection,
    user_id: Option<i64>,
    action: &str,
    entity: &str,
    entity_id: Option<i64>,
    details: serde_json::Value,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO audit_log (user_id, action, entity, entity_id, details) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![user_id, action, entity, entity_id, details.to_string()])?;
    Ok(())
}

#[tauri::command]
pub fn list_audit(
    db: State<Db>,
    session: State<Session>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<Vec<AuditEntry>> {
    require_owner(&session)?;
    let conn = db.lock();
    let mut stmt = conn.prepare_cached(
        "SELECT a.id, u.username, a.action, a.entity, a.entity_id, a.details, a.created_at,
                CASE WHEN a.origin_device IS NULL THEN NULL ELSE COALESCE((SELECT name FROM devices WHERE device_id = a.origin_device), 'Another computer') END
         FROM audit_log a LEFT JOIN users u ON u.id = a.user_id
         ORDER BY a.created_at DESC, a.id DESC LIMIT ?1 OFFSET ?2",
    )?;
    let rows = stmt.query_map(params![limit.unwrap_or(200), offset.unwrap_or(0)], |r| {
        Ok(AuditEntry {
            id: r.get(0)?,
            user: r.get(1)?,
            action: r.get(2)?,
            entity: r.get(3)?,
            entity_id: r.get(4)?,
            details: r.get(5)?,
            created_at: r.get(6)?,
            till: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
