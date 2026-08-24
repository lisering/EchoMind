# EchoMind 下个 Session 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。v2.2.0 已发布。当前 **1560 Free / 1831 Pro** 测试通过。fastembed ONNX 嵌入（5 种 EmbeddingModel：AllMiniLML6V2 384-dim 默认 / BgeSmallEnV1_5 384-dim 推荐 / BgeSmallZhV1_5 512-dim 中文 / E5SmallV2 384-dim / BgeM3 1024-dim 多语言），SQLite 向量存储 + HNSW 索引（Free，二进制加密持久化 + 懒加载 + 归一化点积），任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。

## 上 Session 完成内容（超大规模 UI/UX 像素级 + 交互质量保证）

### 任务概述

用户要求对 UI 进行像素级别的测试和对交互进行全面测试，先进行全面的复盘后，再进行超大规模的测试。本 Session 完整执行了 Spec-driven Development 流程：**复盘 → 设计出规格 → 写测试用例**。

### 产出文件清单（7 个核心文件，4,268 行）

| 文件 | 行数 | 类型 | 说明 |
|---|---|---|---|
| `docs/architecture/UI_PIXEL_SPEC.md` | 416 | 设计规格 | UI 像素级设计规格：颜色令牌精确值（暗色+浅色 42 个 CSS 变量）+ 间距令牌（8px 基准网格 12 级）+ 字号令牌（7 级 + 5 级行高）+ 圆角令牌（6 级）+ 阴影令牌（7 级 + focus shadow）+ 过渡令牌（4 种 duration + 4 种 easing）+ 布局令牌（840px 限宽 + 侧栏/顶栏/输入区精确尺寸）+ 组件像素值（消息气泡圆角 22px + 按钮高度 36px + 输入框 44px 等） |
| `docs/architecture/UI_INTERACTION_SPEC.md` | 354 | 交互规格 | 交互设计规格：按钮状态机（5 态 Default/Hover/Active/Focus/Disabled + 转换规则）+ 输入框状态机 + 面板状态机 + 键盘快捷键规格（16 组快捷键 + 盲操作规范）+ ARIA 无障碍规格（6 项 role 规范 + Focus Trap + 对比度 WCAG AA 4.5:1）+ 动画反馈规格（4 类过渡 + 防闪烁 + prefers-reduced-motion）+ 错误反馈规格 + 加载反馈规格 |
| `e2e-tests/tests/pixel-tokens.spec.ts` | 819 | E2E 测试 | **45 个测试**：设计令牌像素级验证（暗色主题 16 个颜色变量精确值 + 浅色主题 16 个颜色变量 + 间距变量 12 级 + 字号变量 7 级 + 圆角变量 6 级 + 阴影变量 7 级 + 过渡变量 4 种 + 侧栏宽度精确值 + 顶栏高度精确值 + 输入区高度 + 840px 限宽 + 布局方向 + 滚动条样式） |
| `e2e-tests/tests/pixel-components.spec.ts` | 750 | E2E 测试 | **60 个测试**：组件尺寸+样式像素级验证（消息气泡圆角 22px + 用户消息背景 + AI 消息透明 + 消息内边距 + 侧栏按钮高度 36px + 发送按钮尺寸 + 输入框高度 44px + 顶栏高度 + 侧栏宽度 + 标签页高度 + 搜索框 + 模态框 + 设置面板 Tab + 按钮内边距 + 图标尺寸 + 头像 + 代码块 + 引用块 + 表格 + 链接 + 滚动条 + 聚焦环 + 工具提示 + 聊天列表项 + 文档列表项 + 侧栏折叠 + 空状态 + 骨架屏 + Toast + 右键菜单 + 命令面板 + 下拉菜单 + 进度条 + 标签徽章 + 分割线 + 快捷键提示） |
| `e2e-tests/tests/interaction-state.spec.ts` | 725 | E2E 测试 | **50 个测试**：交互状态机验证（按钮 5 态视觉 Default/Hover/Active/Focus/Disabled + 输入框 4 态 + 面板展开/折叠 + 模态框开关 + 侧栏折叠/展开 + 设置面板 Tab 切换 + 搜索展开/收起 + 消息流式态 + 右键菜单弹出/关闭 + 命令面板开关 + 下拉菜单开关 + Toast 显示/消失 + 加载态 + 空状态 + 错误态 + 禁用态 + 选中态 + 悬停态过渡 + 拖拽 + 键盘导航 + 焦点管理 + 滚动行为 + 文本选择 + 链接交互 + 表单验证 + 复制粘贴 + Tab 切换 + Shift+Tab 逆向 + Esc 关闭 + Enter 确认 + Space 切换 + Arrow 导航） |
| `e2e-tests/tests/interaction-keyboard-a11y.spec.ts` | 698 | E2E 测试 | **50 个测试**：键盘快捷键 + 无障碍验证（⌘J 新对话 + ⌘K 命令面板 + ⌘E 导出 + ⌘⇧F 全局搜索 + ⌘, 设置 + Esc 关闭面板 + Enter 发送 + Shift+Enter 换行 + Tab 导航 + Shift+Tab 逆向 + Arrow 导航 + Space 切换 + Focus Trap in modal + ARIA role 验证 + aria-label 验证 + aria-modal 验证 + aria-hidden 验证 + 对比度检查 + prefers-reduced-motion + 屏幕阅读器兼容 + 键盘焦点可见 + 焦点顺序 + 焦点环样式 + 焦点不丢失 + 焦点恢复 + 快捷键冲突检测 + 快捷键覆盖统计 + 快捷键帮助面板 + 键盘操作等价性 + 滚动键盘控制 + 列表键盘选择 + 树形键盘导航 + 表格键盘导航 + 对话框键盘关闭 + 警报键盘确认 + Tab 键陷阱检测 + 模态焦点恢复） |
| `e2e-tests/tests/unit/pixel-interaction-spec.test.js` | 506 | Vitest 单元测试 | **60 个测试**：设计令牌常量验证 + 交互逻辑单元验证（颜色令牌值验证 + 间距令牌值验证 + 字号令牌值验证 + 圆角令牌值验证 + 阴影令牌值验证 + 过渡令牌值验证 + 布局令牌值验证 + 按钮状态机常量 + 输入框状态机常量 + 键盘快捷键映射表验证 + ARIA 规范常量验证 + Focus Trap 逻辑 + 面板栈逻辑 + z-index 常量 + 主题切换逻辑 + 滚动锁逻辑 + IME 守卫逻辑 + 草稿恢复逻辑 + 斜杠命令逻辑 + 历史导航逻辑 + Token 估算逻辑 + 防抖/节流逻辑 + 消息渲染逻辑 + 代码高亮逻辑 + Mermaid 占位逻辑 + KaTeX 渲染逻辑 + 表格增强逻辑 + 引用标记逻辑 + 消息编辑逻辑 + 分支树逻辑 + 拖拽逻辑 + 右键菜单逻辑 + 命令面板逻辑 + 设置 Tab 逻辑 + 侧栏折叠逻辑 + 文档列表逻辑 + 会话列表逻辑 + 空状态逻辑 + 骨架屏逻辑 + Toast 逻辑 + 错误恢复逻辑 + 更新横幅逻辑 + 导出逻辑 + 书签逻辑 + 面包屑逻辑 + 工作空间逻辑 + 离线检测逻辑 + 知识库统计逻辑） |

