mod commands;
mod db;
mod error;
mod models;
mod shops;
mod sync;

use tauri::Manager;

use commands::auth::Session;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            // Open the shop that was used last. A till has exactly one.
            let shops = shops::Shops::load(&dir)?;
            let shop = shops.current();
            let conn = db::open_shop(&shops.path_of(&shop))?;
            shops.refresh(&shop.id, &conn)?;
            commands::audit::log(
                &conn,
                None,
                "app_start",
                "app",
                None,
                serde_json::json!({ "version": app.package_info().version.to_string() }),
            )?;
            app.manage(db::Db::new(conn));
            app.manage(shops);
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
            commands::shops::list_shops,
            commands::shops::open_shop,
            commands::shops::add_shop,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Liquor POS");
}
