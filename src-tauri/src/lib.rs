mod commands;
mod db;
mod error;
mod models;
mod sync;

use std::sync::Mutex;

use rusqlite::params;
use tauri::Manager;

use commands::auth::Session;

/// First run only: create the owner account so the store can log in.
fn seed_default_owner(conn: &rusqlite::Connection) -> error::AppResult<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
    if count == 0 {
        let hash = commands::auth::hash_pin("1234")?;
        conn.execute(
            "INSERT INTO users (username, pin_hash, role) VALUES ('admin', ?1, 'owner')",
            params![hash],
        )?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('device_id', lower(hex(randomblob(8))))",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('device_name', ?1)",
        params![computer_name().unwrap_or_else(|| "This computer".into())],
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open(&dir.join("liquorpos.db"))?;
            seed_default_owner(&conn)?;
            commands::audit::log(
                &conn,
                None,
                "app_start",
                "app",
                None,
                serde_json::json!({ "version": app.package_info().version.to_string() }),
            )?;
            app.manage(db::Db(Mutex::new(conn)));
            app.manage(Session::default());
            sync::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::login,
            commands::auth::logout,
            commands::auth::current_user,
            commands::users::list_users,
            commands::users::create_user,
            commands::users::set_user_active,
            commands::users::change_pin,
            commands::products::list_products,
            commands::products::find_product_by_barcode,
            commands::products::list_categories,
            commands::products::save_product,
            commands::products::set_product_active,
            commands::products::import_products,
            commands::stock::receive_stock,
            commands::stock::adjust_stock,
            commands::stock::list_stock_movements,
            commands::stock::low_stock_products,
            commands::sales::create_sale,
            commands::sales::get_sale,
            commands::sales::list_sales,
            commands::sales::void_sale,
            commands::reports::sales_summary,
            commands::reports::daily_sales,
            commands::reports::top_products,
            commands::reports::stock_value,
            commands::reports::export_sales_csv,
            commands::audit::list_audit,
            commands::system::get_settings,
            commands::system::update_settings,
            commands::system::backup_database,
            commands::system::database_path,
            commands::sync::configure_sync,
            commands::sync::disable_sync,
            commands::sync::sync_now,
            commands::sync::sync_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Liquor POS");
}
