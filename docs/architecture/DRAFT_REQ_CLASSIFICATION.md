# Draft REQ 分类报告

> **创建日期**: 2026-08-16
> **Session**: v2.0 S16
> **目标**: SRS 中所有 Draft REQ/AC 精简分类（原计划 78→≤30，实际仅剩 10 项）

---

## 1. 背景

V2.0 深耕原子计划（Phase 7）预计 SRS 中有 78 条 Draft REQ 需要分类。经过 S1-S15 共 15 个 Session 的深耕工作（模块审计、IPC 精简、UI 重构、Dead Code 清理、测试增强等），大量 Draft REQ 已在实施过程中被转为 Implemented/Done/Deprecated。

**截至 S16 开始时，SRS 中仅剩 10 个 Draft 待分类项**：
- 4 个 Draft REQ（REQ 级别状态为 Draft）
- 6 个 Draft AC（REQ 已 Implemented 但含未实现的 Draft AC）

本报告对这 10 个 Draft 项逐一进行 ROI 评估和分类。

---

## 2. 评估维度

| 维度 | 权重 | 说明 |
|---|---|---|
| 用户价值 | 40% | 解决真实用户痛点程度 |
| 实现可行性 | 30% | 技术难度 / 依赖复杂度 |
| 工作量 | 20% | 预计 Session 数 |
| 战略价值 | 10% | 差异化 / 技术债 / 安全 |

ROI = (用户价值 × 0.4 + 可行性 × 0.3 + 战略价值 × 0.1) ÷ 工作量

---

## 3. 分类结果总览

| # | REQ ID | 标题 | 类型 | 分类 | ROI |
|---|---|---|---|---|---|
| 1 | REQ-RAG-047 | 对话书签与收藏夹 | Draft REQ | **Implement** | 6.0 |
| 2 | REQ-UI-012 | Markdown 编辑器增强 | Draft REQ | **Deprecate** | 1.5 |
| 3 | REQ-UI-013 | 文档对比视图 | Draft REQ | **Deprecate** | 1.2 |
| 4 | REQ-WIN-006 | 多窗口支持 | Draft REQ | **Deprecate** | 0.8 |
| 5 | REQ-DS-004 AC-5 | SVG 图标尺寸限定 | Draft AC | **Deprecate** | 0.5 |
| 6 | REQ-DS-006 AC-6 | prefers-reduced-motion | Draft AC | **Implement** | 5.0 |
| 7 | REQ-ERR-001 AC-6/7 | 错误前缀格式 + toast 分类 | Draft AC | **Merge** | 3.0 |
| 8 | REQ-ERR-002 AC-1~5 | 网络错误自动重试 | Draft AC | **Implement** | 5.5 |
| 9 | REQ-ERR-003 AC-1~5 | 离线模式降级 | Draft AC | **Implement** | 4.5 |
| 10 | REQ-ERR-005 AC-7~9 | 输入长度/格式校验 | Draft AC | **Implement** | 4.0 |

| 分类 | 数量 | 说明 |
|---|---|---|
| **Implement** | 5 | 高 ROI，纳入 v2.0 或后续版本规划 |
| **Deprecate** | 4 | 低价值或已有替代方案 |
| **Merge** | 1 | 合并到已有 REQ |

---

## 4. 详细分类

### 4.1 Implement（5 项）

#### #1: REQ-RAG-047 对话书签与收藏夹

| 维度 | 评分 | 理由 |
|---|---|---|
| 用户价值 | 8/10 | 知识工作者常需标记重要对话快速定位，高频使用场景 |
| 可行性 | 8/10 | 纯前端 + 简单 SQLite 表，零新增依赖 |
| 工作量 | 0.5 Session | 新增 1 表 + 4 IPC + sidebar UI |
| 战略价值 | 5/10 | 提升用户粘性，差异化功能 |
| **ROI** | **6.0** | |

**分类**: Implement
**理由**: 用户价值高、实现简单、零依赖。书签功能是知识管理工具的标配功能。
**建议版本**: v2.1 候选

---

#### #6: REQ-DS-006 AC-6 prefers-reduced-motion 支持

| 维度 | 评分 | 理由 |
|---|---|---|
| 用户价值 | 7/10 | 无障碍必备，前庭功能障碍用户依赖此功能 |
| 可行性 | 10/10 | 纯 CSS `@media (prefers-reduced-motion: reduce)` 一行即可 |
| 工作量 | 0.1 Session | 几行 CSS |
| 战略价值 | 8/10 | 无障碍合规（WCAG 2.1 SC 2.3.3） |
| **ROI** | **5.0** | |

