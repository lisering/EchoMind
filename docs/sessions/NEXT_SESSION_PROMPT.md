# EchoMind 下个 Session 衔接提示词

> **复制下方全部内容作为新会话的第一条消息即可获取完整上下文。**

---

## 使命

EchoMind（灵犀）是一个 Rust + Tauri v2 桌面应用：本地 RAG 知识库。使用 fastembed ONNX 嵌入（all-MiniLM-L6-v2, 384-dim）、SQLite 向量存储、任意 OpenAI 兼容 LLM。功能包括 SQLCipher AES-256 加密、PII 检测/脱敏、审计哈希链日志、自动锁屏，面向律师、研究人员等高隐私场景。

## 架构

六边形（端口与适配器）架构，七个 crate：

```
crates/models  →  crates/prompt  →  crates/core  →  crates/infra  →  crates/tauri-app
  (contracts)     (prompts)       (ports+logic)  (adapters)     (assembly)
                                     ↑
                              crates/compact
                              crates/context
```

依赖方向严格内向。`crates/core` 禁止依赖 Tauri/reqwest/rusqlite。Edition 2024（≥Rust 1.85），原生 `async fn` in trait，无 `#[async_trait]` 宏。

前端：`ui/src/*.js` 模块化 ES modules（56 文件，vanilla JS），通过 `node scripts/build-ui.mjs`（esbuild）打包为单文件 `ui/index.html`。TypeScript checkJs 类型检查（`npx tsc --noEmit`）。Tailwind/marked.js/DOMPurify/highlight.js 本地 vendored，无 CDN。

## 当前测试基线（2026-08-10）

| 项目 | 数值 |
|---|---|
| Rust Free 全量 | **1224 passed**, 0 failed, 28 ignored |
| Clippy Free | 零警告 |
| cargo fmt | 通过 |
| TypeScript | 0 errors |
| esbuild 构建 | 13890 行 |
| 版本号 | 1.4.0（全部 7 crate + tauri.conf.json）|
| IPC 命令数 | 194 Free / 206 Pro（含 12 个 Pro 门控）|
| 前端模块数 | 56（`ui/src/*.js`）|

## 上个 Session 完成内容

### REQ-RAG-046 Session Strip（会话条带化，DS4 DS-03 借鉴）✅

借鉴 ds4 (DwarfStar) `/strip` 命令，实现会话消息条带化操作。允许用户从对话历史中移除指定范围的消息，可选地用摘要 system 消息替代，以减少上下文窗口消耗。零新增依赖。

**新增文件**：
- `crates/models/src/lib.rs` — `StripConfig`（from_index/to_index/replace_with_summary/summary_text）+ `StripResult`（stripped_count/summary_inserted/stripped_message_ids/estimated_tokens_saved）+ `StripPreview`（messages/total_messages/estimated_tokens_saved）数据模型
- `crates/core/src/session_strip.rs` — `SessionStripper` 结构体 + `strip_range()`（按索引范围删除消息）+ `strip_keeping_recent()`（保留最后 N 条）+ `preview()`（预览不执行）+ `estimate_tokens()`（4 字符≈1 token）+ `validate_range()` 范围校验
- `crates/core/src/session_strip_tests.rs` — 13 TDD 测试 TC-STRIP-001~013
- `ui/src/session-strip.js` — 前端 strip 面板（模态框 + 范围/保留模式 + 预览 + 摘要选项 + 确认对话框）

**关键设计**：
- 借鉴 ds4 `/strip <sha>` 命令：移除数据以减少上下文消耗
- EchoMind 适配：移除消息历史条目（非 KV cache payload）
- 支持三种操作：按范围 strip、保留最近 N 条、预览
- 可选摘要替代：插入 system 消息作为被移除内容的摘要
- Storage trait 新增 `delete_messages_by_ids()` 方法（默认空操作，SqliteStorage 实现 DELETE IN (?)）
- 3 IPC 命令（strip_messages / strip_keeping_recent / preview_strip）

**验证结果**：core 744 passed | models 40 passed | clippy 零警告 | fmt 通过 | tsc 0 errors | esbuild 13890 行 | sync-to-github 11 files 1185 insertions

## 已完成版本

| 版本 | 状态 | 功能 |
|---|---|---|
| v1.0.0 | ✅ 发布 | 核心功能（导入/检索/对话/安全/加密/本地LLM/知识图谱/极致性能优化10项）|
| v1.1.0 | ✅ 发布 | .docx 导入 / .html 导入 / 浅色主题 / 语音输入+TTS / 网页搜索 |
| v1.2.0 | ✅ 发布 | PDF 导出 / .pptx 导入 / .epub 导入 / 自定义 ONNX 嵌入模型上传 |
| v1.3.0 | ✅ 发布 | 知识图谱导出 / 文档摘要 / 对话模板 / Markdown 双向链接 / 对话分支树 |
| v1.4.0 | ✅ 发布 | .xlsx/.csv 导入 / 对话全文搜索 / Contextual Retrieval / 文档标签 / commands.rs 模块化 |

## 已完成借鉴计划

