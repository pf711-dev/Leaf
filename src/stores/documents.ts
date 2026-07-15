import { defineStore } from "pinia";
import { ref } from "vue";
import type { ConflictInfo, Document } from "../types/document";
import {
  checkImportConflicts,
  deleteDocument,
  importFiles,
  listDocuments,
} from "../api/client";
import type { ConflictResolution } from "../api/client";

export const useDocumentsStore = defineStore("documents", () => {
  /** 全部文档，按导入时间倒序 */
  const documents = ref<Document[]>([]);
  /** 是否正在加载列表 */
  const loading = ref(false);
  /** 列表加载错误信息（空表示无错误） */
  const error = ref("");
  /** 正在导入中的文件数量（用于 UI 反馈） */
  const importing = ref(0);

  /** 拉取文档列表 */
  async function refresh() {
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

  /** 预检导入冲突：返回撞名清单（由组件层决定如何弹窗）。 */
  async function checkConflicts(paths: string[]): Promise<ConflictInfo[]> {
    return checkImportConflicts(paths);
  }

  /**
   * 执行导入。resolution 由用户在冲突弹窗中选择后传入。
   * 返回成功导入的文档数量。
   */
  async function importWithResolution(
    paths: string[],
    resolution?: ConflictResolution,
  ): Promise<number> {
    if (paths.length === 0) return 0;
    importing.value += paths.length;
    try {
      const imported = await importFiles(paths, resolution);
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

  return {
    documents,
    loading,
    error,
    importing,
    refresh,
    checkConflicts,
    importWithResolution,
    remove,
  };
});