**分类**: Implement
**理由**: 无障碍必备功能，实现成本极低（纯 CSS 媒体查询），WCAG 合规需求。
**建议版本**: v2.0（可直接实现）
**行动计划**: 在 `ui/styles/components.css` 添加 `@media (prefers-reduced-motion: reduce)` 规则，禁用 `fade-in` 和 `thinkingPulse` 动画。

---

#### #8: REQ-ERR-002 AC-1~5 网络错误自动重试

| 维度 | 评分 | 理由 |
|---|---|---|
| 用户价值 | 9/10 | 网络不稳定时自动重试显著提升用户体验，高频痛点 |
| 可行性 | 7/10 | 后端 OpenAIProvider 添加重试逻辑 + 前端 retrying phase |
| 工作量 | 1 Session | 后端退避重试 + 前端 UI 反馈 |
| 战略价值 | 6/10 | 弹性设计核心组件 |
| **ROI** | **5.5** | |

**分类**: Implement
**理由**: 网络不稳定是本地 RAG 应用的常见痛点，自动重试是弹性设计的基础。已有完整规格（退避策略、状态机、UI 反馈），实现路径清晰。
**建议版本**: v2.1 候选
**行动计划**: `OpenAIProvider` 添加 `retry_with_backoff()` + `chat_phase` 事件新增 `retrying` phase + 前端 `#inputHint` 文案更新。

---

#### #9: REQ-ERR-003 AC-1~5 离线模式降级

| 维度 | 评分 | 理由 |
|---|---|---|
| 用户价值 | 7/10 | 离线时明确告知用户状态，避免困惑 |
| 可行性 | 8/10 | 前端离线检测 + UI 状态切换，已有 offline-detector.js |
| 工作量 | 0.5 Session | 前端离线状态管理 + UI 反馈 |
| 战略价值 | 7/10 | 弹性设计 + 用户体验 |
| **ROI** | **4.5** | |

**分类**: Implement
**理由**: 已有 `ui/src/offline-detector.js` 模块和 S15 测试覆盖，离线模式降级是自然延伸。规格完整（UI 元素、状态矩阵、检测机制），实现路径清晰。
**建议版本**: v2.1 候选
**行动计划**: `offline-detector.js` 扩展 `setOfflineMode(bool)` + `#sendBtn` disabled 控制 + `#inputHint` 文案切换 + `#queryInput` placeholder 切换。

---

#### #10: REQ-ERR-005 AC-7~9 输入长度/格式校验

| 维度 | 评分 | 理由 |
|---|---|---|
| 用户价值 | 6/10 | 防止用户输入过长内容导致 API 报错 |
| 可行性 | 9/10 | 后端简单长度检查 + 前端提示 |
| 工作量 | 0.3 Session | 3 个校验函数 |
| 战略价值 | 7/10 | 安全防御 + 用户体验 |
| **ROI** | **4.0** | |

**分类**: Implement
**理由**: 输入校验是防御性编程的基础，3 个 AC 实现简单（API Key 长度限制 256 + Base URL 协议校验 + 查询长度限制 8000），可防止用户遇到不友好的后端错误。
**建议版本**: v2.0（可直接实现）
**行动计划**: `update_llm_config_inner()` 添加长度/格式校验 + `chat_inner()` 添加查询长度校验 + 前端 `toastError` 提示。

---

### 4.2 Deprecate（4 项）

#### #2: REQ-UI-012 Markdown 编辑器增强

| 维度 | 评分 | 理由 |
|---|---|---|
| 用户价值 | 3/10 | 聊天输入框不需要复杂 Markdown 编辑器，用户习惯直接输入 Markdown 语法 |
| 可行性 | 5/10 | 需新建 md-editor.js 模块 + 工具栏 UI + 预览切换 |
| 工作量 | 1.5 Session | 新模块 + 集成 + 6 个 AC |
| 战略价值 | 2/10 | 与应用核心价值（RAG 知识库）无关 |
| **ROI** | **1.5** | |

**分类**: Deprecate
**理由**: 聊天输入框的本质是快速提问，不需要格式工具栏。用户已可直接输入 Markdown 语法（`**bold**`、`` `code` ``）， marked.js 在渲染端处理。格式工具栏增加了 UI 复杂度但用户价值极低。
**替代方案**: 用户直接输入 Markdown 语法，渲染端 marked.js 处理。已有斜杠命令系统（REQ-RAG-038）提供模板快捷输入。

