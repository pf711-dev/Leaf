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
