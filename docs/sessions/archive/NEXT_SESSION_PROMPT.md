# S8 会话提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate，v1.20.0，250 SRS REQ 全部完成。S1-S7 七轮审计+实施闭环全部完成（P0/P1/P2 共 16 项行动清单）。

## 测试基线

- Rust: 1269 passed / Clippy 零警告 / fmt 通过
- Vitest: 440 passed / tsc 0 errors / esbuild 1622 行
- E2E: 1163 passed, **205 failed (S5/S6 遗留)**, 10 skipped
- IPC: 242 命令 / 幻影调用 0 ✅

## 已知问题

E2E 205 个失败均为 S5/S6 UI 重构遗留（顶栏 8→5 按钮、F 级模块删除、设置 35→8 分区导致选择器失效），涉及 dream-panel/trace-panel/graph-viewer/audit/bookmark/document-action 等测试文件。

## 开发规范

- 严禁 unwrap()/expect()/panic!()
- 改源码后必须 `./sync-to-github.sh`
- 构建：`node scripts/build-ui.mjs` | 检查：`npx tsc --noEmit` | E2E：`cd e2e-tests && E2E_SPEED=0.2 npm run test:bridge`

## 下一阶段三选一

**方向 A（推荐）：修复 205 个 E2E 失败** — 更新测试选择器匹配 S5/S6 后 UI 结构，目标 0 failed

**方向 B：发布 v1.22.0** — 更新版本号 + CHANGELOG + release build + git tag

**方向 C：新功能 v1.23** — 参考 `docs/SRS_v1.0.md` Needed REQ

## 必读文件

`docs/sessions/CURRENT_STATE.md` — 项目状态快照（完整功能矩阵 + 验证命令 + 关键文件索引）
