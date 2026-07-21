import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { ConflictInfo, DirectoryImportResult, Document, Folder, FolderTreeNode } from "../types/document";
import {
  checkImportConflicts,
  createFolder as apiCreateFolder,
  deleteDocument,
  deleteFolder as apiDeleteFolder,
  importDirectory as apiImportDirectory,
  importFiles,
  listDocuments,
  listFolders,
  moveDocument as apiMoveDocument,
  moveFolder as apiMoveFolder,
  renameDocument as apiRenameDocument,
  renameFolder as apiRenameFolder,
} from "../api/client";
import type { ConflictResolution } from "../api/client";

export const useDocumentsStore = defineStore("documents", () => {
  /** 全部文档，按导入时间倒序 */
  const documents = ref<Document[]>([]);
  /** 全部文件夹，按创建时间正序 */
  const folders = ref<Folder[]>([]);
  /** 是否正在加载列表 */
  const loading = ref(false);
  /** 列表加载错误信息（空表示无错误）。仅用于 refresh 失败，会覆盖整个侧栏。 */
  const error = ref("");
  /**
   * 文件夹操作错误信息（新建/重命名/删除/移动失败时设置）。
   * 与 error 区分：这个只在右下角短暂提示，不影响目录树渲染。
   * 由组件层在显示后自动清空。
   */
  const folderError = ref("");
  /** 正在导入中的文件数量（用于 UI 反馈） */
  const importing = ref(0);

  /**
   * 把扁平的 folders + documents 组装成树（根 = parentId 为 null 的一级文件夹）。
   * 根目录下散落的文档（folderId 为空）单独返回，由调用方决定如何展示。
   */
  const tree = computed<FolderTreeNode[]>(() => {
    const buildNodes = (parentId: string | null): FolderTreeNode[] => {
      return folders.value
        .filter((f) => f.parentId === parentId)
        .map((folder) => ({
          folder,
          children: buildNodes(folder.id),
          docs: documents.value.filter((d) => d.folderId === folder.id),
        }));
    };
    return buildNodes(null);
  });

  /** 根目录下散落的文档（folderId 为空/null）。 */
  const rootDocuments = computed<Document[]>(() =>
    documents.value.filter((d) => !d.folderId),
  );

  /** 是否有文件夹或文档 */
  const hasItems = computed(() => documents.value.length > 0 || folders.value.length > 0);

  /** 拉取文档列表 */
  async function refreshDocuments() {
    loading.value = true;
    error.value = "";
    try {
      documents.value = await listDocuments();
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  /** 拉取文件夹列表 */
  async function refreshFolders() {
    try {
      folders.value = await listFolders();
    } catch (e) {
      error.value = String(e);
    }
  }

  /** 一次性刷新文档 + 文件夹 */
  async function refresh() {
    await Promise.all([refreshDocuments(), refreshFolders()]);
  }

  /** 预检导入冲突：返回目标文件夹内的撞名清单（由组件层决定如何弹窗）。 */
  async function checkConflicts(paths: string[], folderId?: string | null): Promise<ConflictInfo[]> {
    return checkImportConflicts(paths, folderId);
  }

  /**
   * 执行导入。resolution 由用户在冲突弹窗中选择后传入。
   * folderId 指定目标文件夹（null/undefined 表示根目录）。
   * 返回成功导入的文档数量。
   */
  async function importWithResolution(
    paths: string[],
    resolution?: ConflictResolution,
    folderId?: string | null,
  ): Promise<number> {
    if (paths.length === 0) return 0;
    importing.value += paths.length;
    try {
      const imported = await importFiles(paths, resolution, folderId ?? null);
      await refresh();
      return imported.length;
    } finally {
      importing.value = 0;
    }
  }

  /** 删除文档（库文件 + 数据库记录） */
  async function remove(id: string, libraryPath: string) {
    await deleteDocument(id, libraryPath);
    documents.value = documents.value.filter((d) => d.id !== id);
  }

  /**
   * 导入整个文件夹（连同目录结构）。后端递归遍历并重建文件夹层级。
   * 失败时设置 folderError（右下角 toast），不污染目录树渲染。
   */
  async function importDirectory(rootPath: string, parentFolderId?: string | null): Promise<DirectoryImportResult | null> {
    importing.value += 1;
    try {
      const result = await apiImportDirectory(rootPath, parentFolderId);
      await refresh();
      return result;
    } catch (e) {
      folderError.value = String(e);
      return null;
    } finally {
      importing.value = 0;
    }
  }

  // -------------------- 文件夹操作 --------------------

  /** 新建文件夹。parentId 为空表示在根目录下创建（level 1）。 */
  async function createFolder(name: string, parentId?: string | null): Promise<Folder | null> {
    try {
      const folder = await apiCreateFolder(name, parentId ?? null);
      await refreshFolders();
      return folder;
    } catch (e) {
      folderError.value = String(e);
      return null;
    }
  }

  /** 重命名文件夹 */
  async function renameFolder(id: string, newName: string): Promise<boolean> {
    try {
      await apiRenameFolder(id, newName);
      await refreshFolders();
      return true;
    } catch (e) {
      folderError.value = String(e);
      return false;
    }
  }

  /**
   * 重命名文档（newName 为不含后缀的主名）。
   * 后端会同步重命名库内磁盘文件。这里用全量刷新重新拉 documents，
   * 调用方需在 currentDoc 可能受影响时自行按 id 重新绑定（见 App.vue）。
   */
  async function renameDocument(id: string, newName: string): Promise<boolean> {
    try {
      await apiRenameDocument(id, newName);
      await refreshDocuments();
      return true;
    } catch (e) {
      folderError.value = String(e);
      return false;
    }
  }

  /** 删除文件夹（递归删除其下所有子文件夹与文档） */
  async function deleteFolder(id: string): Promise<boolean> {
    try {
      await apiDeleteFolder(id);
      await refresh();
      return true;
    } catch (e) {
      folderError.value = String(e);
      return false;
    }
  }

  /**
   * 把文档移动到指定文件夹（folderId 为 null 表示移到根目录）。
   * 镜像化后移动会改变 libraryPath（磁盘文件搬到目标文件夹的子目录），
   * 后端不返回新 libraryPath，这里用全量刷新保证一致；调用方需在 currentDoc
   * 可能受影响时自行按 id 重新绑定（见 App.vue）。
   */
  async function moveDocument(docId: string, folderId: string | null): Promise<boolean> {
    try {
      await apiMoveDocument(docId, folderId);
      await refreshDocuments();
      return true;
    } catch (e) {
      folderError.value = String(e);
      return false;
    }
  }

  /** 把文件夹移动到另一个父级下（newParentId 为 null 表示移到根目录） */
  async function moveFolder(folderId: string, newParentId: string | null): Promise<boolean> {
    try {
      await apiMoveFolder(folderId, newParentId);
      await refresh();
      return true;
    } catch (e) {
      folderError.value = String(e);
      return false;
    }
  }

  return {
    documents,
    folders,
    tree,
    rootDocuments,
    hasItems,
    loading,
    error,
    folderError,
    importing,
    refresh,
    refreshDocuments,
    refreshFolders,
    checkConflicts,
    importWithResolution,
    importDirectory,
    remove,
    createFolder,
    renameFolder,
    renameDocument,
    deleteFolder,
    moveDocument,
    moveFolder,
  };
});