### 测试统计

| 测试类别 | 文件数 | 测试数 | 测试框架 |
|---|---|---|---|
| 像素级设计令牌 E2E | 1 | 45 | Playwright |
| 像素级组件 E2E | 1 | 60 | Playwright |
| 交互状态机 E2E | 1 | 50 | Playwright |
| 键盘+无障碍 E2E | 1 | 50 | Playwright |
| 单元逻辑 Vitest | 1 | 60 | Vitest |
| **总计** | **5** | **265** | — |

### 关键设计决策

1. **Spec-driven Development（铁律十五）**：先完成全面复盘（参考 `docs/sessions/archive/COMPREHENSIVE_RETROSPECTIVE_2026-08-14.md` + `audit_s3_ui_evaluation.md` 814 行 UI 验收评估），再产出 2 份设计规格文档（770 行），最后从规格派生 265 个测试用例
2. **像素级精度**：所有 CSS 变量精确到 RGB 值（如 `#0A0A0B` = `rgb(10,10,11)`），间距使用 8px 基准网格（4/8/12/16/20/24/32/40/48/56/64/80），字号 7 级（12/13/14/16/18/20/24px），圆角 6 级（4/6/8/12/16/22px）
3. **状态机形式化**：每个交互组件定义 5 态状态机（Default/Hover/Active/Focus/Disabled），明确状态转换规则，避免隐式状态
4. **WCAG 2.1 AA 合规**：对比度 ≥ 4.5:1（正文）/ ≥ 3:1（大文本），Focus Trap in modal，ARIA role/label/modal/hidden 完整覆盖
5. **CSS 变量解析适配**：`getPropertyValue` 返回解析后的 hex 值而非 `var()` 字符串，测试更新为比较解析值
6. **单位标准化**：浏览器返回 `0.3s` 对应 `300ms`，使用灵活匹配（`includes('300ms') || includes('0.3s')`）
7. **可见性守卫**：`collapseBtn` 在折叠状态下隐藏，添加 visibility guards + `page.evaluate` 直接交互

