// 抑制 vendored 的 mac_rounded_corners 插件中 objc 0.2 宏触发的 cfg 警告。
// 该警告来自上游 objc crate，与本项目代码无关，无法在插件源码侧修复。
#![allow(unexpected_cfgs)]

mod commands;
mod db;
mod library;
mod parser;
mod plugins;

use db::Database;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 初始化数据库，作为托管状态注入到所有命令中。
            let db_path = library::db_path().expect("无法定位数据库路径");
            let db = Database::open(&db_path).expect("无法打开数据库");
            app.manage(db);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::import_files,
            commands::list_documents,
            commands::read_document_content,
            commands::delete_document,
            commands::get_library_dir,
            plugins::mac_rounded_corners::enable_rounded_corners,
            plugins::mac_rounded_corners::enable_modern_window_style,
            plugins::mac_rounded_corners::reposition_traffic_lights,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
