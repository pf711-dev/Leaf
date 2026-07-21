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
    folder_id: Option<String>,
) -> Result<Vec<ConflictInfo>, String> {
    let mut conflicts = Vec::new();
    for path_str in &paths {
        let source = PathBuf::from(path_str);
        let file_name = match source.file_name() {
            Some(n) => n.to_string_lossy().into_owned(),
            None => continue,
        };
        // 查目标文件夹内是否有同名文档（镜像化后唯一性按 folder 划分）
        let existing = db
            .find_by_filename_in_folder(&file_name, folder_id.as_deref())
            .map_err(|e| e.to_string())?;
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

/// 前端调用：在 Finder 中定位某文档（open -R）。
#[tauri::command]
pub fn reveal_in_finder(library_path: String) -> Result<(), String> {
    let full = library::resolve_library_path(&library_path).map_err(|e| e.to_string())?;
    std::process::Command::new("open")
        .arg("-R")
        .arg(&full)
        .spawn()
        .map_err(|e| format!("无法打开 Finder: {}", e))?;
    Ok(())
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

    // 去重检查：同一文件夹内已有同 file_name 的文档（镜像化后唯一性按 folder 划分）
    let existing = db.find_by_filename_in_folder(&file_name, folder_id)?;
    if !existing.is_empty() {
        match resolution {
            Resolution::Skip => {
                return Err(format!("跳过：{} 已存在", file_name).into());
            }
            Resolution::Overwrite => {
                // 覆盖：只删除同一文件夹下同名的旧文档（文件 + 记录）
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

    // 落库路径：根据 folder_id 链拼出子目录（磁盘目录用 folder.id 命名）。
    // 同 folder 内的文件名唯一性已由 find_by_filename_in_folder 拦截。
    let folders = db.list_folders()?;
    let chain = library::build_folder_chain_path(folder_id, &folders);
    let dest_name = if chain.is_empty() {
        file_name.clone()
    } else {
        format!("{}/{}", chain, file_name)
    };
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
    let new_name = new_name.trim().to_string();
    if new_name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    if new_name.chars().any(|c| c == '\0' || c.is_control()) {
        return Err("名称包含非法字符".to_string());
    }
    db.rename_folder(&id, &new_name).map_err(|e| e.to_string())
}

/// 前端调用：重命名文档。
///
/// `new_name` 为不含后缀的主名（如 "default"）。后端保留原后缀（.html/.htm），
/// 仅更新 DB 的 file_name。磁盘文件用 folder.id 命名，重命名不触碰磁盘层。
/// 唯一性约束：同一文件夹内不允许同名（跨文件夹可同名）。
#[tauri::command]
pub fn rename_document(
    db: tauri::State<'_, Database>,
    id: String,
    new_name: String,
) -> Result<(), String> {
    let new_name = new_name.trim().to_string();
    if new_name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    // 非法字符：路径分隔符 / 控制字符。允许的字符范围与主流文件系统一致。
    if new_name.chars().any(|c| c == '/' || c == '\\' || c == ':' || c == '\0' || c.is_control()) {
        return Err("名称包含非法字符".to_string());
    }

    let doc = db
        .get_document(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "文档不存在".to_string())?;

    // 提取原后缀（含点），保留后缀不变
    let ext = Path::new(&doc.file_name)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let new_file_name = format!("{}{}", new_name, ext);

    // 名称未变化直接成功
    if new_file_name == doc.file_name {
        return Ok(());
    }

    // 同文件夹内唯一性校验（跨文件夹允许同名）
    let dup = db
        .find_by_filename_in_folder(&new_file_name, doc.folder_id.as_deref())
        .map_err(|e| e.to_string())?;
    if dup.iter().any(|d| d.id != id) {
        return Err(format!("已存在同名文件「{}」", new_file_name));
    }

    // 磁盘用 folder.id 命名，重命名只改 DB 的 file_name，不动 library_path、不碰磁盘
    db.update_document_name(&id, &new_file_name).map_err(|e| e.to_string())?;

    Ok(())
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

    // 删除磁盘子目录树（镜像化后整个文件夹树是单个磁盘目录树，一次删完）
    let folders = db.list_folders().map_err(|e| e.to_string())?;
    let chain = library::build_folder_chain_path(Some(&id), &folders);
    let dir_rel = format!("{}/{}", chain, id);
    let _ = library::delete_library_dir_tree(&dir_rel);

    // 删除 DB 中所有子孙文档记录 + 文件夹记录
    for fid in &all_ids {
        let docs = db.find_documents_in_folder(Some(fid)).map_err(|e| e.to_string())?;
        for doc in &docs {
            db.delete(&doc.id).map_err(|e| e.to_string())?;
        }
        db.delete_folder_row(fid).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 前端调用：把文档移动到指定文件夹（folder_id 为 None 表示移到根目录）。
///
/// 镜像化后需同步搬运磁盘文件到目标文件夹的子目录，并更新 library_path。
/// 校验：目标文件夹内不允许同名文档。
#[tauri::command]
pub fn move_document(
    db: tauri::State<'_, Database>,
    doc_id: String,
    folder_id: Option<String>,
) -> Result<(), String> {
    let doc = db
        .get_document(&doc_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "文档不存在".to_string())?;

    // 目标与当前相同，直接成功
    if doc.folder_id.as_deref() == folder_id.as_deref() {
        return Ok(());
    }

    // 目标文件夹内同名校验
    let dup = db
        .find_by_filename_in_folder(&doc.file_name, folder_id.as_deref())
        .map_err(|e| e.to_string())?;
    if !dup.is_empty() {
        return Err(format!(
            "目标文件夹已存在同名文档「{}」",
            doc.file_name
        ));
    }

    // 算出新 library_path
    let folders = db.list_folders().map_err(|e| e.to_string())?;
    let new_chain = library::build_folder_chain_path(folder_id.as_deref(), &folders);
    let new_library_path = if new_chain.is_empty() {
        doc.file_name.clone()
    } else {
        format!("{}/{}", new_chain, doc.file_name)
    };

    // 搬磁盘文件
    library::move_library_file(&doc.library_path, &new_library_path).map_err(|e| e.to_string())?;

    // 更新 DB（folder_id + library_path）；失败回滚磁盘
    if let Err(e) = db
        .update_document_folder(&doc_id, folder_id.as_deref())
        .and_then(|_| db.update_library_path(&doc_id, &new_library_path))
    {
        let _ = library::move_library_file(&new_library_path, &doc.library_path);
        return Err(format!("更新数据库失败: {}", e));
    }

    Ok(())
}

/// 前端调用：把文件夹移到另一个父级下（folder_id 为 None 表示移到根目录）。
///
/// 校验规则：
/// - 目标父文件夹必须存在且 level < 3
/// - 移动后该文件夹及其子孙的层级均不得超过 3
/// - 不能移动到自身或自己的子孙下（前端通过 excludeId 保证，后端做防御校验）
#[tauri::command]
pub fn move_folder(
    db: tauri::State<'_, Database>,
    folder_id: String,
    new_parent_id: Option<String>,
) -> Result<(), String> {
    // 1. 获取被移动的文件夹
    let folder = db
        .get_folder(&folder_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "文件夹不存在".to_string())?;

    // 2. 计算新层级
    let new_level = match &new_parent_id {
        None => 1,
        Some(pid) => {
            if *pid == folder_id {
                return Err("不能将文件夹移到它自己里面".to_string());
            }
            let parent = db
                .get_folder(pid)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "目标文件夹不存在".to_string())?;
            if parent.level >= MAX_FOLDER_LEVEL {
                return Err(format!(
                    "已达最大层级（{} 级），无法放入子文件夹",
                    MAX_FOLDER_LEVEL
                ));
            }
            parent.level + 1
        }
    };

    // 3. 收集子孙文件夹（BFS）
    let mut descendants: Vec<String> = Vec::new();
    let mut queue: Vec<String> = vec![folder_id.clone()];
    while let Some(cur) = queue.pop() {
        let subs = db.find_subfolders(&cur).map_err(|e| e.to_string())?;
        for sub in subs {
            // 防御：不能移到子孙下
            if Some(&sub.id) == new_parent_id.as_ref() {
                return Err("不能将文件夹移到它自己的子文件夹下".to_string());
            }
            descendants.push(sub.id.clone());
            queue.push(sub.id);
        }
    }

    // 4. 校验所有子孙的层级不越界
    let level_shift = new_level - folder.level;
    for desc_id in &descendants {
        let desc = db
            .get_folder(desc_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "子孙文件夹不存在".to_string())?;
        if desc.level + level_shift > MAX_FOLDER_LEVEL {
            return Err(format!(
                "移动后「{}」的层级将超过 {} 级上限",
                desc.name, MAX_FOLDER_LEVEL
            ));
        }
    }

    // 5. 搬磁盘子目录（磁盘目录用 folder.id 命名，子孙 library_path 不用改——路径段全是 id）
    //    旧路径基于当前 parent 链；新路径基于目标 new_parent_id 链 + folder_id。
    let (old_rel, new_rel) = {
        let folders = db.list_folders().map_err(|e| e.to_string())?;
        let old_chain = library::build_folder_chain_path(Some(&folder_id), &folders);
        let new_chain = library::build_folder_chain_path(new_parent_id.as_deref(), &folders);
        (
            format!("{}/{}", old_chain, folder_id),
            format!("{}/{}", new_chain, folder_id),
        )
    };
    if let Err(e) = library::move_library_dir(&old_rel, &new_rel) {
        return Err(format!("移动磁盘目录失败: {}", e));
    }

    // 6. 更新自身 parent_id 和 level（失败回滚磁盘）
    if let Err(e) =
        db.update_folder_parent(&folder_id, new_parent_id.as_deref(), new_level)
    {
        let _ = library::move_library_dir(&new_rel, &old_rel);
        return Err(format!("更新数据库失败: {}", e));
    }

    // 7. 递归更新所有子孙的 level（DB 内部数据，不涉及磁盘）
    for desc_id in &descendants {
        let desc = db.get_folder(desc_id).map_err(|e| e.to_string())?;
        if let Some(desc) = desc {
            let desc_new_level = desc.level + level_shift;
            db.update_folder_parent(desc_id, desc.parent_id.as_deref(), desc_new_level)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
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

    // 目标文件夹的磁盘子目录路径（用 folder.id 命名）。新文件统一落到该子目录下。
    let chain = library::build_folder_chain_path(Some(&target_folder_id), &db.list_folders().map_err(|e| e.to_string())?);

    for file in &files {
        // 同名跳过：目标文件夹内已有同名则跳过（镜像化后唯一性按 folder 划分）
        if let Ok(existing) = db.find_by_filename_in_folder(&file.file_name, Some(&target_folder_id)) {
            if !existing.is_empty() {
                skipped += 1;
                continue;
            }
        }

        let dest_name = if chain.is_empty() {
            file.file_name.clone()
        } else {
            format!("{}/{}", chain, file.file_name)
        };

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
