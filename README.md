<div align="center">

  <img src="docs/logo.png" alt="Leaf" width="120" />

# Leaf

Leaf 是一个轻量级的桌面应用，用于管理、浏览 HTML 文档。

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Platform: macOS](https://img.shields.io/badge/platform-macOS-lightgrey.svg)](#平台支持)

</div>

## 功能特性

- 本地文档库 — 导入 HTML 文件，自动复制到统一的本地库目录管理
- 自动元数据 — 导入时自动提取标题与摘要
- 沙箱渲染 — 通过沙箱 iframe 安全渲染任意 HTML 文档
- 目录提取 — 自动从文档中提取目录，快速跳转
- 本地索引 — 基于 SQLite 索引，启动即用，离线可用

## 下载安装

前往 [Releases 页面](https://github.com/pf711-dev/leaf/releases)，下载最新版本的： **macOS**：`Leaf_x.x.x_aarch64.dmg`（Apple Silicon）— 双击打开，将 Leaf 拖入 Applications 文件夹

> 首次打开若提示「Leaf 已损坏，无法打开」，这是因为应用暂未经 Apple 公证。可用以下任一方式解决：
> 1. 终端：执行 xattr -dr com.apple.quarantine /Applications/Leaf.app（路径按实际安装位置替换）
> 2. 系统设置：打开 系统设置 → 隐私与安全性，在底部找到 Leaf 的提示，点「仍要打开」

## 贡献

欢迎提交 Issue 反馈问题或建议新功能，也欢迎通过 Pull Request 贡献代码。

## 许可证

本项目基于 [Apache License 2.0](LICENSE) 开源，Copyright 2026 pf711-dev.