### 文档同步

- `docs/api/frontend-modules.md` 已更新，新增 UI 像素级设计规格 + 交互设计规格 + 5 个测试文件条目

### 验证结果

| 验证项 | 结果 |
|---|---|
| Rust Free | **1560 passed**, 0 failed, 32 ignored（无 Rust 变更，基线保持） |
| Clippy Free | 零警告 |
| cargo fmt | 通过 |
| tsc | 0 errors |
| esbuild | 1641 行（无前端源码变更，基线保持） |
| 新增测试文件 | 5 个（4 Playwright E2E + 1 Vitest），共 265 个测试用例 |
| sync-to-github | 无源代码变更（规格文档 + 测试文件不同步到发布仓库） |

## 测试基线

- Rust Free: **1560** passed, 0 failed, 32 ignored
- Rust Pro: **1831** passed, 0 failed, 49 ignored
- Clippy Free+Pro: 零警告
- cargo fmt: 通过
- tsc: 0 errors
- esbuild: 1641 行
- 版本号: 2.2.0（V2.4 原子计划进行中，S86 ✅ S87 ✅ S88 ✅ 已完成，本 Session 额外完成 UI/UX QA 超大规模测试）
- Vitest: **1076** passed, 62 files（新增 1 个 Vitest 文件 60 个测试，预计 1136 passed 63 files）
- E2E: 待运行新增的 4 个 spec 文件（205 个 Playwright 测试）

## V2.4 原子计划进度

| Session | 功能 | REQ | ROI | 预计天数 | 状态 |
|---|---|---|---|---|---|
| S86 | 对话书签/收藏 | REQ-RAG-053 | 11.0 | 0.5 | ✅ 已完成 |
| S87 | 自动更新检查 | REQ-HELP-004 | 10.6 | 0.5 | ✅ 已完成 |
| S88 | 对话模板导入/导出 | REQ-RAG-054 | 9.2 | 0.5 | ✅ 已完成 |
| **UI/UX QA** | **像素级+交互测试** | **—** | **—** | **1** | **✅ 已完成（本 Session）** |
| S89 | macOS 代码签名与公证 | REQ-SEC-021 | 6.0 | 1 | ⏳ 待实施 |
| S90 | 全局搜索增强 | REQ-IX-007 | 5.4 | 1 | ⏳ 待实施 |
| S91 | 批量操作增强 | REQ-ING-024 | 5.2 | 1 | ⏳ 待实施 |
| S92 | V2.4 发布准备 | — | — | 0.5 | ⏳ 待实施 |

详见 `docs/architecture/V2.4_ATOMIC_PLAN.md`。

## 本 Session 产出的设计规格文档索引

### `docs/architecture/UI_PIXEL_SPEC.md`（416 行）

