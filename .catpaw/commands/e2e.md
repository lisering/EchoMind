---
description: E2E 桥接测试（Playwright + tauri-stub.js）
---

运行 EchoMind E2E 桥接测试，验证前端 UI + IPC mock 全链路。

执行命令：`cd e2e-tests && npm run test:bridge`

参数: $ARGUMENTS（可选）
- 无参数：运行全部 E2E 测试（955+ passed, 9 skipped）
- 指定文件名：如 "memory-panel"，运行 `npx playwright test tests/bridge/memory-panel.spec.ts`
- 指定 "unit"：运行 `npm run test:unit`（Vitest 前端单元测试，407 passed）
- 指定 "headed"：运行 `npx playwright test --headed`（有头模式，用于调试）

运行前确认：
1. 前端已构建：`node scripts/build-ui.mjs`
2. tauri-stub.js 已生成：`e2e-tests/tests/bridge/tauri-stub.js`
3. `npx tsc --noEmit` 类型检查通过

运行后报告：通过/失败/跳过数量。如果失败，分析失败原因并修复。
