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
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// 打开（必要时创建）数据库，并建表。db_path 是一个完整的 .db 文件路径。
    pub fn open(db_path: &std::path::Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(db_path)?;
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
                ON documents(imported_at DESC);",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert(&self, doc: &Document) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO documents
                (id, title, file_name, library_path, file_size, summary, imported_at, source_created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                doc.id,
                doc.title,
                doc.file_name,
                doc.library_path,
                doc.file_size as i64,
                doc.summary,
                doc.imported_at,
                doc.source_created_at,
            ],
        )?;
        Ok(())
    }

    /// 按导入时间倒序返回全部文档。
    pub fn list(&self) -> rusqlite::Result<Vec<Document>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, file_name, library_path, file_size, summary, imported_at, source_created_at
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

    /// 返回所有文档的库内相对路径（library_path），用于生成可读文件名时的冲突检测。
    pub fn list_library_paths(&self) -> rusqlite::Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT library_path FROM documents")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect()
    }

    /// 按原始文件名（file_name）查找文档，用于导入去重预检。
    /// 返回所有匹配的文档（理论上 file_name 不唯一，同名不同内容会有多条）。
    pub fn find_by_filename(&self, file_name: &str) -> rusqlite::Result<Vec<Document>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, file_name, library_path, file_size, summary, imported_at, source_created_at
             FROM documents WHERE file_name = ?1",
        )?;
        let rows = stmt.query_map(params![file_name], row_to_document)?;
        rows.collect()
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
    })
}
