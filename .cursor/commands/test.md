---
description: 运行测试（支持 free / pro / unit / e2e / vitest 模式）
---

根据参数选择测试模式。如果未指定参数，使用 free 模式。

可用模式：
- **free** (默认): Rust 免费版测试，`cargo test`（822+ passed, 0 failed, 24 ignored）
- **pro**: Rust Pro 版测试，`cargo test --features pro`（1094 passed, 0 failed, 44 ignored）
- **crate**: 单 crate 测试，`cargo test -p <crate_name>`
- **focused**: 聚焦测试，`cargo test <test_name>`（按名称匹配）
- **e2e**: E2E 桥接测试，`cd e2e-tests && npm run test:bridge`（955+ passed, 9 skipped）
- **vitest**: 前端单元测试，`cd e2e-tests && npm run test:unit`（407 passed, 27 files）
- **clippy-free**: Free Clippy，`cargo clippy --all-targets -- -D warnings`
- **clippy-pro**: Pro Clippy，`cargo clippy --all-targets --features pro -- -D warnings`（~6m39s）
- **fmt**: 格式检查，`cargo fmt --check`
- **tsc**: TypeScript 类型检查，`npx tsc --noEmit`
- **all**: 全量回归（Free + Pro + Clippy + fmt + tsc + Vitest + E2E）

参数: $ARGUMENTS

也可以直接指定测试名称运行：`cargo test -p echomind-core -- <测试名>`

运行后汇总通过/失败数量。如果失败，分析失败原因并修复。
注意：Pro clippy 耗时较长（~6min），非必要不运行。
