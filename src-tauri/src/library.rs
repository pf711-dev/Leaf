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

/// 把一个源 HTML 文件复制进库目录，返回库内的相对路径（形如 "<id>.html"）。
///
/// 用传入的 id 命名，避免重名冲突。
pub fn import_file(source: &Path, id: &str) -> std::io::Result<(String, u64)> {
    let dir = ensure_library_dir()?;
    let dest_name = format!("{}.html", id);
    let dest = dir.join(&dest_name);
    fs::copy(source, &dest)?;
    let size = fs::metadata(&dest)?.len();
    Ok((dest_name, size))
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
