use notify::{Event, EventKind, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::AppHandle;
use tauri::Emitter;
use walkdir::WalkDir;

/// 仓库中一个 HTML 文件的索引条目。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultFile {
    pub id: String,
    pub title: String,
    pub file_name: String,
    /// 从仓库根目录起算的相对路径，如 "slides/slide1.html"
    pub rel_path: String,
    /// 磁盘上的绝对路径
    pub abs_path: String,
    pub file_size: u64,
    /// 提取出的文本摘要
    pub summary: String,
    /// 源文件最后修改时间，Unix 毫秒
    pub last_modified: i64,
    /// 文件所在目录相对路径（从仓库根起算），如 "slides"；根目录则为 ""
    pub dir_path: String,
}

/// 仓库中的一个目录。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultDir {
    /// 从仓库根起算的相对路径，如 "slides/projects"；根目录自身用 ""
    pub rel_path: String,
    /// 目录名称
    pub name: String,
    /// 父目录相对路径；根目录自身为 None
    pub parent_path: Option<String>,
    /// 层级深度（根目录下的一级 = 1）
    pub level: u32,
}

/// VaultState：内部可变状态，由 RwLock 保护。
struct VaultState {
    files: Vec<VaultFile>,
    dirs: Vec<VaultDir>,
    root: PathBuf,
}

/// 全局仓库管理器（单例）。None 表示仓库尚未配置。
static VAULT: Lazy<RwLock<Option<Arc<VaultState>>>> = Lazy::new(|| RwLock::new(None));

/// 文件变更事件的防抖间隔（毫秒）。
const DEBOUNCE_MS: u64 = 500;

/// 静默事件的累积容器，由后台线程通过 try_lock 消费。
struct PendingEvents {
    added: Vec<String>,
    modified: Vec<String>,
    removed: Vec<String>,
    last_flush: Option<std::time::Instant>,
}

// ─── 公开接口：由 commands 层调用 ───

/// 初始化仓库：扫描 + 启动监听。
/// `root` 必须是已存在的目录。`app_handle` 用于后续 emit 事件。
pub fn init(root: &Path, app_handle: &AppHandle) -> Result<(), String> {
    if !root.is_dir() {
        return Err("仓库路径不是文件夹".to_string());
    }
    let root = root.canonicalize().map_err(|e| format!("路径解析失败: {}", e))?;

    let (files, dirs) = scan(&root);
    let state = Arc::new(VaultState {
        files,
        dirs,
        root: root.clone(),
    });

    // 写入全局单例
    {
        let mut guard = VAULT.write().map_err(|e| e.to_string())?;
        *guard = Some(state.clone());
    }

    // 启动文件监听
    start_watcher(root.clone(), app_handle.clone())?;

    Ok(())
}

/// 返回当前仓库的根路径（字符串）。未配置返回 None。
pub fn root_path() -> Option<String> {
    let guard = VAULT.read().ok()?;
    guard.as_ref().map(|s| s.root.to_string_lossy().into_owned())
}

/// 返回仓库根目录名称。
pub fn root_name() -> Option<String> {
    let guard = VAULT.read().ok()?;
    guard.as_ref().and_then(|s| {
        s.root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
    })
}

/// 返回所有已索引的 HTML 文件（按 rel_path 排序）。
pub fn list_files() -> Vec<VaultFile> {
    let guard = match VAULT.read() {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };
    match guard.as_ref() {
        Some(state) => {
            let mut files = state.files.clone();
            files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
            files
        }
        None => Vec::new(),
    }
}

/// 返回所有目录（按 rel_path 排序，根目录放在最前）。
pub fn list_dirs() -> Vec<VaultDir> {
    let guard = match VAULT.read() {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };
    match guard.as_ref() {
        Some(state) => {
            let mut dirs = state.dirs.clone();
            dirs.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
            // 根目录放在最前面
            dirs
        }
        None => Vec::new(),
    }
}

// ─── 文件系统操作 ───

/// 创建目录。
pub fn create_dir(parent_rel: Option<&str>, name: &str) -> Result<(), String> {
    let root = get_root()?;
    let path = match parent_rel {
        Some(p) if !p.is_empty() => root.join(p).join(name),
        _ => root.join(name),
    };
    std::fs::create_dir_all(&path)
        .map_err(|e| format!("创建文件夹失败: {}", e))?;
    // 重建索引
    rebuild_index()?;
    Ok(())
}

