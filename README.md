<div align="center">

  <img src="docs/logo.png" alt="Leaf" width="120" />

# Leaf

基于 Tauri + Vue 3 的本地 HTML 文档阅读库

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Platform: macOS](https://img.shields.io/badge/platform-macOS-lightgrey.svg)](#平台支持)

</div>

Leaf 是一个轻量级的桌面应用，用来在本地管理、浏览 HTML 文档。所有文档都保存在你自己的电脑上，不依赖任何云服务。

## ✨ 功能特性

- 📂 **本地文档库** — 导入 HTML 文件，自动复制到统一的本地库目录管理
- 📑 **自动元数据** — 导入时自动提取标题与摘要
- 🔒 **沙箱渲染** — 通过沙箱 iframe 安全渲染任意 HTML 文档
- 📑 **目录提取** — 自动从文档中提取目录（TOC），快速跳转
- 💾 **本地索引** — 基于 SQLite 索引，启动即用，离线可用
- 🎨 **原生体验** — macOS 原生圆角窗口，贴合系统观感

## 📥 下载安装

### 方式一：下载预编译包（推荐普通用户）

前往 [Releases 页面](https://github.com/pf711-dev/leaf/releases)，下载最新版本的：

- **macOS**：`Leaf_x.x.x_aarch64.dmg`（Apple Silicon）— 双击打开，将 Leaf 拖入 Applications 文件夹

> 首次打开若提示"无法验证开发者"，前往 **系统设置 → 隐私与安全性**，点击"仍要打开"。

### 方式二：从源码构建

见下方 [🛠️ 从源码构建](#️-从源码构建) 章节。

## 🛠️ 从源码构建

### 前置依赖

| 依赖 | 说明 |
| --- | --- |
| [Node.js](https://nodejs.org/) | 建议 18 及以上 |
| [Rust](https://www.rust-lang.org/tools/install) | 通过 rustup 安装，stable 通道 |
| Xcode Command Line Tools | 终端执行 `xcode-select --install` |

### 步骤

```bash
# 1. 克隆仓库
git clone https://github.com/pf711-dev/leaf.git
cd leaf

# 2. 安装前端依赖
npm install

# 3. 开发模式（热重载，访问 http://localhost:1420）
npm run tauri dev

# 4. 正式构建（产物在 src-tauri/target/release/bundle/）
npm run tauri build
```

构建产物：

- `src-tauri/target/release/bundle/macos/Leaf.app` — macOS 应用包
- `src-tauri/target/release/bundle/dmg/Leaf_x.x.x_aarch64.dmg` — macOS 分发包

## 🧱 技术栈

| 层 | 技术 |
| --- | --- |
| 桌面框架 | [Tauri 2](https://tauri.app/)（Rust 后端） |
| 前端 | [Vue 3](https://vuejs.org/) + TypeScript + [Vite](https://vitejs.dev/) |
| 状态管理 | [Pinia](https://pinia.vuejs.org/) |
| 本地存储 | SQLite（`rusqlite`） |
| HTML 解析 | [`scraper`](https://crates.io/crates/scraper) |

## 📁 项目结构

```
.
├── src/                    # Vue 3 前端
│   ├── components/         # UI 组件
│   ├── stores/             # Pinia 状态
│   ├── api/                # Tauri invoke 封装
│   └── utils/              # 工具函数（TOC 提取、格式化等）
├── src-tauri/              # Rust 后端
│   └── src/
│       ├── commands.rs     # Tauri 命令（导入/列表/读取/删除）
│       ├── db.rs           # SQLite 数据层
│       ├── library.rs      # 本地文档库目录管理
│       └── parser.rs       # HTML 标题/摘要提取
└── docs/                   # 文档与资源
```

## 🖥️ 平台支持

当前主要面向 **macOS**（Apple Silicon / Intel）。应用使用了 macOS 圆角窗口插件与 `cocoa`/`objc` 依赖，Windows / Linux 暂未充分验证，欢迎贡献者帮助适配。

## 🤝 贡献

欢迎提交 Issue 反馈问题或建议新功能，也欢迎通过 Pull Request 贡献代码。

1. Fork 本仓库
2. 创建特性分支（`git checkout -b feature/xxx`）
3. 提交更改（`git commit -m 'Add xxx'`）
4. 推送到分支（`git push origin feature/xxx`）
5. 发起 Pull Request

## 📄 许可证

本项目基于 [Apache License 2.0](LICENSE) 开源。

Copyright 2026 pf711-dev
