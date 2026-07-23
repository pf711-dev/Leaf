import { invoke } from "@tauri-apps/api/core";
import type { VaultDir, VaultFile, VaultInfo } from "../types/document";

// ==================== 仓库配置 ====================

/** 设定仓库根目录，触发扫描 + 监听。 */
export function setVaultRoot(rootPath: string): Promise<void> {
  return invoke<void>("set_vault_root", { rootPath });
}

/** 返回仓库信息。无仓库时返回 null。 */
export function getVaultInfo(): Promise<VaultInfo | null> {
  return invoke<VaultInfo | null>("get_vault_info");
}

// ==================== 文件列表 ====================

/** 返回仓库中所有已索引的 HTML 文件。 */
export function listVaultFiles(): Promise<VaultFile[]> {
  return invoke<VaultFile[]>("list_vault_files");
}

/** 返回仓库中所有目录。 */
export function listVaultDirs(): Promise<VaultDir[]> {
  return invoke<VaultDir[]>("list_vault_dirs");
}

// ==================== 文件读写 ====================

/** 读取文件内容（通过相对路径）。 */
export function readFileContent(relPath: string): Promise<string> {
  return invoke<string>("read_file_content", { relPath });
}

/** 写入文件内容（覆盖原文件）。 */
export function writeFileContent(relPath: string, content: string): Promise<void> {
  return invoke<void>("write_file_content", { relPath, content });
}

// ==================== 文件/文件夹操作 ====================

/** 在仓库中创建目录。parentRel 为 null/空字符串表示在根目录下创建。 */
export function createVaultDir(parentRel: string | null, name: string): Promise<void> {
  return invoke<void>("create_vault_dir", { parentRel, name });
}

/** 重命名文件或目录。 */
export function renameVaultItem(relPath: string, newName: string, isDir: boolean): Promise<void> {
  return invoke<void>("rename_vault_item", { relPath, newName, isDir });
}

/** 删除文件或目录。isDir=true 时递归删除整个目录。 */
export function deleteVaultItem(relPath: string, isDir: boolean): Promise<void> {
  return invoke<void>("delete_vault_item", { relPath, isDir });
}

/** 把文件移动到另一个目录下。targetDirRel 为空字符串表示移到根目录。 */
export function moveVaultFile(relPath: string, targetDirRel: string): Promise<void> {
  return invoke<void>("move_vault_file", { relPath, targetDirRel });
}

// ==================== 工具 ====================

/** 在 Finder 中定位文件。 */
export function revealInFinder(relPath: string): Promise<void> {
  return invoke<void>("reveal_in_finder", { relPath });
}

/** 返回某文件的绝对路径。 */
export function getFileAbsPath(relPath: string): Promise<string> {
  return invoke<string>("get_file_abs_path", { relPath });
}