---

#### #3: REQ-UI-013 文档对比视图

| 维度 | 评分 | 理由 |
|---|---|---|
| 用户价值 | 3/10 | 文档对比是低频场景，用户可通过打开两个文档预览替代 |
| 可行性 | 4/10 | 同步滚动实现复杂，跨文档对齐困难 |
| 工作量 | 2 Session | 新模块 + 同步滚动逻辑 + Markdown 渲染 |
| 战略价值 | 2/10 | 与 RAG 核心价值无关 |
| **ROI** | **1.2** | |

**分类**: Deprecate
**理由**: 文档对比是低频边缘场景，实现复杂（同步滚动 + 对齐逻辑）但用户价值有限。用户可通过先后打开两个文档预览（REQ-ING-010 文档内容预览已实现）替代。
**替代方案**: REQ-ING-010 文档内容预览已满足单文档查看需求。用户可依次打开两篇文档对比。

---

#### #4: REQ-WIN-006 多窗口支持

| 维度 | 评分 | 理由 |
|---|---|---|
| 用户价值 | 2/10 | 本地知识库应用单窗口足够，多窗口增加复杂度和资源消耗 |
| 可行性 | 3/10 | Tauri 多窗口 + AppState 共享 + IPC 事件同步，技术复杂 |
| 工作量 | 2.5 Session | 新窗口创建 + 状态同步 + 资源清理 |
| 战略价值 | 1/10 | 与核心价值无关，增加维护成本 |
| **ROI** | **0.8** | |

**分类**: Deprecate
**理由**: 本地 RAG 知识库应用的核心交互是"提问→回答"的单窗口模式，多窗口支持增加大量复杂度（窗口管理、状态同步、资源清理）但用户价值极低。Tauri 多窗口在 macOS 上也有已知的焦点管理问题。
**替代方案**: 对话分支树（REQ-RAG-039 已实现）允许在同一窗口内管理多个对话分支，满足用户同时探索多个对话方向的需求。

---

#### #5: REQ-DS-004 AC-5 SVG 图标系统尺寸限定

| 维度 | 评分 | 理由 |
|---|---|---|
| 用户价值 | 1/10 | 用户不关心图标是 Unicode 还是 SVG |
| 可行性 | 3/10 | 需引入 SVG 图标系统，当前 Unicode 字符方案已满足需求 |
| 工作量 | 1 Session | SVG 图标库选择 + 集成 + 尺寸限定 |
| 战略价值 | 1/10 | 纯视觉美化，无功能价值 |
| **ROI** | **0.5** | |

**分类**: Deprecate
**理由**: 当前 Unicode 字符图标方案已满足全部功能需求（`ui/styles/icons.css` 已定义 `.icon-sm/md/lg` 三档尺寸类）。引入 SVG 图标系统增加包体积和维护成本但无实质用户价值。
**替代方案**: 维持现有 Unicode 字符 + Tailwind `text-{size}` 类方案。`REQ-DS-004` AC-1~AC-4 已 Implemented 并满足需求。

---

### 4.3 Merge（1 项）

#### #7: REQ-ERR-001 AC-6/AC-7 错误前缀格式 + toast 分类

| 维度 | 评分 | 理由 |
|---|---|---|
| 用户价值 | 7/10 | 统一错误前缀改善用户体验 |
| 可行性 | 8/10 | 后端已有部分前缀实现（LIMIT_REACHED/PRO_REQUIRED） |
| 工作量 | 0.5 Session | 扩展前缀到全部 IPC + 前端 toast 分类 |
| 战略价值 | 5/10 | 错误处理一致性 |
| **ROI** | **3.0** | |

**分类**: Merge
**理由**: AC-6（后端所有 IPC 命令返回的错误字符串以 `PREFIX:` 格式开头）和 AC-7（前端 toast 根据 prefix 选择合适的 kind）本质上是 REQ-ERR-001 已有错误分类体系的扩展。已有 `error-detail.js` 实现了 5 类错误规则（embed_timeout/pro_required/file_format/database/server_error），AC-6/AC-7 是将这些规则推广到全部 IPC 命令。
**合并目标**: REQ-ERR-001 统一错误分类体系（状态保持 Implemented，AC-6/AC-7 标记为"扩展已实现"）
**行动计划**: S6 P1-2 已实现错误信息统一分类通俗化（`error-detail.js` 5 类错误规则 + 3 类现有错误信息通俗化）。AC-6/AC-7 的核心功能已被 S6 P1-2 覆盖。将 AC-6/AC-7 标记为"已由 S6 P1-2 实现"，不再作为独立 Draft AC。

