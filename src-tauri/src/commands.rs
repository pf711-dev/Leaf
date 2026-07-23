use std::path::PathBuf;

use crate::db::Database;
use crate::vault;

// ==================== 仓库配置 ====================

/// 设定仓库根目录，持久化配置，触发扫描 + 监听。
#[tauri::command]
pub fn set_vault_root(
    db: tauri::State<'_, Database>,
    app: tauri::AppHandle,
    root_path: String,
) -> Result<(), String> {
    let root = PathBuf::from(&root_path);
    if !root.is_dir() {
        return Err("所选路径不是文件夹".to_string());
    }

    let name = root
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "我的仓库".to_string());

    // 持久化到数据库
    db.set_vault_config(&root_path, &name)
        .map_err(|e| e.to_string())?;

    // 初始化仓库：扫描 + 监听
    vault::init(&root, &app)?;

    Ok(())
}

/// 返回仓库信息。若配置的目录已不存在，自动清除配置并返回 None。
#[tauri::command]
pub fn get_vault_info(db: tauri::State<'_, Database>) -> Result<Option<serde_json::Value>, String> {
    match db.get_vault_config().map_err(|e| e.to_string())? {
        Some((root_path, name)) => {
            let root = PathBuf::from(&root_path);
            if !root.is_dir() {
                // 目录已被删除，清除过期配置
                db.clear_vault_config().map_err(|e| e.to_string())?;
                return Ok(None);
            }
            let mut map = serde_json::Map::new();
            map.insert("rootPath".to_string(), serde_json::Value::String(root_path));
            map.insert("name".to_string(), serde_json::Value::String(name));
            Ok(Some(serde_json::Value::Object(map)))
        }
        None => Ok(None),
    }
}

/// 启动时初始化仓库（如果 DB 中已存储 path）。
pub fn init_vault_from_db(
    db: &Database,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    if let Some((root_path, _)) = db.get_vault_config().map_err(|e| e.to_string())? {
        let root = PathBuf::from(&root_path);
        if root.is_dir() {
            vault::init(&root, app)?;
        }
    }
    Ok(())
}

// ==================== 文件列表 ====================

/// 返回仓库中所有已索引的 HTML 文件。
#[tauri::command]
pub fn list_vault_files() -> Vec<vault::VaultFile> {
    vault::list_files()
}

/// 返回仓库中所有目录。
#[tauri::command]
pub fn list_vault_dirs() -> Vec<vault::VaultDir> {
    vault::list_dirs()
}

// ==================== 文件读写 ====================

/// 读取文件内容（通过相对路径）。
#[tauri::command]
pub fn read_file_content(rel_path: String) -> Result<String, String> {
    vault::read_file(&rel_path)
}

/// 写入文件内容（覆盖原文件）。
#[tauri::command]
pub fn write_file_content(rel_path: String, content: String) -> Result<(), String> {
    vault::write_file(&rel_path, &content)
}

// ==================== 文件/文件夹操作 ====================

/// 在仓库中创建目录。
/// `parent_rel` 为 None/空字符串表示在根目录下创建。
#[tauri::command]
pub fn create_vault_dir(parent_rel: Option<String>, name: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    if name.chars().any(|c| c == '/' || c == '\\' || c == '\0' || c.is_control()) {
        return Err("名称包含非法字符".to_string());
    }
    let p = parent_rel.as_deref().filter(|s| !s.is_empty());
    vault::create_dir(p, name)
}

/// 重命名文件或目录。
#[tauri::command]
pub fn rename_vault_item(rel_path: String, new_name: String, is_dir: bool) -> Result<(), String> {
    let new_name = new_name.trim();
    if new_name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    vault::rename_item(&rel_path, new_name, is_dir)
}

/// 删除文件或目录。`is_dir=true` 时递归删除整个目录。
#[tauri::command]
pub fn delete_vault_item(rel_path: String, is_dir: bool) -> Result<(), String> {
    vault::delete_item(&rel_path, is_dir)
}

/// 把文件移动到另一个目录下。`target_dir_rel` 为空字符串表示移到根目录。
#[tauri::command]
pub fn move_vault_file(rel_path: String, target_dir_rel: String) -> Result<(), String> {
    vault::move_file(&rel_path, &target_dir_rel)
}

// ==================== 工具 ====================

/// 在 Finder 中定位文件。
#[tauri::command]
pub fn reveal_in_finder(rel_path: String) -> Result<(), String> {
    let abs = vault::resolve_abs_path(&rel_path)?;
    std::process::Command::new("open")
        .arg("-R")
        .arg(&abs)
        .spawn()
        .map_err(|e| format!("无法打开 Finder: {}", e))?;
    Ok(())
}

/// 返回某文件的绝对路径（复制/引用用）。
#[tauri::command]
pub fn get_file_abs_path(rel_path: String) -> Result<String, String> {
    let abs = vault::resolve_abs_path(&rel_path)?;
    Ok(abs.to_string_lossy().into_owned())
}