| 借鉴来源 | 状态 | 内容 |
|---|---|---|
| QM (Quotient) | ✅ 全部完成 (Q01-Q11) | 记忆系统 / 上下文压缩 / 安全层 / 基础设施 / LLMProvider |
| OpenCode | ✅ 全部完成 (B01-B14) | System Context / Epoch / 压缩模板 / Prompt Caching / Admission / Coordinator / Tool Bounding / Skill / 多协议 / Permission |
| 跨 Phase 整合 | ✅ 全部完成 | Burst Buffer / Sweeper / Shadow 筛查 |
| OpenMontage | ✅ 全部完成 | MMR 多样性重排 / 检索质量门控 / Scored Selector / Pipeline Checkpoint / Event Stream |
| DS4 (DwarfStar) | ✅ DS-01 + DS-02 + DS-03 完成 | Prompt Prefix 磁盘缓存 / 上下文压缩原文尾部保留 / Session Strip 会话条带化 |
| RAGAS | ✅ REQ-RAG-045 完成 | RAG 评估指标系统（9 种指标）|

## 待定方向（下个 Session 可选）

### 方向 1: DS4 剩余借鉴（DS-04~DS-07）
- DS-04: 功率管理（CPU/GPU 功率感知推理调度，ROI 6.0）
- DS-05: Tool Call 续接（中断后自动恢复工具调用链，ROI 4.0）
- DS-06: Radix Tree（前缀树加速 Prompt Cache 查找，ROI 低）
- DS-07: SSD 流式（大模型分片从 SSD 流式加载，ROI 低）

### 方向 2: v1.5 新功能规划
候选功能（需 ROI 评估）：
- RAG 评估指标 E2E 测试（为 REQ-RAG-045 添加 Playwright E2E 测试）
- Session Strip E2E 测试（为 REQ-RAG-046 添加 Playwright E2E 测试）
- 对话导出 PDF（复用 window.print + Markdown 渲染管线）
- 批量文档导入性能优化（并行嵌入 + 进度优化）
- 插件系统 MVP（借鉴 OpenCode Plugin V2）
- 模型对比面板（多模型并排对比回答质量）
- 检索结果手动排序/标注（用户反馈闭环）
- 文档版本管理（历史版本对比 + 回滚）

### 方向 3: 技术债清理
- TypeScript 残余类型错误修复（逐步迁移）
- Vitest 前端单元测试补全
- Pro 测试全量验证（`cargo test --features pro`）
- E2E 真实 LLM 测试扩展

### 方向 4: 文档治理
- `docs/SOFTWARE_ASSET.md` 全量同步最新状态
- ADR-014 RAG 评估指标系统架构决策记录
- ADR-015 Session Strip 架构决策记录

## 铁律提醒

1. **铁律一**: 代理仅限开发调研，软件本身禁止代理（所有 `reqwest::Client::builder()` 必须加 `.no_proxy()`）
2. **铁律二**: TDD 驱动（先写红灯测试再实现变绿）
3. **铁律五**: 生产代码严禁 `unwrap()/expect()/panic!()`（workspace deny lint 强制）
4. **铁律七**: 代码变更必须同步更新 `docs/api/` 对应文档
5. **铁律十一**: 修改源代码后必须运行 `./sync-to-github.sh`
6. **铁律十二**: Session 结束必须更新 `CURRENT_STATE.md` + `LOG.md` + 生成衔接提示词
7. **铁律十三**: Tailwind 样式规范（禁止硬编码 z-index/hex 颜色）
8. **铁律十五**: Spec-driven Development（先写规格再写代码）

## 验证命令速查

```bash
# 编译
cargo build
cargo build --release --features pro

# 测试
cargo test --workspace                    # Free (1224 passed)
cargo test --features pro                 # Pro
cargo test -p echomind-core -- session_strip_tests  # Session Strip 测试

# Lint
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check

# 前端
node scripts/build-ui.mjs                 # esbuild 打包
npx tsc --noEmit                          # TypeScript 类型检查
cd e2e-tests && npm run test:unit         # Vitest
cd e2e-tests && npm run test:bridge       # Playwright E2E

# 同步
./sync-to-github.sh                       # GitHub 发布仓库
```

## 关键文件索引

| 文件 | 用途 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
| `docs/api/ipc-commands.md` | IPC 命令签名（194 Free / 206 Pro）|
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 |
| `docs/api/database-schema.md` | 数据库表结构 |
| `docs/api/settings-reference.md` | 设置键 + 事件 |
| `docs/api/frontend-modules.md` | 前端模块函数清单 |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照 |
| `.catpaw/rules/system-constraints.md` | 铁律规则（21 条）|
| `AGENTS.md` / `CLAUDE.md` | AI Agent 指南 |
| `sync-to-github.sh` | GitHub 发布同步脚本 |

## Git 状态

- **分支**: `main`
- **工作区**: 干净（所有变更已提交）
- **发布仓库**: `EchoMind/EchoMind/`（通过 `sync-to-github.sh` 同步，已提交 11 files 1185 insertions）

## 请选择下个方向

请从上述「待定方向」中选择一个方向并说明理由，然后开始执行。建议优先考虑**方向 1（DS4 剩余借鉴）**或**方向 2（v1.5 新功能）**，因为技术债清理和文档治理可在功能开发间隙穿插进行。
