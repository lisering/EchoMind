---
description: 构建 EchoMind 项目（Free / Pro / 前端）
---

根据参数构建 EchoMind 对应组件。如果未指定参数，构建 Free 版本。

构建命令：
- **Free**: `cargo build` — 免费版编译
- **Pro**: `cargo build --features pro` — Pro 版编译（含本地 LLM / 多模态 / 代码沙箱）
- **Release**: `cargo build --release --features pro` — Pro 发布构建
- **前端**: `node scripts/build-ui.mjs` — esbuild 打包 IIFE 生产构建
- **前端开发**: `node scripts/build-ui.mjs --dev` — 开发模式外部引用

参数: $ARGUMENTS

如果参数是 "free"，只构建 Free 版本。
如果参数是 "pro"，只构建 Pro 版本（需要 `--features pro`）。
如果参数是 "release"，构建 Pro Release 版本。
如果参数是 "ui" 或 "frontend"，构建前端。
如果参数为空或 "all"，先构建 Free 再构建前端。

构建完成后报告编译时间和警告数。如果出现编译错误，分析原因并修复。
注意：Pro 版本编译时间较长（~6min clippy），优先使用 Free 版本开发。
