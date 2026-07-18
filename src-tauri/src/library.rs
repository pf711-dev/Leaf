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

/// 迁移历史库内文件名到原始文件名。
///
/// 早期版本库内文件名基于 HTML <title> 生成，与原始文件名不一致。
/// 此函数遍历 docs，把 library_path != file_name 的库内文件重命名为原始文件名，
/// 并回调 on_update 更新 DB。
///
/// - docs：所有文档（调用方负责从 DB 读取）
/// - on_update：更新某条文档的 library_path（参数：doc_id, new_path）
///
/// 幂等：迁移完成后所有文档均满足 library_path == file_name。
pub fn migrate_filenames<F>(docs: &[crate::db::Document], on_update: F)
where
    F: Fn(&str, &str) -> Result<(), String>,
{
    let dir = match ensure_library_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("迁移：无法定位库目录: {}", e);
            return;
        }
    };

    for doc in docs {
        if doc.library_path == doc.file_name {
            continue; // 已是原始名，跳过
        }
        let src = dir.join(&doc.library_path);
        let dst = dir.join(&doc.file_name);

        // 防御：目标名在库目录已存在（且不是自身），跳过避免覆盖
        if dst.exists() && src != dst {
            eprintln!(
                "迁移：跳过 {}（目标名 {} 已存在）",
                doc.library_path, doc.file_name
            );
            continue;
        }

        // 源文件存在则重命名；不存在（文件已丢失）只更新 DB
        if src.exists() {
            if let Err(e) = std::fs::rename(&src, &dst) {
                eprintln!(
                    "迁移：重命名失败 {} → {}: {}",
                    doc.library_path, doc.file_name, e
                );
                continue;
            }
        }
        // 更新 DB
        if let Err(e) = on_update(&doc.id, &doc.file_name) {
            eprintln!("迁移：更新 DB 失败 {}: {}", doc.id, e);
        }
    }
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
