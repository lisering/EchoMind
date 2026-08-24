# EchoMind v1.18 Session 提示词

> 复制下方 --- 之间的全部内容作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.17.0，1279 Rust passed / 0 failed / 31 ignored。211 Free IPC / 223 Pro。esbuild 18388 行。SRS 250 REQ（100 Done + 141 Implemented + 8 Draft + 1 Deprecated，完成率 96.4%）。

上一 Session（v1.17）：SRS 状态修正 5 REQ → Implemented + REQ-A11Y-005 高对比度模式 + REQ-RAG-018 用户提问建议 + REQ-EXP-002/003 全量数据备份与恢复。10 E2E 全通过。

## 本 Session 任务：命令面板 + 全局搜索（2 个高 ROI 纯前端 Draft REQ）

### 功能 1: REQ-KB-002 命令面板（Cmd+K）

**SRS 描述**：`Cmd/Ctrl+K` 打开全局命令面板，模糊搜索快速执行命令（新建会话、导入文件、打开设置、切换混合检索等）。面板居中浮于主界面，半透明遮罩。

**实现要点**：
- 新建 `ui/src/command-palette.js` 模块
- 模糊搜索：简单的 includes 匹配即可，无需引入 fuzzysort 库
- 命令注册：复用已有 `ui/src/slash-commands.js` 的命令列表（系统指令 + 自定义模板）
- 键盘导航：上下箭头选择 + Enter 执行 + Esc 关闭
- 面板打开时输入框自动聚焦，关闭后焦点回到之前元素（复用 `ui/src/focus-trap.js`）
- 半透明遮罩 + 居中布局（复用 `ui/src/panel-stack.js`）

**AC 验收标准**：
1. `Cmd/Ctrl+K` 打开面板，居中显示
2. 输入关键词后命令列表实时模糊过滤
3. 上下箭头选择命令，Enter 执行
4. Esc 关闭面板
5. 支持中文搜索
6. 面板打开时自动聚焦，关闭后焦点回到之前元素

### 功能 2: REQ-NAV-002 全局搜索（Cmd+Shift+F）

**SRS 描述**：`Cmd/Ctrl+Shift+F` 打开全局搜索，同时搜索会话标题、文档名、消息内容。结果按类型分组（会话/文档/消息），每组前 5 条。点击跳转到对应位置。

**实现要点**：
- 新建 `ui/src/global-search.js` 模块
- 复用已有 IPC：`search_conversations`（FTS5 消息搜索）+ `get_documents`（文档列表过滤）
- 三类结果分组渲染：会话标题匹配 + 文档名匹配 + 消息内容匹配
- 点击会话 → `loadConversation()`；点击文档 → 打开预览；点击消息 → 加载会话+滚动定位
- 关键词高亮：`<mark>` 标签包裹匹配文本
- 键盘导航同命令面板

**AC 验收标准**：
1. `Cmd/Ctrl+Shift+F` 打开全局搜索面板
2. 结果按类型分组（会话/文档/消息），每组最多 5 条
3. 点击会话结果加载该会话
4. 点击文档结果打开预览
5. 点击消息结果加载会话并滚动定位
6. 关键词在结果中高亮
7. 无结果时显示空状态

## 执行步骤

1. TDD 实现 REQ-KB-002（先写 E2E 红灯测试 → 实现 → 绿灯）
2. TDD 实现 REQ-NAV-002（同上）
3. 验证全链路
4. 更新 SRS 状态 + CURRENT_STATE.md
5. `./sync-to-github.sh --commit`

## E2E 测试要求

创建 `e2e-tests/tests/v1.18-features.spec.ts`，覆盖：
- 命令面板：打开/搜索/键盘导航/执行/Esc 关闭（~5 测试）
- 全局搜索：打开/搜索/分组显示/空状态/Esc 关闭（~5 测试）

E2E 模式：`injectStub(page)` → `page.addInitScript(配置 mock state)` → `injectLocales(page)` → `page.goto(uiUrl)` → 操作 UI → 断言。参考 `e2e-tests/tests/helpers.mjs`。

## 开发规范

- TDD：先写红灯测试再实现
- 零 unwrap/expect/panic（workspace deny lint）
- Tailwind design tokens，禁止硬编码 hex
- 前端构建：改 `ui/src/*.js` 后运行 `node scripts/build-ui.mjs`

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npx playwright test tests/v1.18-features.spec.ts --reporter=line
./sync-to-github.sh --commit
```

## 当前测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1279 passed, 31 ignored |
| Clippy | 零警告 |
| tsc | 0 errors |
| esbuild | 18388 行 |
| 版本号 | 1.17.0 |
| IPC | 211 Free / 223 Pro |

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格（REQ-KB-002 第 2948 行，REQ-NAV-002 第 3079 行） |
| `ui/src/slash-commands.js` | 已有命令列表（复用） |
| `ui/src/focus-trap.js` | 焦点管理（复用） |
| `ui/src/panel-stack.js` | 面板栈管理（复用） |
| `ui/src/zindex.js` | Z-index 常量（复用） |
| `ui/src/sidebar.js` | 会话搜索（参考实现） |
| `e2e-tests/tests/helpers.mjs` | E2E 工具函数 |
| `ui/styles/tokens.css` | CSS 变量 |

---