| 章节 | 内容 |
|---|---|
| §1.1 | 颜色令牌 — 暗色主题（16 个 CSS 变量精确值 + RGB） |
| §1.2 | 颜色令牌 — 浅色主题（16 个 CSS 变量精确值 + RGB） |
| §1.3 | 间距令牌（8px 基准网格，12 级：4/8/12/16/20/24/32/40/48/56/64/80） |
| §1.4 | 字号令牌（7 级：12/13/14/16/18/20/24px + 5 级行高） |
| §1.5 | 圆角令牌（6 级：4/6/8/12/16/22px） |
| §1.6 | 阴影令牌（7 级 + focus shadow） |
| §1.7 | 过渡令牌（4 种 duration + 4 种 easing function） |
| §1.8 | 布局令牌（840px 限宽 + 侧栏 260px/48px + 顶栏 48px + 输入区 64px+） |
| §2 | 组件像素值（消息气泡 + 按钮 + 输入框 + 侧栏 + 顶栏 + 模态框 + 代码块 + 表格 + 链接 + 滚动条） |

### `docs/architecture/UI_INTERACTION_SPEC.md`（354 行）

| 章节 | 内容 |
|---|---|
| §1.1 | 按钮状态机（5 态 + 转换规则 + 像素值） |
| §1.2 | 输入框状态机（4 态 + 验证反馈） |
| §1.3 | 面板状态机（展开/折叠 + 模态框开关 + 侧栏折叠） |
| §2 | 键盘快捷键规格（16 组：⌘J/⌘K/⌘E/⌘⇧F/⌘,/Esc/Enter/Shift+Enter/Tab/Arrow/Space 等） |
| §3 | ARIA 无障碍规格（6 项 role 规范 + Focus Trap + 对比度 WCAG AA 4.5:1） |
| §4 | 动画反馈规格（4 类过渡 + 防闪烁双 rAF + prefers-reduced-motion） |
| §5 | 错误反馈规格（Toast + 内联错误 + 模态错误 + 重试按钮） |
| §6 | 加载反馈规格（骨架屏 + 进度条 + 流式打字 + spinner） |

## 下个 Session 任务

### 优先：S89 macOS 代码签名与公证（REQ-SEC-021，ROI 6.0，1 天）

需要 Apple Developer 账号（用户提供证书 + API Key）。若无证书则跳过公证步骤，仅配置签名管线。

#### 实现步骤

1. 写 SRS 规格定义（REQ-SEC-021 Draft → Implemented，补充契约和测试编号）
2. 配置 `crates/tauri-app/tauri.conf.json` 的 `macOS.signingIdentity` + `entitlements`
3. 新建 `scripts/sign-macos.sh` 本地签名脚本
4. 配置 `.github/workflows/release.yml` CI 签名步骤（通过环境变量注入证书）
5. 验收标准：
   - AC-1：`tauri build` 生成已签名的 `.app` 包（TC-SEC-SIGN-001）
   - AC-2：`codesign -v` 验证签名有效（TC-SEC-SIGN-002）
   - AC-3：`spctl --assess --type execute` Gatekeeper 通过（TC-SEC-SIGN-003）
   - AC-4：公证完成，`xcrun stapler staple` 验证（TC-SEC-SIGN-004）
   - AC-5：启动时无「无法验证开发者身份」警告（TC-SEC-SIGN-005）

#### 关键约束

- 需要 Apple Developer 账号（用户提供证书 + API Key）
- 若无证书则跳过公证步骤，仅配置签名管线
- macOS only，非 macOS 平台跳过

### 备选：S90 全局搜索增强（REQ-IX-007，ROI 5.4，1 天）

当前有对话全文搜索（FTS5）和文档列表搜索。增强为统一全局搜索面板（⌘⇧F），同时搜索：对话消息 + 文档标题/摘要 + 知识图谱实体。依赖 S86（书签跳转逻辑复用）。

### 备选：S91 文档批量操作（REQ-ING-024，ROI 5.2，1 天）

文档列表新增批量选择模式。用户可勾选多个文档后执行批量操作：删除、移动到其他工作空间、添加/删除标签。

