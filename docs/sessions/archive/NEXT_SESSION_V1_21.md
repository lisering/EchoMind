# EchoMind v1.21 Session 提示词

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.20.0。SRS 250 REQ，完成率 99.2%（100 Done + 148 Implemented + 1 Draft + 1 Deprecated）。剩余 1 Draft：REQ-DS-004（图标系统 P2）。

## 本 Session 目标

实施 **REQ-DS-004 SVG inline 图标系统**（SRS 第 2403 行），替代当前 Unicode 字符方案，确保跨平台一致渲染。

### AC 验收标准

1. 所有交互元素使用 SVG inline 图标替代 Unicode 字符（`＋`/`⚙`/`■`/`◈`/`×`/`↻`/`⇩` 等）
2. 图标尺寸三档：sm 16px / md 20px / lg 24px（CSS 类 `.icon-sm`/`.icon-md`/`.icon-lg`）
3. disabled 状态图标 `opacity: 0.4`
4. 图标继承 `currentColor`（与现有 Tailwind `hover:text-accent` 体系兼容）
5. 零新增依赖（纯 SVG inline，不引入图标库）

### 当前图标方案（待替换）

| 使用位置 | 当前字符 | 元素 ID |
|---|---|---|
| 导入文件 | `＋` | `#plusBtn` |
| 折叠/展开 | `‹` / `›` | `#collapseBtn` |
| 新对话 | `＋` | `#newChatBtn` |
| 设置 | `⚙` | `#settingsBtn` |
| 停止 | `■` | `#stopBtn` |
| 拖拽 | `⇩` | `#dragOverlay` |
| 品牌图标 | `◈` | 空状态 / Paywall |
| 删除 | `×` | 文档/会话列表项 |
| 重试 | `↻` | Failed 文档项 |
| 审计 | `🔍` | Indexed 文档项 |
| 警告 | `⚠️` | VLM 弹窗 |
| 复制/删除菜单 | `📋`/`🗑` | 右键菜单 |

### 实现步骤

1. 新建 `ui/src/icons.js` — SVG 图标常量库（` ICONS = { plus, collapse, expand, settings, stop, drag, brand, delete, retry, audit, warning, copy, trash, ... }`，每个图标返回 SVG string with `currentColor` + `width`/`height` 可配）
2. 新建 `ui/styles/icons.css` — `.icon-sm`/`.icon-md`/`.icon-lg` 尺寸类 + `opacity: 0.4` disabled 规则
3. 全局替换：`ui/src/*.js` + `ui/index.html` 中所有 Unicode 图标字符替换为 `ICONS.xxx` 调用或 inline SVG
4. TDD：`e2e-tests/tests/v1.21-features.spec.ts` — E2E 验证 SVG 元素存在 + 尺寸正确 + disabled opacity
5. 更新 SRS REQ-DS-004 Draft → Implemented + changelog
6. 更新 `docs/sessions/CURRENT_STATE.md`
7. `./sync-to-github.sh --commit`

## 开发规范（铁律摘要）

- TDD 先红后绿 | 零 unwrap/expect/panic | 改前端后 `node scripts/build-ui.mjs` 重建
- 文档同步：SRS + CURRENT_STATE | 铁律十一：改源码后必须 `./sync-to-github.sh`
- 样式铁律十三：Tailwind 工具类 + design tokens，不硬编码 hex/z-index

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test -p echomind-tauri-app
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npx playwright test tests/v1.21-features.spec.ts
./sync-to-github.sh --commit
```

## 测试基线

Rust Free 1282 passed / 31 ignored | Clippy 零警告 | tsc 0 errors | esbuild 19165 行 | IPC 220 Free / 230 Pro

## 关键文件

| 文件 | 用途 |
|---|---|
| `ui/src/icons.js` | **新建** SVG 图标常量库 |
| `ui/styles/icons.css` | **新建** 图标尺寸 + disabled 样式 |
| `ui/src/main.js` | 替换 `＋`/`⚙` 等字符为 SVG |
| `ui/src/sidebar.js` | 替换 `‹`/`›`/`×`/`↻`/`🔍` |
| `ui/src/chat.js` | 替换 `■` 停止按钮 |
| `ui/src/settings.js` | 替换 `⚙` 设置按钮 |
| `ui/src/context-menu.js` | 替换 `📋`/`🗑` |
| `ui/index.html` | 替换 `⇩`/`◈`/`⚠️` 等 |
| `docs/SRS_v1.0.md` | REQ-DS-004 第 2403 行，改状态 + changelog |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（更新） |

## 上一 Session 摘要

v1.21（本 Session 上一个）实施 REQ-NFR-019 构建产物体积优化。`Cargo.toml` 新增 `[profile.release]`（`lto = "fat"` + `strip = true` + `codegen-units = 1`）；裁剪 5 个 KaTeX 冷门字体（Fraktur + SansSerif，~57KB），ui/vendor/ 从 5.04MB 降至 4.98MB ≤ 5MB；新增 `bundle_size.rs` 3 个 TDD 测试 + `check-bundle-size.sh` CI 脚本。SRS Draft 从 2 减至 1（仅剩 DS-004）。
