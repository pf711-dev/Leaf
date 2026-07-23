import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { DirTreeNode, VaultDir, VaultFile } from "../types/document";
import {
  createVaultDir as apiCreateVaultDir,
  deleteVaultItem,
  listVaultDirs,
  listVaultFiles,
  moveVaultFile as apiMoveVaultFile,
  moveVaultDir as apiMoveVaultDir,
  renameVaultItem,
} from "../api/client";

export const useDocumentsStore = defineStore("documents", () => {
  /** 仓库中全部 HTML 文件 */
  const files = ref<VaultFile[]>([]);
  /** 仓库中全部目录 */
  const dirs = ref<VaultDir[]>([]);
  /** 是否正在加载 */
  const loading = ref(false);
  /** 错误信息 */
  const error = ref("");
  /** 操作错误（toast 提示用） */
  const folderError = ref("");

  /**
   * 把扁平的 dirs + files 组装成树。
   * 根目录本身不参与展示（由侧边栏头部单独显示仓库名），
   * 树从一级子目录（parentPath=""）开始。
   */
  const tree = computed<DirTreeNode[]>(() => {
    const dirMap = new Map<string, VaultDir[]>();
    for (const d of dirs.value) {
      const key = d.parentPath ?? "";
      const list = dirMap.get(key) || [];
      list.push(d);
      dirMap.set(key, list);
    }

    const fileMap = new Map<string, VaultFile[]>();
    for (const f of files.value) {
      const key = f.dirPath || "";
      const list = fileMap.get(key) || [];
      list.push(f);
      fileMap.set(key, list);
    }

    const buildNodes = (parentRel: string): DirTreeNode[] => {
      const children = dirMap.get(parentRel) || [];
      return children.map((d) => ({
        dir: d,
        children: buildNodes(d.relPath),
        files: fileMap.get(d.relPath) || [],
      }));
    };

    // 从一级目录（parentPath="" 即仓库根之下）开始构建
    return buildNodes("");
  });

  /** 仓库根目录下的散落文件（dirPath 为空 = 未在任何子目录中）。 */
  const rootFiles = computed<VaultFile[]>(() =>
    files.value.filter((f) => !f.dirPath),
  );

  /** 是否有文件或目录 */
  const hasItems = computed(() => files.value.length > 0);

  /** 文件数量 */
  const fileCount = computed(() => files.value.length);

  /** 刷新文件列表 + 目录列表 */
  async function refresh() {
    loading.value = true;
    error.value = "";
    try {
      const [f, d] = await Promise.all([listVaultFiles(), listVaultDirs()]);
      files.value = f;
      dirs.value = d;
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  // ─── 文件夹操作（同步到文件系统） ───

  /** 新建文件夹 */
  async function createDir(parentRel: string | null, name: string): Promise<boolean> {
    try {
      await apiCreateVaultDir(parentRel, name);
      await refresh();
      return true;
    } catch (e) {
      folderError.value = String(e);
      return false;
    }
  }

  /** 重命名文件或目录 */
  async function renameItem(relPath: string, newName: string, isDir: boolean): Promise<boolean> {
    try {
      await renameVaultItem(relPath, newName, isDir);
      await refresh();
      return true;
    } catch (e) {
      folderError.value = String(e);
      return false;
    }
  }

  /** 删除文件或目录 */
  async function removeItem(relPath: string, isDir: boolean): Promise<boolean> {
    try {
      await deleteVaultItem(relPath, isDir);
      await refresh();
      return true;
    } catch (e) {
      folderError.value = String(e);
      return false;
    }
  }

  /** 把文件移到另一个目录 */
  async function moveFile(relPath: string, targetDirRel: string): Promise<boolean> {
    try {
      await apiMoveVaultFile(relPath, targetDirRel);
      await refresh();
      return true;
    } catch (e) {
      folderError.value = String(e);
      return false;
    }
  }

  /** 把文件夹移动到另一个父目录下。targetParentRel 为空字符串表示移到根目录。 */
  async function moveDir(dirRel: string, targetParentRel: string): Promise<boolean> {
    try {
      await apiMoveVaultDir(dirRel, targetParentRel);
      await refresh();
      return true;
    } catch (e) {
      folderError.value = String(e);
      return false;
    }
  }

  return {
    files,
    dirs,
    tree,
    rootFiles,
    hasItems,
    fileCount,
    loading,
    error,
    folderError,
    refresh,
    createDir,
    renameItem,
    removeItem,
    moveFile,
    moveDir,
  };
});
