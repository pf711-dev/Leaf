use std::fs;
use std::path::{Path, PathBuf};

/// 库根目录的定位与创建，以及导入文件的落盘逻辑。
///
/// 数据目录定位与创建。注意：目录名仍沿用历史标识 "HTMLLibrary"，
/// 修改会导致已导入的文档丢失。用户可见的产品名为 "Leaf"。

/// 返回库根目录，必要时创建它。
pub fn ensure_library_dir() -> std::io::Result<PathBuf> {
    let dir = library_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// 返回库目录路径（不保证已存在）。
fn library_dir() -> std::io::Result<PathBuf> {
    let proj = directories::ProjectDirs::from("com", "mapengfei", "HTMLLibrary")
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "无法定位用户目录"))?;
    // 数据目录：macOS 下是 ~/Library/Application Support/com.mapengfei.HTMLLibrary
    // 沿用旧标识以保持数据兼容；产品名已改为 Leaf
    Ok(proj.data_dir().to_path_buf())
}

/// 返回数据库文件路径，父目录（库目录）会被创建。
pub fn db_path() -> std::io::Result<PathBuf> {
    let dir = ensure_library_dir()?;
    Ok(dir.join("library.db"))
}

/// 把源文件复制进库，用指定的可读名命名（dest_name 已含 .html 后缀）。
///
/// 调用前需确保 dest_name 已通过 build_library_filename 处理过冲突。
pub fn import_file_named(source: &Path, dest_name: &str) -> std::io::Result<(String, u64)> {
    let dir = ensure_library_dir()?;
    let dest = dir.join(dest_name);
    fs::copy(source, &dest)?;
    let size = fs::metadata(&dest)?.len();
    Ok((dest_name.to_string(), size))
}

/// 基于文档标题生成合法的库内文件名，冲突时自动加后缀。
///
/// 规则：清理非法字符 → 限制长度 → 与 existing 比对 → 冲突加 "-2"/"-3" 后缀。
/// title 为空时 fallback 到 "未命名"。
pub fn build_library_filename(title: &str, existing: &[String]) -> String {
    let cleaned = sanitize_filename(title);
    let base = if cleaned.is_empty() {
        "未命名".to_string()
    } else {
        cleaned
    };

    // 无冲突直接用 base.html
    let candidate = format!("{}.html", base);
    if !existing.iter().any(|e| e == &candidate) {
        return candidate;
    }

    // 冲突：尝试 base-2.html, base-3.html ...
    for n in 2..u32::MAX {
        let candidate = format!("{}-{}.html", base, n);
        if !existing.iter().any(|e| e == &candidate) {
            return candidate;
        }
    }
    // 理论上跑不到这里
    format!("{}-{}.html", base, uuid::Uuid::new_v4())
}

/// 清理标题中的文件系统非法字符，限制长度，去除首尾空格和点。
fn sanitize_filename(title: &str) -> String {
    const INVALID_CHARS: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    let cleaned: String = title
        .trim()
        .chars()
        .map(|c| if INVALID_CHARS.contains(&c) { '_' } else { c })
        .collect::<String>()
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .chars()
        .take(80)
        .collect();
    cleaned
}

/// 读取库内某个文档文件的全部内容（阅读器渲染用）。
pub fn read_library_file(library_path: &str) -> std::io::Result<String> {
    let dir = ensure_library_dir()?;
    let full = dir.join(library_path);
    fs::read_to_string(full)
}

/// 删除库内的文档文件。文件不存在视为成功。
pub fn delete_library_file(library_path: &str) -> std::io::Result<()> {
    let dir = ensure_library_dir()?;
    let full = dir.join(library_path);
    if full.exists() {
        fs::remove_file(full)?;
    }
    Ok(())
}

/// 返回库目录的字符串表示（给前端展示用）。
pub fn library_dir_string() -> std::io::Result<String> {
    let dir = ensure_library_dir()?;
    Ok(dir.to_string_lossy().into_owned())
}

/// 返回库内某个文档副本的绝对路径（拼接库目录 + 相对路径）。
pub fn resolve_library_path(library_path: &str) -> std::io::Result<PathBuf> {
    let dir = ensure_library_dir()?;
    Ok(dir.join(library_path))
}
