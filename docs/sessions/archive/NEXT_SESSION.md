# EchoMind 下一会话提示词

> 复制以下 `---` 分隔线之间的全部内容粘贴到新会话即可。

---

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate（models → prompt → core → infra → tauri-app + compact + context + license-issuer），版本 v1.20.0。面向律师/研究员等高隐私场景用户。250 SRS REQ 全部完成。S1-S7 七轮审计+实施闭环全部完成（P0/P1/P2 共 16 项行动清单）。

## 测试基线

- Rust Free: 1282 passed / Rust Pro: 1573 passed / Clippy 零警告 / fmt 通过
- Vitest: 440 passed / tsc 0 errors / esbuild 1622 行
- E2E: 1163 passed, **205 failed (S5/S6 UI 重构遗留)**, 10 skipped
- IPC: 242 命令 / 幻影调用 0 / 僵尸命令 0

## 上一会话成果

完成了 Servo 浏览器引擎深入集成 EchoMind 的可行性调研。结论：**当前不建议集成**（ROI 1.5/10）。理由：Servo WebView 嵌入 API 尚未成熟（官方原话 "work is still ongoing"）；tauri-runtime-verso 近一年无更新（132 star）；硬编码 Origin header 破坏 Tauri capabilities 安全验证（对 EchoMind 隐私优先定位致命）；Tauri 版本鸿沟（EchoMind 2.0 vs verso 要求 =2.7.0）；150+ IPC 命令兼容性未验证；Servo 尚未 1.0（8 个 1.0 阻塞项）。重新评估触发条件：Servo 1.0 + verso 恢复活跃 + Origin 安全问题修复 + arm64 全平台支持。

## 已知问题

E2E 205 个失败均为 S5/S6 UI 重构遗留（顶栏 8→5、F 级模块删除、设置 35→8 分区导致选择器失效），涉及 dream-panel/trace-panel/graph-viewer/audit/bookmark/document-action 等测试文件。

## 开发规范

- 严禁 unwrap()/expect()/panic!()/unreachable!()，错误返回 Err
- 改源码后必须 `./sync-to-github.sh`
- 构建：`node scripts/build-ui.mjs` | 检查：`npx tsc --noEmit` | E2E：`cd e2e-tests && E2E_SPEED=0.2 npm run test:bridge`
- 前端编辑 `ui/src/*.js`，不要手改 `ui/index.html`（构建产物）
- 代理仅限开发调研，软件运行时禁止（reqwest 加 `.no_proxy()`）

## 下一阶段方向（三选一）

**方向 A（推荐）：修复 205 个 E2E 失败** — 更新测试选择器匹配 S5/S6 后 UI 结构，逐个 spec 文件修复，目标 0 failed

**方向 B：发布 v1.22.0** — 版本号 1.20.0→1.22.0 + CHANGELOG + release build + git tag

**方向 C：新功能** — 参考 `docs/SRS_v1.0.md` Needed REQ 或用户指定

## 必读文件

- `docs/sessions/CURRENT_STATE.md` — 完整状态快照（功能矩阵 + 验证命令 + 关键文件索引）
- `AGENTS.md` — 架构概览 + 数据流图 + 关键文件清单

---