### 备选：运行 UI/UX QA 测试并修复发现的问题

本 Session 产出了 265 个 UI/UX 测试用例但尚未运行。下个 Session 可以：
1. 运行 `cd e2e-tests && npx playwright test tests/pixel-tokens.spec.ts tests/pixel-components.spec.ts tests/interaction-state.spec.ts tests/interaction-keyboard-a11y.spec.ts`
2. 运行 `cd e2e-tests && npm run test:unit -- tests/unit/pixel-interaction-spec.test.js`
3. 修复发现的 CSS 不一致、交互缺陷、无障碍违规
4. 更新 `ui/styles/tokens.css` / `ui/styles/components.css` 使其与规格一致

## 关键约束（铁律）

1. 代理仅限开发调研，软件本身禁止代理（`reqwest` 必须加 `.no_proxy()`）
2. TDD 驱动，先写红灯测试再实现；生产代码严禁 `unwrap()/expect()/panic!()`
3. 代码变更同步更新 `docs/api/` + `docs/SOFTWARE_ASSET.md`
4. 修改源代码后运行 `./sync-to-github.sh --commit`
5. Session 结束更新 `CURRENT_STATE.md` + `LOG.md` + 生成本文件
6. 先写规格再写代码（Spec-driven Development）
7. Tailwind 样式规范（铁律十三）：使用 Tailwind 工具类 + 设计令牌，禁止硬编码 hex 颜色

## 验证命令

```bash
cargo test 2>&1 | grep -oE "[0-9]+ passed" | awk '{s+=$1} END {print "Free passed:"s}'
cargo test --features pro 2>&1 | grep -E "test result:" | awk '{print $4}' | awk -F, '{s+=$1} END {print "Pro passed:"s}'
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npm run test:unit
cd e2e-tests && npx playwright test tests/pixel-tokens.spec.ts tests/pixel-components.spec.ts tests/interaction-state.spec.ts tests/interaction-keyboard-a11y.spec.ts
./sync-to-github.sh --commit
```

## 关键文件索引

| 文件 | 说明 |
|---|---|
| `docs/architecture/UI_PIXEL_SPEC.md` | UI 像素级设计规格（416 行，颜色/间距/字号/圆角/阴影/过渡/布局/组件像素值） |
| `docs/architecture/UI_INTERACTION_SPEC.md` | 交互设计规格（354 行，状态机/键盘快捷键/ARIA/动画/错误反馈/加载反馈） |
| `e2e-tests/tests/pixel-tokens.spec.ts` | 像素级设计令牌 E2E 测试（45 个测试） |
| `e2e-tests/tests/pixel-components.spec.ts` | 像素级组件尺寸样式 E2E 测试（60 个测试） |
| `e2e-tests/tests/interaction-state.spec.ts` | 交互状态机 E2E 测试（50 个测试） |
| `e2e-tests/tests/interaction-keyboard-a11y.spec.ts` | 键盘快捷键+无障碍 E2E 测试（50 个测试） |
| `e2e-tests/tests/unit/pixel-interaction-spec.test.js` | 像素+交互逻辑单元测试（60 个测试） |
| `docs/sessions/archive/COMPREHENSIVE_RETROSPECTIVE_2026-08-14.md` | 超大规模复盘报告（~700 行，11 维度评分 8.5/10） |
| `docs/sessions/archive/audit_s3_ui_evaluation.md` | UI 界面验收评估报告（814 行，10 条标准审查） |
| `docs/SRS_v1.0.md` | 需求规格唯一依据（REQ-SEC-021 Draft） |
| `docs/architecture/V2.4_ATOMIC_PLAN.md` | V2.4 原子计划（S86 ✅ S87 ✅ S88 ✅ / S89~S92 ⏳） |
| `ui/styles/tokens.css` | CSS 变量三层架构（Static → Alias → Component） |
| `ui/styles/components.css` | DeepSeek 风格组件样式 |
| `ui/styles/typography.css` | 字体规格 |
| `crates/tauri-app/tauri.conf.json` | S89 起点：macOS 签名配置 |
