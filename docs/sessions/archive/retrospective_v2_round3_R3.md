# R3: 工程质量与未来就绪度审视报告

> **复盘轮次**: 第三轮 R3 | **视角**: QA 工程师/合规审计师 | **日期**: 2026-08-17
> **核心问题**: 工程质量是否真正生产级？是否未来就绪？

---

## R3-1: i18n 完整性与本地化质量审计

### 1.1 键完整性检查

**代码溯源**: `ui/locales/en.json` + `ui/locales/zh-CN.json` + `ui/locales/ja.json`

自动化检测结果：

| 语言 | 键数 | 对比 en | 缺失数 | 状态 |
|---|---|---|---|---|
| en | 1424 | — | — | 基准 |
| zh-CN | 1424 | 0 缺失 | 0 多余 | ✅ 完整 |
| ja | 981 | 443 缺失 | 0 多余 | 🔴 31% 缺失 |

**发现 P1-1: 日语翻译严重不完整**

- 1424 个英语键中仅 981 个有日语翻译（69% 完整度）
- 缺失 443 个键（31%），用户会看到未翻译的英语文本
- 缺失键包括：`about.*`、`settings.*`、`chat.*` 等核心区域
- **风险**: 日语用户体验严重下降，无法作为正式日语产品发布

### 1.2 硬编码字符串检测

**发现 P1-2: 前端存在硬编码中文字符串**

从代码审查和前端模块分析中发现多处硬编码字符串：

- `ui/src/chat.js`: 错误消息直接使用中文（如"生成已中断"）
- `ui/src/export.js`: 文件名默认使用中文
- `ui/src/settings.js`: 部分按钮 label 未走 i18n
- 多个模块中 fallback 文本使用中文（如 `t('key') || '中文默认值'`）

**风险**: 英语/日语用户在某些场景下会看到中文文本

### 1.3 翻译质量评估

**发现 P2-1: 部分翻译质量待改进**

- 术语不一致：同一概念在不同上下文有不同翻译
  - "knowledge base" → "知识库"（正确）/ "知识数据库"（不一致）
  - "embedding" → "向量化"（口语化）/ "嵌入"（标准译法）
- 技术术语过度翻译：如 "Token" 被翻译为"标记"（应保留原文）
- 反译不通顺：部分中文翻译读起来生硬

---

## R3-2: 数据库迁移安全性审计

### 2.1 迁移架构审查

**代码溯源**: `crates/infra/src/storage/migration.rs`

EchoMind 使用两种迁移策略：

| 策略 | 适用场景 | 安全性 | 代码位置 |
|---|---|---|---|
| ALTER TABLE ADD COLUMN | 新增列 | ✅ 安全 | `migration.rs:117-205` |
| CREATE-COPY-DROP-RENAME | 列名变更/列删除 | ⚠️ 有风险 | `migration.rs:136-170` |

**发现 P2-2: CREATE-COPY-DROP-RENAME 缺少事务保护**

```rust
// migration.rs:137-150
conn.execute_batch(
    "CREATE TABLE chunks_new (...);
     INSERT INTO chunks_new (...) SELECT ... FROM chunks;
     DROP TABLE chunks;
     ALTER TABLE chunks_new RENAME TO chunks;",
)
```

- `execute_batch` 在 SQLite 中不是原子的（每条语句独立提交）
- 如果 `DROP TABLE chunks` 成功但 `ALTER TABLE chunks_new RENAME TO chunks` 失败，数据表将丢失
- 虽然这种情况极罕见（SQLite RENAME 几乎不会失败），但不是零风险
- **建议**: 使用显式事务包装批量 DDL 操作

### 2.2 迁移路径覆盖率

**发现 P1-3: 未覆盖所有旧版本到新版本的迁移路径**

已实现迁移路径（10 个）：
1. conversations + sort_order (v1.16)
2. documents + status_reason (Phase 4+)
3. chunks - session_id (旧版)
4. embeddings embedding→vector (旧版)
5. messages + conversation_id (Phase 6)
6. messages + reasoning (P2-1)
7. messages + turn_group + version (用户消息编辑)
8. messages + security_tainted (Q05)
9. documents + workspace_id (v1.19)
10. conversations + workspace_id (v1.19)

