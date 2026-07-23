<div align="center">

  <img src="docs/logo.png" alt="Leaf" width="120" />

# Leaf

**Leaf** 是一个轻量级的桌面应用，以本地文件夹为仓库，管理、浏览、编辑、演示 HTML 文档。

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Platform: macOS](https://img.shields.io/badge/platform-macOS-lightgrey.svg)](#平台支持)

<img src="docs/Leaf.png" alt="Leaf 产品介绍" />

</div>

## 功能特性

- **仓库模式**：打开本地文件夹作为仓库，自动索引所有 HTML 文件，无需手动导入
- **实时同步**：文件增删改即时反映，编辑保存直接覆盖原文件
- **自动元数据**：自动提取标题与摘要，告别未命名文档
- **沙箱渲染**：通过沙箱 iframe 安全渲染任意 HTML 文档
- **目录提取**：自动从文档中提取目录，快速跳转
- **文件夹管理**：侧边栏目录树，支持新建/重命名/删除文件夹（最多 3 级）
- **演示模式**：沉浸式全屏演示
- **编辑模式**：可直接编辑 HTML 内容并保存回原文件


## 下载安装

### 1. 下载
前往 [Releases 页面](https://github.com/pf711-dev/leaf/releases)，根据你的 Mac 芯片类型选择对应的安装包。

| 文件 | 适用芯片 | 说明 |
|------|---------|------|
| `Leaf_x.x.x_aarch64.dmg` | Apple Silicon（M1 ~ M4） | 标准安装包（推荐） |
| `Leaf_x.x.x_x64.dmg` | Intel Mac | 标准安装包（推荐） |
| `Leaf_aarch64.app.tar.gz` | Apple Silicon | 便携版，解压即用 |
| `Leaf_x64.app.tar.gz` | Intel Mac | 便携版，解压即用 |

### 2. 安装

**方式一（.dmg 标准安装，推荐）：**
双击 DMG 文件，将 Leaf 拖入 `Applications` 文件夹。

**方式二（.app.tar.gz 便携版）：**
解压后将 `.app` 文件放入 `Applications` 文件夹，或直接运行。

### 3. 首次打开
双击 Leaf 图标，系统可能会提示「Leaf 已损坏，无法打开」—— 这是 macOS 对未公证应用的安全限制，**不是应用本身的问题**。

请任选一种方式通过：

- **方式一（推荐）**：打开 `系统设置` → `隐私与安全性`，滚动到底部，找到 Leaf 的提示，点击 **「仍要打开」**。
- **方式二（终端）**：打开终端，粘贴运行以下命令后重试：
  ```bash
  xattr -dr com.apple.quarantine /Applications/Leaf.app
  ```

## 贡献

欢迎提交 Issue 反馈问题或建议新功能，也欢迎通过 Pull Request 贡献代码。

## 许可证

本项目基于 [Apache License 2.0](LICENSE) 开源，Copyright 2026 pf711-dev.
