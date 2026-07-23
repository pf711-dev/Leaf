/** 仓库中的 HTML 文件元数据，由 Rust 侧 vault.rs 返回。 */
export interface VaultFile {
  id: string
  title: string
  fileName: string
  /** 从仓库根目录起算的相对路径，如 "slides/slide1.html" */
  relPath: string
  /** 磁盘上的绝对路径 */
  absPath: string
  fileSize: number
  summary: string
  /** 源文件最后修改时间，Unix 毫秒 */
  lastModified: number
  /** 文件所在目录相对路径，如 "slides"；根目录则为 "" */
  dirPath: string
}

/** 仓库中的一个目录。 */
export interface VaultDir {
  /** 从仓库根起算的相对路径，如 "slides/projects"；根目录自身用 "" */
  relPath: string
  name: string
  parentPath: string | null
  /** 层级深度（根目录下的一级 = 1） */
  level: number
}

/** 仓库信息。 */
export interface VaultInfo {
  rootPath: string
  name: string
}

/** 侧栏目录树节点（前端组装）。 */
export interface DirTreeNode {
  dir: VaultDir
  children: DirTreeNode[]
  files: VaultFile[]
}

// ==================== 兼容旧类型（逐步废弃） ====================

/** 文档元数据，对应 Rust 侧 db::Document。
 *  注意：字段名用 camelCase，因为 Rust 侧 serde 会把 snake_case 转成 camelCase
 *  （serde 默认就是 camelCase，除非加了 #[serde(rename_all = ...)]）。 */
export interface Document {
  id: string
  title: string
  fileName: string
  /** 库内相对路径，例如 "abc123.html" */
  libraryPath: string
  fileSize: number
  /** 提取出的纯文本摘要，用于卡片预览 */
  summary: string
  /** 导入时间，Unix 毫秒 */
  importedAt: number
  /** 原文件创建时间，Unix 毫秒 */
  sourceCreatedAt: number
  /** 所属文件夹 id；null/undefined 表示位于根目录 */
  folderId?: string | null
}

/** 一条「待导入文件与库内已有文档撞名」的预检结果。 */
export interface ConflictInfo {
  /** 待导入源文件的绝对路径 */
  sourcePath: string
  /** 撞名的原始文件名（如 "default.html"） */
  fileName: string
  /** 库中已存在文档的 id */
  existingDocId: string
  /** 库中已存在文档的标题（弹窗展示用） */
  existingTitle: string
}

/** 文件夹，对应 Rust 侧 db::Folder。含根目录在内最多 3 级。
 *  level = 1 表示根目录下的一级文件夹，2 表示二级，3 表示三级。 */
export interface Folder {
  id: string
  name: string
  /** 父文件夹 id；null 表示位于根目录（level = 1） */
  parentId: string | null
  /** 创建时间，Unix 毫秒 */
  createdAt: number
  /** 层级：1 / 2 / 3 */
  level: number
}

/** 树节点：文件夹 + 其子文件夹 + 其下文档。用于侧栏目录树渲染。 */
export interface FolderTreeNode {
  folder: Folder
  /** 直接子文件夹（递归） */
  children: FolderTreeNode[]
  /** 该文件夹下（非递归）的文档 */
  docs: Document[]
}

/** 导入文件夹的结果摘要。对应 Rust 侧 DirectoryImportResult。 */
export interface DirectoryImportResult {
  /** 成功导入的文档数 */
  importedCount: number
  /** 因根级同名已存在而跳过的文档数 */
  skippedCount: number
  /** 因原始目录超过 3 级被拍平到第 3 级的文档数 */
  flattenedCount: number
  /** 新建的文件夹数 */
  folderCount: number
  /** 收集到的首个失败原因（用于诊断）。空字符串表示无失败。 */
  firstError: string
  /** 因读取/解析/写入失败而被跳过的文件数 */
  failedCount: number
}