/// 重命名文件或目录。`_is_dir` 暂未使用，保留用于后续区分。
pub fn rename_item(rel_path: &str, new_name: &str, _is_dir: bool) -> Result<(), String> {
    let root = get_root()?;
    let src = root.join(rel_path);
    let parent = src.parent().unwrap_or(&root);
    let dst = parent.join(new_name);

    if dst.exists() {
        return Err(format!("「{}」已存在", new_name));
    }

    // 防穿越：不允许通过 new_name 中的路径段跨越目录边界
    if new_name.contains(std::path::MAIN_SEPARATOR) {
        return Err("名称中包含非法字符".to_string());
    }

    std::fs::rename(&src, &dst)
        .map_err(|e| format!("重命名失败: {}", e))?;

    rebuild_index()?;
    Ok(())
}

/// 删除文件或目录。`is_dir=true` 时递归删除整个目录。
pub fn delete_item(rel_path: &str, is_dir: bool) -> Result<(), String> {
    let root = get_root()?;
    let path = root.join(rel_path);

    if !path.exists() {
        return Ok(());
    }

    if is_dir {
        std::fs::remove_dir_all(&path)
            .map_err(|e| format!("删除文件夹失败: {}", e))?;
    } else {
        std::fs::remove_file(&path)
            .map_err(|e| format!("删除文件失败: {}", e))?;
    }

    rebuild_index()?;
    Ok(())
}

/// 把文件移动到目标目录下。target_dir_rel 为 "" 表示根目录。
pub fn move_file(rel_path: &str, target_dir_rel: &str) -> Result<(), String> {
    let root = get_root()?;
    let src = root.join(rel_path);
    let file_name = src
        .file_name()
        .ok_or("无效的文件路径")?
        .to_string_lossy()
        .into_owned();

    let dst = if target_dir_rel.is_empty() {
        root.join(&file_name)
    } else {
        root.join(target_dir_rel).join(&file_name)
    };

    if dst.exists() {
        return Err(format!("目标位置已存在同名文件「{}」", file_name));
    }

    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    std::fs::rename(&src, &dst)
        .map_err(|e| format!("移动文件失败: {}", e))?;

    rebuild_index()?;
    Ok(())
}

/// 以 UTF-8 读取文件内容。
pub fn read_file(rel_path: &str) -> Result<String, String> {
    let root = get_root()?;
    let path = root.join(rel_path);
    std::fs::read_to_string(&path)
        .map_err(|e| format!("读取文件失败: {}", e))
}

/// 以 UTF-8 写回文件内容（覆盖原文件）。
pub fn write_file(rel_path: &str, content: &str) -> Result<(), String> {
    let root = get_root()?;
    let path = root.join(rel_path);
    std::fs::write(&path, content)
        .map_err(|e| format!("保存文件失败: {}", e))?;
    Ok(())
}

/// 返回文件在磁盘上的绝对路径（供 Finder 定位等）。
pub fn resolve_abs_path(rel_path: &str) -> Result<PathBuf, String> {
    let root = get_root()?;
    let path = root.join(rel_path);
    if path.exists() {
        Ok(path)
    } else {
        Err("文件不存在".to_string())
    }
}

// ─── 内部实现 ───

