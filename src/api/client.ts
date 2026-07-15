import { invoke } from "@tauri-apps/api/core";
import type { ConflictInfo, Document } from "../types/document";

/** 冲突解决策略，由用户在弹窗中选择后传入。 */
export type ConflictResolution = "skip" | "overwrite";

/** 导入一批 HTML 文件。paths 是源文件的绝对路径列表。返回成功导入的文档。 */
export function importFiles(
  paths: string[],
  resolution?: ConflictResolution,
): Promise<Document[]> {
  return invoke<Document[]>("import_files", { paths, resolution });
}

/** 预检导入冲突：返回待导入路径中与库内已有 file_name 撞名的清单。 */
export function checkImportConflicts(paths: string[]): Promise<ConflictInfo[]> {
  return invoke<ConflictInfo[]>("check_import_conflicts", { paths });
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

/** 删除一个文档（库文件 + 数据库记录）。 */
export function deleteDocument(id: string, libraryPath: string): Promise<void> {
  return invoke<void>("delete_document", { id, libraryPath });
}

/** 返回库目录路径（设置/展示用）。 */
export function getLibraryDir(): Promise<string> {
  return invoke<string>("get_library_dir");
}