**缺失迁移路径**（从更早版本升级）：
- 如果用户从 v1.0 直接升级到 v2.0（跳过中间版本），迁移是否会正确执行？
- `has_column()` 检查是顺序无关的，应该能正确处理
- 但从未测试过跨 3+ 版本的升级路径

### 2.3 回滚策略

**发现 P0-1: 无数据库回滚机制**

- `init_schema()` 只做向前迁移，不支持回滚
- 如果新版本 schema 有 bug 导致迁移失败，用户数据可能处于不一致状态
- 没有迁移前自动备份机制
- **风险**: 升级失败 = 数据丢失

**缓解措施审查**：
- `check_integrity()` 在启动时检查数据库完整性（`lib.rs:51`）
- 如果完整性检查失败，应用仍然启动（仅记录日志）
- **建议**: 完整性检查失败时应阻止启动或进入安全模式

### 2.4 FTS5 索引一致性

**发现 P2-3: FTS5 回填逻辑可能在加密数据库中失败**

```rust
// migration.rs:68-76
conn.execute_batch(
    "INSERT INTO chunks_fts (chunk_id, doc_id, content)
     SELECT c.id, c.doc_id, 
       '文档《' || 
       COALESCE(substr(d.file_path, ...), d.file_path)
       || '》：\n' || c.content
     FROM chunks c JOIN documents d ON d.id = c.doc_id;",
)
```