/// 递归扫描 root 目录，收集所有 HTML 文件 + 所有目录。
fn scan(root: &Path) -> (Vec<VaultFile>, Vec<VaultDir>) {
    let mut files = Vec::new();
    let mut dir_paths: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    // 根目录自身
    dir_paths.insert(PathBuf::new());

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // 跳过隐藏文件和常见忽略目录
        if is_hidden(path) || should_ignore(path) {
            if entry.file_type().is_dir() {
                continue; // 不进入忽略目录
            } else {
                continue;
            }
        }

        if entry.file_type().is_dir() {
            // 记录目录（相对于 root）
            if let Ok(rel) = path.strip_prefix(root) {
                dir_paths.insert(rel.to_path_buf());
            }
        } else {
            // 检查是不是 HTML
            if is_html(path) {
                if let Ok(rel) = path.strip_prefix(root) {
                    let rel_str = rel.to_string_lossy().into_owned();
                    let file_name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "unknown.html".to_string());

                    // 读取文件内容并解析
                    let content = match std::fs::read_to_string(path) {
                        Ok(s) => s,
                        Err(_) => {
                            let bytes = match std::fs::read(path) {
                                Ok(b) => b,
                                Err(_) => continue,
                            };
                            String::from_utf8_lossy(&bytes).into_owned()
                        }
                    };

                    let parsed = crate::parser::parse(&content);
                    let title = parsed.title.unwrap_or_else(|| {
                        path.file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    });

                    let meta = match std::fs::metadata(path) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    let file_size = meta.len();
                    let last_modified = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);

                    // 计算所在目录相对路径
                    let dir_path = rel
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();

                    let id = uuid::Uuid::new_v4().to_string();

                    // 收起重复的分隔符
                    let rel_str = rel_str.replace('\\', "/");

                    files.push(VaultFile {
                        id,
                        title,
                        file_name,
                        rel_path: rel_str.clone(),
                        abs_path: path.to_string_lossy().into_owned(),
                        file_size,
                        summary: parsed.summary,
                        last_modified,
                        dir_path,
                    });
                }
            }
        }
    }

    // 构建目录列表。根目录自身（relPath=""）不作为展示节点，
    // 仓库名在侧边栏头部单独显示。只收集 level 1~3 的子目录。
    let mut dirs: Vec<VaultDir> = Vec::new();
    let mut sorted_paths: Vec<PathBuf> = dir_paths.into_iter().collect();
    sorted_paths.sort();

    for rel_path in &sorted_paths {
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");
        // 跳过根目录自身
        if rel_str.is_empty() {
            continue;
        }
        let name = rel_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "未命名".to_string());

        let level = rel_str.split('/').count() as u32;
        // 只保留 3 级以内的目录
        if level > 3 {
            continue;
        }

        let parent_path: Option<String> = {
            let raw = rel_path
                .parent()
                .and_then(|p| {
                    let s = p.to_string_lossy().replace('\\', "/");
                    if s.is_empty() { None } else { Some(s) }
                });
            raw
        };

        dirs.push(VaultDir {
            rel_path: rel_str,
            name,
            parent_path,
            level,
        });
    }

    (files, dirs)
}

/// 重建内存索引（文件变更后调用）。
fn rebuild_index() -> Result<(), String> {
    let root = get_root()?;
    let (files, dirs) = scan(&root);
    let new_state = Arc::new(VaultState {
        files,
        dirs,
        root: root.clone(),
    });
    {
        let mut guard = VAULT.write().map_err(|e| e.to_string())?;
        *guard = Some(new_state);
    }
    Ok(())
}

/// 获取当前仓库根目录。
fn get_root() -> Result<PathBuf, String> {
    let guard = VAULT.read().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(state) => Ok(state.root.clone()),
        None => Err("仓库尚未配置".to_string()),
    }
}

