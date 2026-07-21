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

/// 把源文件复制进库，用指定的相对路径命名（dest_name 可含子目录，如 "fid1/fid2/a.html"）。
/// 会自动创建所需的父目录。
pub fn import_file_named(source: &Path, dest_name: &str) -> std::io::Result<(String, u64)> {
    let dir = ensure_library_dir()?;
    let dest = dir.join(dest_name);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, &dest)?;
    let size = fs::metadata(&dest)?.len();
    Ok((dest_name.to_string(), size))
}

/// 把指定内容直接写入库内文件（不复制源文件），用于资源内联后的 HTML。
/// dest_name 可含子目录，会自动创建所需的父目录。返回写入的字节数。
pub fn import_file_content(dest_name: &str, content: &str) -> std::io::Result<u64> {
    let dir = ensure_library_dir()?;
    let dest = dir.join(dest_name);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&dest, content)?;
    let size = fs::metadata(&dest)?.len();
    Ok(size)
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

/// 写回库内某个文档文件的全部内容（编辑保存用），文件不存在则创建。
pub fn write_library_file(library_path: &str, content: &str) -> std::io::Result<()> {
    let dir = ensure_library_dir()?;
    let full = dir.join(library_path);
    fs::write(full, content)
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

/// 把库内文件从 old_rel 搬到 new_rel（可跨子目录）。自动创建目标父目录。
/// 目标已存在则拒绝（不覆盖）。源不存在视为成功。
pub fn move_library_file(old_rel: &str, new_rel: &str) -> std::io::Result<()> {
    let dir = ensure_library_dir()?;
    let src = dir.join(old_rel);
    let dst = dir.join(new_rel);
    if dst.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "目标文件已存在",
        ));
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if src.exists() {
        fs::rename(src, dst)?;
    }
    Ok(())
}

/// 把库内的磁盘子目录从 old_rel 搬到 new_rel（用于 move_folder）。
/// 自动创建目标父目录。源不存在视为成功（可能尚未落盘）。
pub fn move_library_dir(old_rel: &str, new_rel: &str) -> std::io::Result<()> {
    let dir = ensure_library_dir()?;
    let src = dir.join(old_rel);
    let dst = dir.join(new_rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if src.exists() {
        fs::rename(src, dst)?;
    }
    Ok(())
}

/// 删除库内某个磁盘子目录树（用于 delete_folder）。不存在视为成功。
pub fn delete_library_dir_tree(rel: &str) -> std::io::Result<()> {
    let dir = ensure_library_dir()?;
    let full = dir.join(rel);
    if full.exists() {
        fs::remove_dir_all(full)?;
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

/// 根据文件夹 id 链构建从库根到该文件夹的相对目录路径（不含首尾斜杠）。
///
/// 例如 folder_id 位于根 → ""；folder_id 在一级文件夹 → "<folder_id>"；
/// 三级 → "<f1_id>/<f2_id>/<f3_id>"。folder_id 为 None 返回 ""（根目录）。
/// 用 folder_id（而非 name）命名磁盘目录，保证改名/移动时磁盘层零改动。
pub fn build_folder_chain_path(folder_id: Option<&str>, folders: &[crate::db::Folder]) -> String {
    let Some(fid) = folder_id else {
        return String::new();
    };
    let mut segments: Vec<&str> = Vec::new();
    let mut current = fid;
    loop {
        let f = match folders.iter().find(|f| f.id == current) {
            Some(f) => f,
            None => break,
        };
        segments.push(f.id.as_str());
        match &f.parent_id {
            Some(pid) => current = pid,
            None => break,
        }
    }
    segments.reverse();
    segments.join("/")
}

/// 计算某文档应有的「镜像化」相对路径：<folder_chain>/<file_name>，或根目录下的 <file_name>。
pub fn build_document_rel_path(doc: &crate::db::Document, folders: &[crate::db::Folder]) -> String {
    let chain = build_folder_chain_path(doc.folder_id.as_deref(), folders);
    if chain.is_empty() {
        doc.file_name.clone()
    } else {
        format!("{}/{}", chain, doc.file_name)
    }
}

/// 一次性迁移：把扁平的库内文件搬到镜像化的子目录结构（磁盘目录用 folder.id 命名）。
///
/// 对每个 doc，按 folder_id 链算出应有的相对路径，若与当前 library_path 不同则搬移文件并回调更新 DB。
/// 幂等：已是目标格式则跳过。源文件不存在只更新 DB。目标已存在（异常）跳过不覆盖。
pub fn migrate_to_folder_layout<F>(
    docs: &[crate::db::Document],
    folders: &[crate::db::Folder],
    on_update: F,
) where
    F: Fn(&str, &str) -> Result<(), String>,
{
    let dir = match ensure_library_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("镜像化迁移：无法定位库目录: {}", e);
            return;
        }
    };

    for doc in docs {
        let target = build_document_rel_path(doc, folders);
        if target == doc.library_path {
            continue; // 已是目标格式
        }
        let src = dir.join(&doc.library_path);
        let dst = dir.join(&target);

        // 防御：目标已存在（且不是自身），跳过避免覆盖
        if dst.exists() && src != dst {
            eprintln!(
                "镜像化迁移：跳过 {}（目标 {} 已存在）",
                doc.library_path, target
            );
            // 仍更新 DB，使 library_path 指向已存在的目标文件
        } else if src.exists() {
            if let Some(parent) = dst.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    eprintln!(
                        "镜像化迁移：创建目录失败 {}: {}",
                        parent.display(),
                        e
                    );
                    continue;
                }
            }
            if let Err(e) = fs::rename(&src, &dst) {
                eprintln!(
                    "镜像化迁移：搬移失败 {} → {}: {}",
                    doc.library_path, target, e
                );
                continue;
            }
        }
        // 更新 DB
        if let Err(e) = on_update(&doc.id, &target) {
            eprintln!("镜像化迁移：更新 DB 失败 {}: {}", doc.id, e);
        }
    }
}