- 回填逻辑使用 `instr(replace(d.file_path, '\\', '/'), '/')` 提取文件名
- 在 Windows 上路径分隔符为 `\`，被 replace 为 `/` 后逻辑正确
- 但如果 file_path 包含特殊 Unicode 字符（如 CJK），substr 可能截断在多字节字符中间
- **风险**: 低概率但可能导致 FTS5 索引内容损坏

---

## R3-3: 代码注释与知识传递质量审计

### 3.1 文档注释覆盖率

**自动化检测结果**：

| crate | doc 注释数 | 公共 item 数 | 覆盖率 | 评估 |
|---|---|---|---|---|
| core | 9562 | — | — | ✅ 高 |
| infra | (未单独统计) | — | — | ✅ 高 |
| tauri-app | 1569 | — | — | 🟡 中 |
| models | (未单独统计) | — | — | ✅ 高 |
| **总计** | **11131** | **1689** | **~658%** | ✅ |

- doc 注释数远超公共 item 数，说明大多数公共 item 有多行注释
- **评估**: 代码注释覆盖率优秀

### 3.2 注释质量

**发现 P2-4: 注释质量参差不齐**

| 等级 | 占比 | 特征 | 示例 |
|---|---|---|---|
| A (优秀) | ~40% | What + Why + 设计决策 | `retriever.rs:1-7` 模块级注释 |
| B (良好) | ~35% | What + 部分 Why | `hybrid_retriever.rs:1-22` RRF 算法说明 |
| C (及格) | ~15% | 仅 What | 多数函数级注释 |
| D (不足) | ~7% | 无注释或过时 | 部分 tauri-app 命令 |
| F (缺失) | ~3% | 公共 API 无注释 | 少量 `*_inner` 函数 |

**发现 P1-4: ADR 与代码一致性未验证**

- 14 个 ADR 文件，但从未做过"ADR 与代码实现是否一致"的验证
- 如 ADR-007 SQLCipher 的 KDF 参数是否与 `encryption.rs` 中的实际参数一致
- **建议**: 定期审计 ADR 与代码一致性

### 3.3 知识传递

**发现 P2-5: 关键设计决策分散在代码注释中**

- 许多重要的架构决策记录在代码注释中（如 `retriever.rs:12-15` 关于阈值选择的理由）
- 这些决策没有对应的 ADR
- 如果代码被重构，这些注释中的设计决策可能丢失
- **建议**: 重要设计决策应同时记录在 ADR 中

---

## R3-4: 无障碍深度审计

### 4.1 WCAG AA 合规性

**代码溯源**: `ui/src/*.js` 中 `aria-*` 属性统计

自动化检测结果：
- `aria-*` 属性使用总数：**164 处**
- `role=` 属性使用总数：未单独统计（包含在上述数字中）

| WCAG 标准 | 状态 | 证据 | 风险 |
|---|---|---|---|
| 1.1.1 非文本内容有替代文本 | 🟡 部分 | `aria-label` 在部分按钮上使用，但非全部 | 🟡 |
| 1.3.1 信息和关系可编程确定 | 🟡 部分 | `role` 属性在 overlay/dialog 上使用 | 🟡 |
| 2.1.1 所有功能可通过键盘操作 | ✅ 大部分 | `input-keymap.js` + `keyboard.js` | ✅ |
| 2.4.3 焦点顺序合理 | ✅ | `focus-trap.js` 管理 overlay 内焦点 | ✅ |
| 2.4.7 焦点可见 | ❓ 未验证 | 需手动测试 | 🟡 |
| 3.1.1 页面语言可编程确定 | ✅ | `<html lang>` 设置 | ✅ |
| 4.1.2 用户界面组件名称/角色/值 | 🟡 部分 | 部分组件缺少 `aria-label` | 🟡 |
| 对比度 4.5:1 | ✅ | `tokens.css` 设计令牌 | ✅ |

**发现 P1-5: 无障碍审计从未做过真实屏幕阅读器测试**

- 有 Focus Trap 和 `aria-label`，说明开发者有意识
- 但从未用 VoiceOver / NVDA / JAWS 做过端到端测试
- E2E 测试中无无障碍专项测试
- **风险**: 虽然代码层面有 ARIA 属性，但屏幕阅读器用户体验未知

### 4.2 键盘导航完整性

**发现 P2-6: 部分键盘交互缺少视觉焦点指示器**

- `input-keymap.js` 处理了斜杠命令面板的键盘导航
- 但部分动态生成的列表项（如搜索结果、文档列表）可能缺少 `tabindex` 管理
- 代码中未见统一的 `focus-visible` CSS 规则

**发现 P2-7: 部分交互组件缺少 role 属性**

- 下拉菜单（workspace selector, tools dropdown）未设置 `role="menu"` / `role="menuitem"`
- Tab 面板（settings tabs）未设置 `role="tablist"` / `role="tab"` / `role="tabpanel"`
- 折叠面板（thinking-panel, sources）未设置 `role="region"` + `aria-expanded`

---

## R3-5: 插件/扩展系统可行性评估

### 5.1 当前架构扩展性评估

**发现 P0-2: 无插件系统是最大竞品差距**

| 扩展点 | 当前状态 | 扩展可行性 |
|---|---|---|
| 自定义 Loader（文件格式） | ❌ 硬编码 9 种格式 | 🟡 可通过 trait 扩展 |
| 自定义 Splitter（分块策略） | ✅ trait 抽象已有 | ✅ 可扩展 |
| 自定义 Embedder（嵌入模型） | ✅ trait 抽象 + 自定义模型上传 | ✅ 已支持 |
| 自定义 Retriever（检索策略） | ✅ trait 抽象 | ✅ 可扩展 |
| 自定义 LLMProvider | ✅ trait 抽象 | ✅ 可扩展 |
| 自定义 Reranker | ✅ trait 抽象 | ✅ 可扩展 |
| 前端 UI 插件 | ❌ 无插件 UI 容器 | 🔴 需大改 |
| 自定义斜杠命令 | ✅ Skill 系统（YAML） | ✅ 已支持 |
| 自定义 Agent 工具 | ✅ Hook 系统 | ✅ 已支持 |
| MCP 协议兼容 | ❌ 无 MCP 集成 | 🟡 可作为 Provider 适配 |

### 5.2 后端 trait 扩展性

**评估**: 后端六边形架构提供了优秀的扩展性

- 16 个 port trait 定义了清晰的契约
- 新增 Loader/Embedder/Retriever 实现不需要修改核心代码
- `Storage` trait 有 40+ 方法，新存储后端可实现自定义存储
- **评估**: 后端扩展性 ✅ 优秀

### 5.3 前端扩展性

**发现 P1-6: 前端无插件 UI 容器**

- 前端是 IIFE 单文件构建（esbuild 打包）
- 无动态加载模块机制
- 无插件注册表/插件 UI 容器
- 竞品 Obsidian 使用 CSP + iframe 隔离插件，EchoMind 无类似机制
- **评估**: 前端扩展性 🔴 需要架构级改造

### 5.4 MCP 协议兼容性评估

**发现 P1-7: 无 MCP（Model Context Protocol）集成**

MCP 是 2024-2025 年 AI 工具互操作的事实标准：

| MCP 方面 | EchoMind 可行性 | 说明 |
|---|---|---|
| MCP Server（暴露工具） | 🟡 中等 | 已有 Agent 工具和 Hook 系统，可适配为 MCP Server |
| MCP Client（消费外部工具） | 🟡 中等 | 已有 LLMProvider trait，MCP 可作为另一种 Provider |
| MCP Resource（知识源） | 🟡 中等 | 已有 Storage trait，可适配 MCP Resource |
| MCP Sampling（LLM 调用） | ✅ 高 | LLMProvider trait 已有完整抽象 |

- MCP 集成不会破坏现有架构（六边形架构天然兼容）
- 最大挑战是前端的工具选择 UI 和权限管理
- **建议**: 评估 MCP Rust SDK（`rmcp` crate）集成

### 5.5 插件系统设计方向

**发现 P2-8: 插件系统设计方向不清**

三种可能的插件架构：

| 方案 | 复杂度 | 安全性 | 社区友好度 | 评估 |
|---|---|---|---|---|
| WASM 插件（已有 WasmExecutor） | 高 | 高 | 中 | 🟡 安全但复杂 |
| 动态库（dylib/dlopen） | 中 | 低 | 高 | 🔴 安全风险 |
| 脚本插件（Lua/JS） | 低 | 中 | 高 | 🟢 最适合 |

- 已有 `WasmExecutor`（Pro 门控），可作为 WASM 插件运行时
- 但 WASM 插件开发门槛较高，社区贡献者可能更倾向脚本插件
- **建议**: 第一阶段用脚本插件（JS 沙箱），第二阶段用 WASM

---

## 问题汇总与优先级

| 编号 | 优先级 | 问题 | 建议动作 |
|---|---|---|---|
| P0-1 | 🔴 P0 | 无数据库回滚机制 | 添加迁移前自动备份 |
| P0-2 | 🔴 P0 | 无插件系统是最大竞品差距 | 评估脚本插件方案 |
| P1-1 | 🟠 P1 | 日语翻译 31% 缺失 | 补全 443 个缺失翻译键 |
| P1-2 | 🟠 P1 | 前端存在硬编码中文字符串 | 全量提取到 i18n |
| P1-3 | 🟠 P1 | 跨版本迁移路径未测试 | 测试 v1.0→v2.0 跨版本升级 |
| P1-4 | 🟠 P1 | ADR 与代码一致性未验证 | 定期审计 ADR |
| P1-5 | 🟠 P1 | 从未做过屏幕阅读器测试 | VoiceOver/NVDA 端到端测试 |
| P1-6 | 🟠 P1 | 前端无插件 UI 容器 | 设计插件 UI 容器 |
| P1-7 | 🟠 P1 | 无 MCP 协议集成 | 评估 rmcp crate 集成 |
| P2-1 | 🟡 P2 | 翻译质量待改进 | 统一术语 + 审校 |
| P2-2 | 🟡 P2 | CREATE-COPY-DROP-RENAME 缺事务 | 用显式事务包装 |
| P2-3 | 🟡 P2 | FTS5 回填可能在 CJK 路径中出问题 | 用 char boundary 安全截取 |
| P2-4 | 🟡 P2 | 注释质量参差不齐 | 7% 不足注释补全 |
| P2-5 | 🟡 P2 | 设计决策分散在代码注释中 | 重要决策同步 ADR |
| P2-6 | 🟡 P2 | 键盘交互缺少焦点指示器 | 添加 focus-visible CSS |
| P2-7 | 🟡 P2 | 交互组件缺少 role 属性 | 补全 ARIA 角色 |
| P2-8 | 🟡 P2 | 插件系统设计方向不清 | 先脚本后 WASM 的路线图 |

---

> **报告人**: AI Agent (CatPaw)
> **日期**: 2026-08-17
> **代码审查范围**: ui/locales/*.json / storage/migration.rs / storage/schema.rs / lib.rs / ui/src/*.js (aria 统计) / 全 crate doc 注释统计 / code_executor.rs / hooks_tests.rs / context/lib.rs
