use std::path::PathBuf;

use crate::db::{Database, Document};
use crate::{library, parser};
use serde::Serialize;

/// 冲突解决策略（由前端用户选择后传入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resolution {
    /// 跳过与库内 file_name 撞名的文件
    Skip,
    /// 覆盖库内同 file_name 的文档（删旧文件 + 旧记录，再导新的）
    Overwrite,
}

impl Resolution {
    fn parse(s: Option<&str>) -> Resolution {
        match s {
            Some("overwrite") => Resolution::Overwrite,
            // 默认（含 None / "skip" / 未知值）按跳过处理，更安全
            _ => Resolution::Skip,
        }
    }
}

/// 一条「待导入文件与库内已有文档撞名」的预检结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictInfo {
    /// 待导入源文件的绝对路径
    pub source_path: String,
    /// 撞名的原始文件名（如 "default.html"）
    pub file_name: String,
    /// 库中已存在文档的 id
    pub existing_doc_id: String,
    /// 库中已存在文档的标题（弹窗展示用，帮助用户判断是哪一篇）
    pub existing_title: String,
}

/// 前端调用：导入一批 HTML 文件。
///
/// `paths` 是源文件的绝对路径列表（来自文件选择对话框或拖拽）。
/// `resolution` 是撞名时的处理方式："skip"（默认）/ "overwrite"。
/// 返回成功导入的文档列表。单个文件失败不会中断其余文件。
#[tauri::command]
pub fn import_files(
    db: tauri::State<'_, Database>,
    paths: Vec<String>,
    resolution: Option<String>,
) -> Vec<Document> {
    let resolution = Resolution::parse(resolution.as_deref());
    let mut imported = Vec::new();
    for path_str in &paths {
        match import_single(&db, path_str, resolution) {
            Ok(doc) => imported.push(doc),
            Err(e) => {
                eprintln!("导入失败 {}: {}", path_str, e);
            }
        }
    }
    imported
}

/// 前端调用：预检导入冲突。
///
/// 检查待导入路径中，哪些的原始文件名（file_name）与库内已有文档撞名。
/// 前端据此弹窗让用户选择「全部跳过」或「全部覆盖」，再带 resolution 调 import_files。
#[tauri::command]
pub fn check_import_conflicts(
    db: tauri::State<'_, Database>,
    paths: Vec<String>,
) -> Result<Vec<ConflictInfo>, String> {
    let mut conflicts = Vec::new();
    for path_str in &paths {
        let source = PathBuf::from(path_str);
        let file_name = match source.file_name() {
            Some(n) => n.to_string_lossy().into_owned(),
            None => continue,
        };
        // 查库内是否有同 file_name 的文档
        let existing = db.find_by_filename(&file_name).map_err(|e| e.to_string())?;
        if let Some(first) = existing.into_iter().next() {
            conflicts.push(ConflictInfo {
                source_path: path_str.clone(),
                file_name,
                existing_doc_id: first.id,
                existing_title: first.title,
            });
        }
    }
    Ok(conflicts)
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
pub fn delete_document(
    db: tauri::State<'_, Database>,
    id: String,
    library_path: String,
) -> Result<(), String> {
    library::delete_library_file(&library_path).map_err(|e| e.to_string())?;
    db.delete(&id).map_err(|e| e.to_string())?;
    Ok(())
}

/// 前端调用：返回库目录路径（设置/展示用）。
#[tauri::command]
pub fn get_library_dir() -> Result<String, String> {
    library::library_dir_string().map_err(|e| e.to_string())
}

/// 前端调用：返回某文档库内副本的绝对路径（复制路径用）。
#[tauri::command]
pub fn get_document_path(library_path: String) -> Result<String, String> {
    let full = library::resolve_library_path(&library_path).map_err(|e| e.to_string())?;
    Ok(full.to_string_lossy().into_owned())
}

/// 导入单个文件的内部逻辑。
///
/// 返回 Err 仅表示「这个文件导入失败」（如文件读取错误），会被上层跳过。
/// 撞名且 resolution=Skip 时，也用 Err 返回标记跳过（上层统一处理）。
fn import_single(
    db: &Database,
    path_str: &str,
    resolution: Resolution,
) -> Result<Document, Box<dyn std::error::Error>> {
    let source = PathBuf::from(path_str);
    let file_name = source
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown.html".to_string());

    // 去重检查：库内已有同 file_name 的文档
    let existing = db.find_by_filename(&file_name)?;
    if !existing.is_empty() {
        match resolution {
            Resolution::Skip => {
                return Err(format!("跳过：{} 已存在", file_name).into());
            }
            Resolution::Overwrite => {
                // 覆盖：删除所有同 file_name 的旧文档（文件 + 记录）
                for old in &existing {
                    let _ = library::delete_library_file(&old.library_path);
                    db.delete(&old.id)?;
                }
            }
        }
    }

    // 读取并解析源文件
    let content = std::fs::read_to_string(&source)?;
    let parsed = parser::parse(&content);

    let id = uuid::Uuid::new_v4().to_string();
    let title = parsed.title.clone().unwrap_or_else(|| {
        source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });

    // 库内文件名直接用原始文件名（file_name）。
    // 同名文件已被上方的 find_by_filename 去重拦截，库内不会出现重名，无需额外冲突处理。
    let dest_name = file_name.clone();
    let (_, size) = library::import_file_named(&source, &dest_name)?;

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
