use std::path::PathBuf;

use crate::db::{Database, Document};
use crate::{library, parser};

/// 前端调用：导入一批 HTML 文件。
///
/// `paths` 是源文件的绝对路径列表（来自文件选择对话框或拖拽）。
/// 返回成功导入的文档列表。单个文件失败不会中断其余文件。
#[tauri::command]
pub fn import_files(db: tauri::State<'_, Database>, paths: Vec<String>) -> Vec<Document> {
    let mut imported = Vec::new();
    for path_str in &paths {
        match import_single(&db, path_str) {
            Ok(doc) => imported.push(doc),
            Err(e) => {
                eprintln!("导入失败 {}: {}", path_str, e);
            }
        }
    }
    imported
}

/// 前端调用：列出全部文档，按导入时间倒序。
#[tauri::command]
pub fn list_documents(db: tauri::State<'_, Database>) -> Vec<Document> {
    db.list().unwrap_or_default()
}

/// 前端调用：读取某个文档的 HTML 原文（阅读器用 srcdoc 渲染）。
#[tauri::command]
pub fn read_document_content(library_path: String) -> Result<String, String> {
    library::read_library_file(&library_path).map_err(|e| e.to_string())
}

/// 前端调用：删除一个文档（库文件 + 数据库记录）。
#[tauri::command]
pub fn delete_document(db: tauri::State<'_, Database>, id: String, library_path: String) -> Result<(), String> {
    library::delete_library_file(&library_path).map_err(|e| e.to_string())?;
    db.delete(&id).map_err(|e| e.to_string())?;
    Ok(())
}

/// 前端调用：返回库目录路径（设置/展示用）。
#[tauri::command]
pub fn get_library_dir() -> Result<String, String> {
    library::library_dir_string().map_err(|e| e.to_string())
}

fn import_single(db: &Database, path_str: &str) -> Result<Document, Box<dyn std::error::Error>> {
    let source = PathBuf::from(path_str);
    let content = std::fs::read_to_string(&source)?;
    let parsed = parser::parse(&content);

    let id = uuid::Uuid::new_v4().to_string();
    let (dest_name, size) = library::import_file(&source, &id)?;

    let title = parsed
        .title
        .unwrap_or_else(|| source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    let file_name = source
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown.html".to_string());

    let now = chrono::Utc::now().timestamp_millis();
    let doc = Document {
        id,
        title,
        file_name,
        library_path: dest_name,
        file_size: size,
        summary: parsed.summary,
        imported_at: now,
        source_created_at: now,
    };
    db.insert(&doc)?;
    Ok(doc)
}
