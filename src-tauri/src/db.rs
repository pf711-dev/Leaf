use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// 文档元数据，对应数据库 documents 表的一条记录，也直接序列化给前端。
/// `rename_all = "camelCase"` 让序列化输出用 camelCase，与前端 TS 类型一致。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub title: String,
    pub file_name: String,
    /// 库内相对路径（相对库根目录），例如 "abc123.html"
    pub library_path: String,
    pub file_size: u64,
    /// 提取出的纯文本摘要，用于卡片预览
    pub summary: String,
    /// 导入时间，Unix 毫秒
    pub imported_at: i64,
    /// 原文件创建时间（从文件系统读取），Unix 毫秒
    pub source_created_at: i64,
    /// 所属文件夹 id；NULL 表示位于根目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
}

/// 文件夹元数据，对应 folders 表。最多支持 3 级（含根目录）。
/// level = 1 表示根目录下的一级文件夹，2 表示二级，3 表示三级。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    /// 父文件夹 id；NULL 表示位于根目录（level = 1）
    pub parent_id: Option<String>,
    /// 创建时间，Unix 毫秒
    pub created_at: i64,
    /// 层级：1 / 2 / 3
    pub level: i64,
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// 打开（必要时创建）数据库，并建表。db_path 是一个完整的 .db 文件路径。
    pub fn open(db_path: &std::path::Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(db_path)?;

        // 增量迁移：清理历史遗留的「不完整 folders 表」。
        // 早期开发版本可能建过缺 level 列的 folders 表，CREATE TABLE IF NOT EXISTS
        // 看到表已存在就跳过，导致后续 INSERT 缺列报错。这里检测到列缺失就删表重建
        // （脏表里不会有用户数据，安全）。
        if table_exists(&conn, "folders")? && !column_exists(&conn, "folders", "level")? {
            conn.execute("DROP TABLE IF EXISTS folders", [])?;
        }

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents (
                id                TEXT PRIMARY KEY,
                title             TEXT NOT NULL,
                file_name         TEXT NOT NULL,
                library_path      TEXT NOT NULL,
                file_size         INTEGER NOT NULL,
                summary           TEXT NOT NULL,
                imported_at       INTEGER NOT NULL,
                source_created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_documents_imported_at
                ON documents(imported_at DESC);

            CREATE TABLE IF NOT EXISTS folders (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                parent_id   TEXT,
                created_at  INTEGER NOT NULL,
                level       INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_folders_parent_id
                ON folders(parent_id);",
        )?;

        // 增量迁移：给老库的 documents 表补 folder_id 列（IF NOT EXISTS 不会加列）。
        // 通过 PRAGMA table_info 检测列是否存在，不存在则 ALTER TABLE ADD COLUMN。
        if !column_exists(&conn, "documents", "folder_id")? {
            conn.execute("ALTER TABLE documents ADD COLUMN folder_id TEXT", [])?;
        }

        // 仓库配置表（单记录，id 固定为 1）。
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS vault_config (
                id INTEGER PRIMARY KEY DEFAULT 1,
                root_path TEXT NOT NULL,
                name TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert(&self, doc: &Document) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO documents
                (id, title, file_name, library_path, file_size, summary, imported_at, source_created_at, folder_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                doc.id,
                doc.title,
                doc.file_name,
                doc.library_path,
                doc.file_size as i64,
                doc.summary,
                doc.imported_at,
                doc.source_created_at,
                doc.folder_id,
            ],
        )?;
        Ok(())
    }

    /// 按导入时间倒序返回全部文档。
    pub fn list(&self) -> rusqlite::Result<Vec<Document>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, file_name, library_path, file_size, summary, imported_at, source_created_at, folder_id
             FROM documents ORDER BY imported_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_document)?;
        rows.collect()
    }

    pub fn delete(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM documents WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// 更新某条文档的 library_path（文件名迁移用）。
    pub fn update_library_path(&self, id: &str, new_path: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE documents SET library_path = ?1 WHERE id = ?2",
            params![new_path, id],
        )?;
        Ok(())
    }

    /// 仅更新文档的显示名（file_name）。磁盘用 id 命名后，重命名不再动 library_path。
    pub fn update_document_name(&self, id: &str, new_file_name: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE documents SET file_name = ?1 WHERE id = ?2",
            params![new_file_name, id],
        )?;
        Ok(())
    }

    /// 把文档移动到指定文件夹（folder_id 为 None 表示根目录）。
    pub fn update_document_folder(
        &self,
        doc_id: &str,
        folder_id: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE documents SET folder_id = ?1 WHERE id = ?2",
            params![folder_id, doc_id],
        )?;
        Ok(())
    }

    /// 在指定文件夹内按 file_name 查找文档（镜像化后唯一性约束变为「同 folder 内唯一」）。
    /// folder_id 为 None 表示根目录散落文档。
    pub fn find_by_filename_in_folder(
        &self,
        file_name: &str,
        folder_id: Option<&str>,
    ) -> rusqlite::Result<Vec<Document>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, file_name, library_path, file_size, summary, imported_at, source_created_at, folder_id
             FROM documents WHERE file_name = ?1 AND folder_id IS ?2",
        )?;
        let rows = stmt.query_map(params![file_name, folder_id], row_to_document)?;
        rows.collect()
    }

    /// 按 id 查找单条文档（重命名等场景用）。
    pub fn get_document(&self, id: &str) -> rusqlite::Result<Option<Document>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, file_name, library_path, file_size, summary, imported_at, source_created_at, folder_id
             FROM documents WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], row_to_document)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// 返回位于某个文件夹下的全部文档（folder_id 为 None 表示根目录下散落的文档）。
    pub fn find_documents_in_folder(&self, folder_id: Option<&str>) -> rusqlite::Result<Vec<Document>> {
        let conn = self.conn.lock().unwrap();
        let sql = "SELECT id, title, file_name, library_path, file_size, summary, imported_at, source_created_at, folder_id
             FROM documents WHERE folder_id IS ?1 ORDER BY imported_at DESC";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![folder_id], row_to_document)?;
        rows.collect()
    }

    // -------------------- 文件夹 CRUD --------------------

    /// 插入一条文件夹记录。
    pub fn insert_folder(&self, folder: &Folder) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO folders (id, name, parent_id, created_at, level)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                folder.id,
                folder.name,
                folder.parent_id,
                folder.created_at,
                folder.level,
            ],
        )?;
        Ok(())
    }

    /// 返回全部文件夹（扁平列表，由前端组装成树）。
    pub fn list_folders(&self) -> rusqlite::Result<Vec<Folder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, created_at, level FROM folders ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], row_to_folder)?;
        rows.collect()
    }

    /// 按 id 查单个文件夹。
    pub fn get_folder(&self, id: &str) -> rusqlite::Result<Option<Folder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, created_at, level FROM folders WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], row_to_folder)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// 重命名文件夹。
    pub fn rename_folder(&self, id: &str, new_name: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE folders SET name = ?1 WHERE id = ?2",
            params![new_name, id],
        )?;
        Ok(())
    }

    /// 移动文件夹：更新 parent_id 和 level。
    pub fn update_folder_parent(
        &self,
        id: &str,
        new_parent_id: Option<&str>,
        new_level: i64,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE folders SET parent_id = ?1, level = ?2 WHERE id = ?3",
            params![new_parent_id, new_level, id],
        )?;
        Ok(())
    }

    /// 仅删除文件夹自身这一行记录（不连带删除子孙与文档，递归由 commands 层负责）。
    pub fn delete_folder_row(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM folders WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// 返回直接子文件夹（parent_id 等于给定 id）。
    pub fn find_subfolders(&self, parent_id: &str) -> rusqlite::Result<Vec<Folder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, created_at, level FROM folders WHERE parent_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![parent_id], row_to_folder)?;
        rows.collect()
    }

    // ─── vault_config 操作 ───

    /// 保存或更新仓库配置（root_path + 名称）。
    pub fn set_vault_config(&self, root_path: &str, name: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT OR REPLACE INTO vault_config (id, root_path, name, created_at)
             VALUES (1, ?1, ?2, ?3)",
            params![root_path, name, now],
        )?;
        Ok(())
    }

    /// 读取仓库配置。返回 (root_path, name)。
    pub fn get_vault_config(&self) -> rusqlite::Result<Option<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT root_path, name FROM vault_config WHERE id = 1",
        )?;
        let mut rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// 清除仓库配置（目录被删除时调用）。
    pub fn clear_vault_config(&self) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM vault_config WHERE id = 1", [])?;
        Ok(())
    }
}

fn row_to_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<Document> {
    Ok(Document {
        id: row.get(0)?,
        title: row.get(1)?,
        file_name: row.get(2)?,
        library_path: row.get(3)?,
        file_size: row.get::<_, i64>(4)? as u64,
        summary: row.get(5)?,
        imported_at: row.get(6)?,
        source_created_at: row.get(7)?,
        folder_id: row.get(8)?,
    })
}

fn row_to_folder(row: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: row.get(0)?,
        name: row.get(1)?,
        parent_id: row.get(2)?,
        created_at: row.get(3)?,
        level: row.get(4)?,
    })
}

/// 检测某张表是否存在指定列（用于增量迁移时判断是否需要 ALTER TABLE ADD COLUMN）。
fn column_exists(
    conn: &Connection,
    table: &str,
    column: &str,
) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let rows = stmt.query_map([], |row| {
        let name: String = row.get(1)?;
        Ok(name)
    })?;
    for row in rows {
        if row? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// 检测某张表是否存在（通过 sqlite_master 查询）。
fn table_exists(conn: &Connection, table: &str) -> rusqlite::Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}
