use std::path::{Path, PathBuf};

use crate::db::{Database, Document, Folder};
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
/// `folder_id` 是目标文件夹 id；None 表示导入到根目录。
/// 返回成功导入的文档列表。单个文件失败不会中断其余文件。
#[tauri::command]
pub fn import_files(
    db: tauri::State<'_, Database>,
    paths: Vec<String>,
    resolution: Option<String>,
    folder_id: Option<String>,
) -> Vec<Document> {
    let resolution = Resolution::parse(resolution.as_deref());
    let mut imported = Vec::new();
    for path_str in &paths {
        match import_single(&db, path_str, resolution, folder_id.as_deref()) {
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

/// 前端调用：写回某个文档的 HTML 原文（编辑模式保存用）。
#[tauri::command]
pub fn write_document_content(library_path: String, content: String) -> Result<(), String> {
    library::write_library_file(&library_path, &content).map_err(|e| e.to_string())
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
/// `folder_id` 指定导入到哪个文件夹（None 表示根目录）。
fn import_single(
    db: &Database,
    path_str: &str,
    resolution: Resolution,
    folder_id: Option<&str>,
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
        folder_id: folder_id.map(|s| s.to_string()),
    };
    db.insert(&doc)?;
    Ok(doc)
}

// ==================== 文件夹命令 ====================

/// 最大文件夹层级（含根目录）。level=1 是根目录下的一级文件夹，3 是最深层。
const MAX_FOLDER_LEVEL: i64 = 3;

/// 前端调用：新建文件夹。
///
/// `parent_id` 为 None 表示在根目录下创建（level = 1）。
/// 父文件夹已达最大层级时返回错误。
#[tauri::command]
pub fn create_folder(
    db: tauri::State<'_, Database>,
    name: String,
    parent_id: Option<String>,
) -> Result<Folder, String> {
    // 计算新文件夹的 level
    let level = match &parent_id {
        None => 1,
        Some(pid) => {
            let parent = db
                .get_folder(pid)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "父文件夹不存在".to_string())?;
            if parent.level >= MAX_FOLDER_LEVEL {
                return Err(format!(
                    "已达最大层级（{} 级），无法继续创建子文件夹",
                    MAX_FOLDER_LEVEL
                ));
            }
            parent.level + 1
        }
    };

    let folder = Folder {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        parent_id,
        created_at: chrono::Utc::now().timestamp_millis(),
        level,
    };
    db.insert_folder(&folder).map_err(|e| e.to_string())?;
    Ok(folder)
}

/// 前端调用：列出全部文件夹（扁平列表，前端组装成树）。
#[tauri::command]
pub fn list_folders(db: tauri::State<'_, Database>) -> Vec<Folder> {
    db.list_folders().unwrap_or_default()
}

/// 前端调用：重命名文件夹。
#[tauri::command]
pub fn rename_folder(
    db: tauri::State<'_, Database>,
    id: String,
    new_name: String,
) -> Result<(), String> {
    db.rename_folder(&id, &new_name).map_err(|e| e.to_string())
}

/// 前端调用：删除文件夹。
///
/// 递归删除该文件夹下所有子文件夹与文档（库文件 + 数据库记录）。
#[tauri::command]
pub fn delete_folder(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    // BFS 收集所有子孙文件夹 id（含自身）
    let mut all_ids: Vec<String> = vec![id.clone()];
    let mut queue: Vec<String> = vec![id.clone()];
    while let Some(cur) = queue.pop() {
        let subs = db.find_subfolders(&cur).map_err(|e| e.to_string())?;
        for sub in subs {
            all_ids.push(sub.id.clone());
            queue.push(sub.id);
        }
    }

    // 删除每个文件夹下的所有文档（库文件 + DB 记录）
    for fid in &all_ids {
        let docs = db.find_documents_in_folder(Some(fid)).map_err(|e| e.to_string())?;
        for doc in &docs {
            let _ = library::delete_library_file(&doc.library_path);
            db.delete(&doc.id).map_err(|e| e.to_string())?;
        }
    }

    // 删除所有文件夹记录
    for fid in &all_ids {
        db.delete_folder_row(fid).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 前端调用：把文档移动到指定文件夹（folder_id 为 None 表示移到根目录）。
#[tauri::command]
pub fn move_document(
    db: tauri::State<'_, Database>,
    doc_id: String,
    folder_id: Option<String>,
) -> Result<(), String> {
    db.update_document_folder(&doc_id, folder_id.as_deref())
        .map_err(|e| e.to_string())
}

// ==================== 导入文件夹 ====================

/// 导入文件夹的结果摘要。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryImportResult {
    /// 成功导入的文档数
    pub imported_count: u32,
    /// 因根级同名已存在而跳过的文档数
    pub skipped_count: u32,
    /// 因原始目录超过 3 级被拍平到第 3 级的文档数
    pub flattened_count: u32,
    /// 新建 / 复用的文件夹数
    pub folder_count: u32,
    /// 收集到的首个失败原因（用于前端诊断；多个失败只记第一个）。
    /// 空字符串表示无失败。
    pub first_error: String,
    /// 因读取/解析/写入失败而被跳过的文件数
    pub failed_count: u32,
}

/// 一条待导入文件：源绝对路径 + 相对根目录的路径段（用于决定挂到哪个文件夹）。
struct CollectedFile {
    source: PathBuf,
    /// 相对路径段（不含文件名）。例如 "a/b/c.html" → ["a", "b"]
    /// 长度即目录深度：0=根目录散落，1=一级文件夹，2=二级，3+ 都归到第 3 级
    dir_segments: Vec<String>,
    /// 原始文件名（含 .html）
    file_name: String,
}

/// 递归收集目录下所有 HTML 文件及其相对路径段。
fn collect_html_files(root: &Path) -> Vec<CollectedFile> {
    let mut out = Vec::new();
    walk_dir(root, root, &mut out);
    out
}

/// 递归遍历。base 是根目录，用于计算相对路径段。
fn walk_dir(current: &Path, base: &Path, out: &mut Vec<CollectedFile>) {
    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, base, out);
        } else if is_html(&path) {
            // 计算相对 base 的目录段
            let dir_segments: Vec<String> = path
                .parent()
                .and_then(|p| p.strip_prefix(base).ok())
                .map(|rel| {
                    rel.components()
                        .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let file_name = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown.html".to_string());
            out.push(CollectedFile {
                source: path,
                dir_segments,
                file_name,
            });
        }
    }
}

/// 判断是否 HTML 文件（按扩展名，大小写不敏感）。
fn is_html(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(ext.to_lowercase().as_str(), "html" | "htm"),
        None => false,
    }
}

/// 把单个文件导入到指定文件夹。
///
/// `dest_name` 已处理过跨目录同名前缀，`folder_id` 已由调用方解析好。
/// `inline` 为 true 时，把 HTML 引用的本地 CSS/JS/图片内联进内容再写库
/// （用于目录导入，让原型能完整渲染）；为 false 时直接复制源文件（单文件导入）。
/// 返回 Ok(true)=已导入，Ok(false)=因根级同名跳过。
fn import_file_at(
    db: &Database,
    file: &CollectedFile,
    dest_name: &str,
    folder_id: Option<&str>,
    inline: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    // 读取文件：优先按 UTF-8 读，失败则按字节读并做无损转换（UTF-8 无效字节替换为 U+FFFD）。
    // 这样 GBK/GB2312 等非 UTF-8 编码的 HTML 也能导入（内容可能有乱码，但不至于整篇失败）。
    let content = match std::fs::read_to_string(&file.source) {
        Ok(s) => s,
        Err(_) => {
            let bytes = std::fs::read(&file.source)?;
            String::from_utf8_lossy(&bytes).into_owned()
        }
    };
    // title/summary 用原文解析，避免内联进来的 base64/CSS/JS 污染摘要文本。
    let parsed = parser::parse(&content);
    let id = uuid::Uuid::new_v4().to_string();
    let title = parsed.title.clone().unwrap_or_else(|| {
        file.source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });

    // 决定写库内容与写入方式
    let size = if inline {
        // 目录导入：内联资源后写入。base_dir 是 HTML 源文件所在目录。
        let base_dir = file.source.parent().unwrap_or_else(|| Path::new(""));
        let inlined = crate::inliner::inline_resources(&content, base_dir);
        library::import_file_content(dest_name, &inlined)?
    } else {
        // 单文件导入：直接复制源文件（行为不变）。
        let (_, n) = library::import_file_named(&file.source, dest_name)?;
        n
    };

    let now = chrono::Utc::now().timestamp_millis();
    let doc = Document {
        id,
        title,
        file_name: file.file_name.clone(),
        library_path: dest_name.to_string(),
        file_size: size,
        summary: parsed.summary,
        imported_at: now,
        source_created_at: now,
        folder_id: folder_id.map(|s| s.to_string()),
    };
    db.insert(&doc)?;
    Ok(true)
}

/// 前端调用：导入整个文件夹。
///
/// 递归遍历 root 下所有 HTML 文件，以源文件夹名作为新文件夹名，
/// 在用户选定的 parent_folder_id 下创建（为 None 则在根目录下创建），
/// 所有 HTML 文件统一导入到该文件夹内。同名文件跳过。单个文件失败不中断其余。
#[tauri::command]
pub fn import_directory(
    db: tauri::State<'_, Database>,
    root_path: String,
    parent_folder_id: Option<String>,
) -> Result<DirectoryImportResult, String> {
    let root = PathBuf::from(&root_path);
    if !root.is_dir() {
        return Err("所选路径不是文件夹".to_string());
    }

    // 取源文件夹名称作为新建文件夹名
    let folder_name = root
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "未命名文件夹".to_string());

    let files = collect_html_files(&root);
    if files.is_empty() {
        return Err("该文件夹下没有 HTML 文件".to_string());
    }

    // 计算新文件夹的 level
    let level = match &parent_folder_id {
        None => 1,
        Some(pid) => {
            let parent = db
                .get_folder(pid)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "父文件夹不存在".to_string())?;
            if parent.level >= MAX_FOLDER_LEVEL {
                return Err(format!(
                    "已达最大层级（{} 级），无法继续创建子文件夹",
                    MAX_FOLDER_LEVEL
                ));
            }
            parent.level + 1
        }
    };

    // 创建目标文件夹
    let target_folder_id = uuid::Uuid::new_v4().to_string();
    let folder = Folder {
        id: target_folder_id.clone(),
        name: folder_name,
        parent_id: parent_folder_id,
        created_at: chrono::Utc::now().timestamp_millis(),
        level,
    };
    db.insert_folder(&folder).map_err(|e| e.to_string())?;
    let folder_count: u32 = 1;

    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;
    let mut first_error = String::new();

    for file in &files {
        // 库内文件名：加前缀避免与库内已有文件冲突
        let dest_name = format!("{}__{}", target_folder_id, file.file_name);

        // 同名跳过（仅检查根级；子目录文件已加前缀）
        if file.dir_segments.is_empty() {
            if let Ok(existing) = db.find_by_filename(&file.file_name) {
                if !existing.is_empty() {
                    skipped += 1;
                    continue;
                }
            }
        }

        match import_file_at(&db, file, &dest_name, Some(&target_folder_id), true) {
            Ok(true) => imported += 1,
            Ok(false) => skipped += 1,
            Err(e) => {
                let msg = format!("{}: {}", file.source.display(), e);
                eprintln!("导入失败 {}", msg);
                if first_error.is_empty() {
                    first_error = msg;
                }
                failed += 1;
            }
        }
    }

    Ok(DirectoryImportResult {
        imported_count: imported,
        skipped_count: skipped,
        flattened_count: 0,
        folder_count,
        first_error,
        failed_count: failed,
    })
}
