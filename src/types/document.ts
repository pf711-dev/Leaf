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