/// 启动文件系统监听。
fn start_watcher(root: PathBuf, app_handle: AppHandle) -> Result<(), String> {
    use notify::Config;
    use std::sync::Mutex;
    use std::time::Duration;

    // 用 Mutex 保护 pending 事件，因为 notify 的 EventHandler 要求 Send + 'static
    let pending = Arc::new(Mutex::new(PendingEvents {
        added: Vec::new(),
        modified: Vec::new(),
        removed: Vec::new(),
        last_flush: None,
    }));

    let pending_clone = pending.clone();
    let mut watcher = notify::recommended_watcher(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let mut p = match pending_clone.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                for path in &event.paths {
                    let abs = path.to_string_lossy().into_owned();
                    match event.kind {
                        EventKind::Create(_) => {
                            if !p.added.contains(&abs) {
                                p.added.push(abs);
                            }
                        }
                        EventKind::Modify(_) => {
                            if !p.modified.contains(&abs) {
                                p.modified.push(abs);
                            }
                        }
                        EventKind::Remove(_) => {
                            if !p.removed.contains(&abs) {
                                p.removed.push(abs);
                            }
                        }
                        _ => {}
                    }
                }
                p.last_flush = Some(std::time::Instant::now());
            }
        },
    )
    .map_err(|e| format!("创建文件监听器失败: {}", e))?;

    watcher
        .configure(Config::default().with_poll_interval(Duration::from_secs(2)))
        .map_err(|e| format!("配置监听器失败: {}", e))?;

    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| format!("启动监听失败: {}", e))?;

    // 后台线程：定期 flush pending 事件（防抖）
    let root_clone = root.clone();
    let app_clone = app_handle.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(DEBOUNCE_MS));
            let mut p = match pending.lock() {
                Ok(g) => g,
                Err(_) => break,
            };

            // 只有距上次事件 >= DEBOUNCE_MS 才 flush
            let should_flush = match p.last_flush {
                Some(t) => t.elapsed() >= Duration::from_millis(DEBOUNCE_MS),
                None => false,
            };
            if !should_flush {
                continue;
            }

            if p.added.is_empty() && p.modified.is_empty() && p.removed.is_empty() {
                drop(p);
                continue;
            }

            let added = std::mem::take(&mut p.added);
            let modified = std::mem::take(&mut p.modified);
            let removed = std::mem::take(&mut p.removed);
            drop(p);

            // 只关心 HTML 文件和相关目录的变化
            let has_relevant = added.iter().any(|p| is_html_path(p) || Path::new(p).is_dir())
                || modified.iter().any(|p| is_html_path(p))
                || removed.iter().any(|p| is_html_path(p) || Path::new(p).is_dir());

            if has_relevant {
                // 重建索引
                let (scan_files, scan_dirs) = scan(&root_clone);
                let mut guard = match VAULT.write() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                let new_state = Arc::new(VaultState {
                    files: scan_files,
                    dirs: scan_dirs,
                    root: root_clone.clone(),
                });
                *guard = Some(new_state);

                // 通知前端
                let mut payload = serde_json::Map::new();
                if !added.is_empty() {
                    payload.insert(
                        "added".to_string(),
                        serde_json::Value::Array(
                            added.iter().map(|p| serde_json::Value::String(p.clone())).collect(),
                        ),
                    );
                }
                if !modified.is_empty() {
                    payload.insert(
                        "modified".to_string(),
                        serde_json::Value::Array(
                            modified.iter().map(|p| serde_json::Value::String(p.clone())).collect(),
                        ),
                    );
                }
                if !removed.is_empty() {
                    payload.insert(
                        "removed".to_string(),
                        serde_json::Value::Array(
                            removed.iter().map(|p| serde_json::Value::String(p.clone())).collect(),
                        ),
                    );
                }
                let _ = app_clone.emit("vault-updated", payload);
            }
        }
        // 离开作用域时 watcher 自动 drop，停止监听
    });

    // 必须让 watcher 活着——把它 leak 掉，由线程和全局状态管理生命周期
    // (这里不能直接让 watcher 离开作用域，由静态变量保管)
    // 用一个 Mutex<Option<>> 存储，让线程持有
    static WATCHER: Lazy<Mutex<Option<notify::RecommendedWatcher>>> =
        Lazy::new(|| Mutex::new(None));
    let mut w = WATCHER.lock().map_err(|e| e.to_string())?;
    *w = Some(watcher);

    Ok(())
}

// ─── 工具函数 ───

/// 判断路径是否 HTML 文件（按扩展名）。
fn is_html(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(ext.to_lowercase().as_str(), "html" | "htm"),
        None => false,
    }
}

/// 用字符串路径判断是否是 HTML。
fn is_html_path(path_str: &str) -> bool {
    let path = Path::new(path_str);
    is_html(path)
}

/// 判断是否隐藏文件/文件夹（以 . 开头，排除 `.` 和 `..`）。
fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.') && n != "." && n != "..")
        .unwrap_or(false)
}

/// 判断是否应该跳过（node_modules, .git 等常见非文档目录）。
fn should_ignore(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(
        name,
        "node_modules"
            | ".git"
            | ".svn"
            | "__pycache__"
            | "target"
            | ".DS_Store"
            | ".vscode"
            | ".idea"
            | "dist"
            | "build"
    )
}
