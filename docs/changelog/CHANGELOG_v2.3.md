# EchoMind v2.3.0 — 前端功能接线 + Agent 可视化 + 演示模式

**发布日期**：2026-08-18  
**版本号**：2.2.0 → 2.3.0（8 crate + tauri.conf.json）

## 新功能

### 1. REQ-RAG-051 无 Key 演示模式（前端接线）
- **向导「跳过，使用演示模式」按钮**：用户无需配置 API Key 即可体验基础 RAG 功能
- **演示模式提示栏**：顶部蓝色 banner 提示用户当前处于演示模式
- **自动退出**：用户配置 API Key 后自动退出演示模式（清除示例文档）
- **演示模式 IPC 封装**：`demoModeApi.isDemoMode()` / `demoModeApi.exit()` / `demoModeApi.loadDemoDocuments()`
- **启动流程集成**：boot() 检测演示模式状态，直接进入主界面
- **修复后端 `load_demo_documents`**：适配 `Document::new()` + `Chunk::new()`（替代旧字段 `title`/`content`/`file_type` 等）
- **修复后端 `exit_demo_mode`**：`doc.file_path.starts_with("demo://")` 替代 `doc.title.starts_with("[Demo] ")`

### 2. REQ-RAG-052 Agent 步骤可视化增强
- **`agent_step` 事件监听**：`chat.js` 新增 `listen('agent_step', ...)` 接收后端 Agent 步骤
- **Agent 步骤卡片**：`thinking-panel.js` 新增 `appendAgentStep(step)` — 每个步骤渲染为独立卡片
  - Thought（思考） — 紫色灯泡图标，自动展开
  - Action（行动） — 闪电图标，折叠
  - Observation（观察） — 眼睛图标，折叠
  - Answer（回答） — 对勾图标，自动展开
- **Agent 进度条**：`setAgentProgress(current, max)` — 显示第 N/最大轮次推理进度
- **卡片折叠/展开**：点击 header 切换，chevron 旋转动画
- **i18n**：5 个新键（agent_thought / agent_action / agent_observation / agent_answer / agent_progress）
- **CSS**：Agent 步骤卡片样式 + 进度条样式（`components.css`）

### 3. 前端功能接线验证
- **S81 文件夹拖拽导入**：后端 `expand_folder_paths()` + `traverse_dir()` 已实现（5 TDD 测试），前端拖拽事件已有
- **S82 嵌入模型下载境内加速**：后端 `MirrorSource` + 多源容错 + 重试退避 + 断点续传已实现（5 TDD 测试），前端镜像源选择器已有
- **P2-7 智能模式 RAG 参数调优**：`SMART_MODE_SETTINGS` 已包含 `rag.top_k=8` / `rag.score_threshold=0.05` / `rag.chunk_expansion_enabled=true`

## 验证结果

| 指标 | v2.2.0 | v2.3.0 | 变化 |
|---|---|---|---|
| Rust Free | 1540 passed | 1555 passed | +15 |
| Rust Pro | 1831 passed | 1846 passed | +15 |
| Clippy Free+Pro | 零警告 | 零警告 | ✅ |
| cargo fmt | 通过 | 通过 | ✅ |
| tsc | 0 errors | 0 errors | ✅ |
| esbuild | 1626 行 | 1641 行 | +15 行 |
| Vitest | 1076 passed | 1076 passed | ✅ |

## 修改文件清单

### 前端源码（7 文件）
- `ui/src/ipc.js` — 新增 `demoModeApi` 封装（3 方法）
- `ui/src/wizard.js` — 新增演示模式按钮事件 + LLM 配置保存后退出演示模式
- `ui/src/main.js` — 新增演示模式检测 + 提示栏 + `exitDemoModeIfActive()` 全局函数
- `ui/src/thinking-panel.js` — 新增 `appendAgentStep()` + `setAgentProgress()` + 4 个 Agent 图标常量
- `ui/src/chat.js` — 新增 `agent_step` 事件监听
- `ui/src/state.js` — 新增 `demoMode` 属性
- `ui/styles/components.css` — 新增演示模式 banner + Agent 步骤卡片 + 进度条样式

### 国际化（3 文件）
- `ui/locales/zh-CN.json` — 9 个新键
- `ui/locales/en.json` — 9 个新键
- `ui/index.html` — 新增 `wizDemoMode` 按钮 + esbuild 重建

### 后端修复（1 文件）
- `crates/tauri-app/src/commands/chat.rs` — 修复 `load_demo_documents_inner`（Document::new + Chunk）+ `exit_demo_mode_inner`（file_path 替代 title）+ `handle_demo_mode`（sources 类型修复 + emit_chat_token 参数修复）

### 类型声明（2 文件）
- `types/tauri.d.ts` — 新增 `exitDemoModeIfActive` 声明
- `types/dom-ext.d.ts` — 新增 `appendAgentStep` + `setAgentProgress` 方法声明

### E2E mock（1 文件）
- `e2e-tests/tests/bridge/tauri-stub.js` — 新增 3 个演示模式 mock + `demoMode` state

### 版本号（9 文件）
- 8 crate `Cargo.toml`：2.2.0 → 2.3.0
- `crates/tauri-app/tauri.conf.json`：2.2.0 → 2.3.0

### 文档（1 文件）
- `docs/api/ipc-commands.md` — 新增演示模式 3 个 IPC 命令
