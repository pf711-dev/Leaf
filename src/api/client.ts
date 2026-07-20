import { invoke } from "@tauri-apps/api/core";
import type { ConflictInfo, DirectoryImportResult, Document, Folder } from "../types/document";

/** 冲突解决策略，由用户在弹窗中选择后传入。 */
export type ConflictResolution = "skip" | "overwrite";

/** 导入一批 HTML 文件。paths 是源文件的绝对路径列表。返回成功导入的文档。
 *  folderId 指定目标文件夹；省略表示导入到根目录。 */
export function importFiles(
  paths: string[],
  resolution?: ConflictResolution,
  folderId?: string | null,
): Promise<Document[]> {
  return invoke<Document[]>("import_files", { paths, resolution, folderId });
}

/** 预检导入冲突：返回待导入路径中与库内已有 file_name 撞名的清单。 */
export function checkImportConflicts(paths: string[]): Promise<ConflictInfo[]> {
  return invoke<ConflictInfo[]>("check_import_conflicts", { paths });
}

/** 导入整个文件夹（连同目录结构）。后端递归遍历并重建文件夹层级。 */
export function importDirectory(rootPath: string): Promise<DirectoryImportResult> {
  return invoke<DirectoryImportResult>("import_directory", { rootPath });
}

/** 返回某文档库内副本的绝对路径（复制路径用）。 */
export function getDocumentPath(libraryPath: string): Promise<string> {
  return invoke<string>("get_document_path", { libraryPath });
}

/** 列出全部文档，按导入时间倒序。 */
export function listDocuments(): Promise<Document[]> {
  return invoke<Document[]>("list_documents");
}

/** 读取某个文档的 HTML 原文（阅读器用 srcdoc 渲染）。 */
export function readDocumentContent(libraryPath: string): Promise<string> {
  return invoke<string>("read_document_content", { libraryPath });
}

/** 写回某个文档的 HTML 原文（编辑模式保存用）。 */
export function writeDocumentContent(libraryPath: string, content: string): Promise<void> {
  return invoke<void>("write_document_content", { libraryPath, content });
}

/** 删除一个文档（库文件 + 数据库记录）。 */
export function deleteDocument(id: string, libraryPath: string): Promise<void> {
  return invoke<void>("delete_document", { id, libraryPath });
}

/** 返回库目录路径（设置/展示用）。 */
export function getLibraryDir(): Promise<string> {
  return invoke<string>("get_library_dir");
}

// -------------------- 文件夹 --------------------

/** 新建文件夹。parentId 为 null/undefined 表示在根目录下创建（level 1）。 */
export function createFolder(name: string, parentId?: string | null): Promise<Folder> {
  return invoke<Folder>("create_folder", { name, parentId: parentId ?? null });
}

/** 列出全部文件夹（扁平列表，前端组装成树）。 */
export function listFolders(): Promise<Folder[]> {
  return invoke<Folder[]>("list_folders");
}

/** 重命名文件夹。 */
export function renameFolder(id: string, newName: string): Promise<void> {
  return invoke<void>("rename_folder", { id, newName });
}

/** 删除文件夹（递归删除其下所有子文件夹与文档）。 */
export function deleteFolder(id: string): Promise<void> {
  return invoke<void>("delete_folder", { id });
}

/** 把文档移动到指定文件夹。folderId 为 null 表示移到根目录。 */
export function moveDocument(docId: string, folderId: string | null): Promise<void> {
  return invoke<void>("move_document", { docId, folderId });
}
