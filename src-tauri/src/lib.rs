// 抑制 vendored 的 mac_rounded_corners 插件中 objc 0.2 宏触发的 cfg 警告。
// 该警告来自上游 objc crate，与本项目代码无关，无法在插件源码侧修复。
#![allow(unexpected_cfgs)]

mod commands;
mod db;
mod inliner;
mod library;
mod parser;
mod plugins;
mod vault;

use db::Database;
use tauri::Manager;

#[cfg(target_os = "windows")]
use tauri_plugin_frame::FramePluginBuilder;

/// 所有命令列表（含平台相关命令，由 #[cfg] 控制）。
macro_rules! app_commands {
    () => {
        tauri::generate_handler![
            commands::set_vault_root,
            commands::get_vault_info,
            commands::list_vault_files,
            commands::list_vault_dirs,
            commands::read_file_content,
            commands::read_file_inlined,
            commands::write_file_content,
            commands::create_vault_dir,
            commands::rename_vault_item,
            commands::delete_vault_item,
            commands::move_vault_file,
            commands::move_vault_dir,
            commands::reveal_in_finder,
            commands::get_file_abs_path,
            #[cfg(target_os = "macos")]
            plugins::mac_rounded_corners::enable_rounded_corners,
            #[cfg(target_os = "macos")]
            plugins::mac_rounded_corners::enable_modern_window_style,
            #[cfg(target_os = "macos")]
            plugins::mac_rounded_corners::reposition_traffic_lights,
        ]
    };
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg_attr(not(target_os = "windows"), allow(unused_mut))]
    let mut builder = tauri::Builder::default();

    #[cfg(target_os = "windows")]
    {
        builder = builder.plugin(
            FramePluginBuilder::new()
                .titlebar_height(30)
                .button_width(46)
                .auto_titlebar(true)
                .snap_overlay(true)
                .close_hover_bg("rgba(196,85,77,1)")
                .button_hover_bg_light("rgba(55,53,47,0.06)")
                .button_hover_bg_dark("rgba(255,255,255,0.1)")
                .build(),
        );
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            // 初始化数据库
            let db_path = library::db_path().expect("无法定位数据库路径");
            let db = Database::open(&db_path).expect("无法打开数据库");

            // 尝试从 DB 中的仓库配置恢复上次的仓库（扫描 + 监听）。
            let _ = commands::init_vault_from_db(&db, app.handle());

            app.manage(db);
            Ok(())
        })
        .invoke_handler(app_commands!())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