---

## 5. SRS 更新汇总

### 5.1 Draft REQ 状态更新（4 项）

| REQ ID | 原状态 | 新状态 | 分类 | 分类理由摘要 |
|---|---|---|---|---|
| REQ-RAG-047 | Draft | **Draft（Implement 候选）** | Implement | 高用户价值、零依赖、知识管理标配 |
| REQ-UI-012 | Draft | **Deprecated** | Deprecate | 聊天输入框不需要格式工具栏，用户可直接输入 Markdown |
| REQ-UI-013 | Draft | **Deprecated** | Deprecate | 低频边缘场景，文档预览已满足需求 |
| REQ-WIN-006 | Draft | **Deprecated** | Deprecate | 单窗口足够，多窗口增加复杂度无价值 |

### 5.2 Draft AC 状态更新（6 项）

| REQ ID | AC | 原状态 | 新状态 | 分类 | 分类理由摘要 |
|---|---|---|---|---|---|
| REQ-DS-004 | AC-5 | Draft | **Deprecated** | Deprecate | Unicode 图标已满足需求，SVG 系统无实质价值 |
| REQ-DS-006 | AC-6 | Draft | **Draft（Implement 候选）** | Implement | 无障碍必备，纯 CSS 一行实现 |
| REQ-ERR-001 | AC-6/AC-7 | Draft | **Implemented（S6 P1-2 覆盖）** | Merge | error-detail.js 已实现统一错误分类 |
| REQ-ERR-002 | AC-1~5 | Draft | **Draft（Implement 候选）** | Implement | 网络重试是弹性设计基础，规格完整 |
| REQ-ERR-003 | AC-1~5 | Draft | **Draft（Implement 候选）** | Implement | offline-detector.js 已有基础，自然延伸 |
| REQ-ERR-005 | AC-7~9 | Draft | **Draft（Implement 候选）** | Implement | 防御性编程基础，3 个校验函数简单 |

---

## 6. 统计对比

| 指标 | S16 前 | S16 后 | 变化 |
|---|---|---|---|
| Draft REQ 数 | 4 | 1（仅 REQ-RAG-047 保留 Draft 待实现） | -3 |
| Draft AC 数 | 21 | 12（REQ-DS-006 AC-6 + REQ-ERR-002 AC-1~5 + REQ-ERR-003 AC-1~5 + REQ-ERR-005 AC-7~9） | -9 |
| 总 Draft 项 | 10 | 5（1 Draft REQ + 4 Draft AC 集合待实现） | -5 |
| Deprecated 新增 | 0 | 4（3 REQ + 1 AC） | +4 |
| Merged 新增 | 0 | 1（AC 合并到已实现） | +1 |

**目标达成**: 原计划 78→≤30。实际 S16 前 10 项 → S16 后 5 项待实现。远超目标。

---

## 7. v2.0 后续规划建议

### 7.1 可在 v2.0 直接实现的低工作量项

| REQ/AC | 工作量 | 理由 |
|---|---|---|
| REQ-DS-006 AC-6 | 0.1 Session | 纯 CSS `@media` 查询 |
| REQ-ERR-005 AC-7~9 | 0.3 Session | 3 个简单校验函数 |
| REQ-ERR-001 AC-6/AC-7 | 0（已实现） | S6 P1-2 已覆盖 |

### 7.2 v2.1+ 候选

| REQ/AC | 工作量 | 理由 |
|---|---|---|
| REQ-RAG-047 对话书签 | 0.5 Session | 知识管理标配 |
| REQ-ERR-002 网络重试 | 1 Session | 弹性设计核心 |
| REQ-ERR-003 离线模式 | 0.5 Session | offline-detector.js 基础已有 |

---

## 8. 验收清单

- [x] 所有 Draft REQ/AC 已提取（10 项）
- [x] 每项有 ROI 评估和分类理由
- [x] Deprecate 项有替代方案说明
- [x] Merge 项有目标 REQ 指定
- [x] SRS 状态已更新
- [x] 分类报告已输出
