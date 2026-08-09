<p align="center">
  <strong>本地优先的 AI 知识库 — 极速 · 隐私 · 自带 Key</strong>
</p>

<p align="center">
  基于 Rust + Tauri v2 的桌面应用，使用你自己的大模型 API Key 进行私有 RAG（检索增强生成）。
</p>

<p align="center">
  <a href="#-什么是-echomind">功能</a> ·
  <a href="#-快速开始">快速开始</a> ·
  <a href="#-架构设计">架构</a> ·
  <a href="#-技术栈">技术栈</a> ·
  <a href="README.md">📖 English</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.85%2B-orange?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/Tauri-v2-blue?logo=tauri" alt="Tauri v2">
  <img src="https://img.shields.io/badge/Edition-2024-orange" alt="Edition 2024">
  <img src="https://img.shields.io/badge/License-MIT-blue" alt="MIT 许可证">
  <img src="https://img.shields.io/badge/测试-987%20passed-brightgreen" alt="测试">
  <img src="https://img.shields.io/badge/平台-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey" alt="跨平台">
</p>

---

## 🌍 语言 / Language

[English](README.md) · **简体中文**（当前）

---

## 🚀 什么是 EchoMind？

EchoMind 是一款桌面应用，让你用**自己的本地文档**与任何 OpenAI 兼容的大模型对话。你的文件永远不会离开你的设备——解析、分块、向量化、向量存储全部在本地完成。你自带 API Key（**BYOK**），完全掌控成本和数据。

> **核心价值主张：Rust 极速 · 隐私不出域 · MIT 开源**

### 为什么选择 EchoMind？

| | EchoMind | AnythingLLM | Open WebUI | Jan |
|---|:---:|:---:|:---:|:---:|
| **运行时** | Rust + Tauri（~15 MB） | Electron（~150 MB+） | Python + Docker | Tauri |
| **内存占用** | 极低 | 高 | 中高 | 中 |
| **RAG 知识库** | ✅ | ✅ | ✅ | ❌ |
| **BYOK（自带 API Key）** | ✅ | ✅ | ✅ | 本地模型 |
| **本地向量化（ONNX）** | ✅ | ✅ | ✅ | ❌ |
| **本地大模型（GGUF）** | ✅ | ❌ | ❌ | ✅ |
| **数据库加密** | ✅ SQLCipher | ❌ | ❌ | ❌ |
| **许可证** | MIT | MIT | MIT | MIT |
| **零服务器成本** | ✅ | ❌ | ❌ | ✅ |

---

## ✨ 功能特性

### 📥 文档智能处理
- **多格式支持** — Markdown、文本、代码文件（Rust/TS/Python/Go）、HTML、PDF、DOCX、PPTX、EPUB、XLSX/CSV
- **100% 本地处理** — 解析、分块、向量化、向量存储全部在设备端完成
- **语义分块** — 段落 → 句子 → 子句递归分割，保留代码块完整性
- **章节感知分块** — Markdown 标题层级 → 按章节边界分块
- **ONNX 嵌入模型** — all-MiniLM-L6-v2（384 维，约 30 MB），通过 fastembed 本地运行
- **自定义嵌入模型** — 上传自定义 ONNX 模型
- **SQLite 向量存储** — WAL 模式，FTS5 全文索引，零配置
- **HNSW 索引** — 近似最近邻搜索，亚线性复杂度
- **文件去重** — MD5 内容哈希防止重复导入
- **崩溃恢复** — 中断的索引任务在重启时自动恢复

### 💬 RAG 对话
- **混合检索** — 向量搜索 + BM25 关键词匹配 → RRF 融合
- **Cross-Encoder 重排序** — bge-reranker-base 精准重排
- **HyDE 查询改写** — LLM 生成假设性答案 → 嵌入 → 搜索
- **知识图谱** — 实体抽取 + 关系挖掘 → 图遍历检索
- **Agentic RAG** — ReAct 多步推理，并行工具执行
- **渐进式上下文注入** — 从 top-2 chunk 开始，不足时自动扩展
- **Speculative RAG** — 草稿模型生成，验证模型确认
- **检索记忆** — 基于查询类型自适应选择检索方法
- **语义缓存** — 三级缓存（精确 / 语义 / 检索）秒回响应
- **上下文压缩** — LLM 历史摘要替代截断
- **进度阶段提示** — 准备中 → 检索中 → 生成中，无空白等待
- **可中断生成** — 中途停止，已产生内容自动保存
- **多轮对话** — 完整聊天历史，自动提取标题
- **分支树** — ChatGPT 风格可视化对话分支

### 🧠 本地大模型引擎
- **GGUF 推理** — mistral.rs v0.9.0，纯 Rust 实现
- **GPU 加速** — Metal（macOS）/ CUDA（NVIDIA）/ Accelerate（Apple BLAS）
- **PagedAttention** — 高效 KV cache 管理，支持长对话
- **采样参数** — temperature、top-p、top-k、重复惩罚
- **KV cache 持久化** — 跨会话保存/恢复
- **自研 GEMV 内核** — 4 种量化格式推理（Q4_0/Q4_K/Q8_0/Q8_K）
- **权重重排** — CPU cache 友好 Tile-Major 布局
- **Layer 预取** — `madvise(MADV_WILLNEED)` 流式预取
- **RAM 预算** — LRU 驱逐 + 系统内存感知
- **模型下载管理器** — 暂停/恢复/取消 + 崩溃恢复

### 🔒 安全架构
- **SQLCipher 加密** — AES-256 透明数据库加密
- **Argon2id 密钥派生** — 内存硬化 KDF（m=19456KB, t=2, p=1）+ PBKDF2 降级
- **PII 检测与脱敏** — 8 种类型（邮箱/电话/身份证/银行卡/IP/SSN/护照/国际电话）
- **审计哈希链** — SHA-256 链式审计日志，篡改可检测
- **自动锁定** — 空闲超时 → 锁定状态
- **暴力破解防护** — 5 次失败 → 指数退避
- **剪贴板自动清除** — 敏感数据超时自动清除
- **API Key 脱敏** — `****` + 最后 4 位，永不显示明文
- **安全态势分层** — Dangerous / Auto / Strict 三级 + 影子筛查
- **`forbid(unsafe_code)`** — 生产 crate 内存安全由 Rust 保证

### 🎨 富内容渲染
- **Markdown** — 代码语法高亮（highlight.js）
- **Mermaid 图表** — 流程图、时序图、甘特图
- **KaTeX 数学公式** — 行内和块级 LaTeX 公式
- **Chart.js** — 交互式数据可视化
- **双向链接笔记** — Obsidian 风格 `[[wiki-link]]` + 反向链接面板
- **无 CDN 依赖** — 所有前端库本地打包

### 🛠 高级工具
- **AutoDream** — 后台空闲整理：重复检测、矛盾发现
- **持久化记忆** — 三层（Wing/Hall/Room）+ LLM 整合
- **代码符号搜索** — tree-sitter AST 抽取（Rust/TS/Python/Go）
- **代码执行沙箱** — Python/Node，超时/内存/网络限制
- **DAG 工作流** — 可视化工作流构建器 + 模板管理
- **网页搜索融合** — DuckDuckGo Instant Answer + RRF 本地融合
- **知识图谱可视化** — D3.js 力导向图 + 社区检测
- **PDF 导出** — `window.print()` 零依赖导出
- **对话导出** — Markdown 格式，含来源引用
- **文件夹同步** — 文件监听 + 增量同步（新增/更新/删除）

### 🖥 跨平台
- macOS（Apple Silicon + Intel）
- Windows x64
- Linux x64
- 基于 Tauri v2 — 原生性能，非 Electron 套壳

### 🗺 路线图

#### v0.1.0-alpha（当前）
- 初始 alpha 版本，核心 RAG 功能
- 全功能开放，无任何限制

---

## 🏗 架构设计

六边形架构（端口与适配器），8 个 crate。依赖方向严格向内：

```
crates/models → crates/prompt → crates/core → crates/infra → crates/tauri-app
  (契约层)       (提示词)        (端口+逻辑)   (适配器)       (装配层)
                                   ↑
                            crates/compact
                            crates/context
```

| Crate | 职责 |
|---|---|
| `crates/models` | 领域契约（Document、Chunk、ChatMessage 等） |
| `crates/prompt` | 提示词构建：SegmentedPrompt、RAG/Agent 提示词、缓存策略 |
| `crates/compact` | 上下文压缩引擎：LLM 历史摘要 |
| `crates/context` | 系统上下文注册表：纪元管理、持久化基线 |
| `crates/core` | 端口 Trait + 业务逻辑；聊天引擎、导入服务、安全层 |
| `crates/infra` | 适配器：SqliteStorage、LocalEmbedder、OpenAIProvider、HNSW、LocalLlmEngine、OCR、VLM |
| `crates/tauri-app` | Tauri 外壳，190+ IPC 命令，AppState |

**前端**：单文件 SPA（`ui/index.html`）— 50 个 ES 模块经 esbuild 打包。Tailwind CSS（本地 JIT），原生 JavaScript。**无 CDN，无框架。**

### 数据流

**文档导入**（100% 本地）：
```
import_files → Loader.load() → MD5 去重 → Splitter.split()
  → Storage.add_document() + add_chunk() → Embedder.embed_batch()
  → Storage.add_embedding() → EntityExtractor → doc-status-changed 事件
```

**RAG 查询**（BYOK）：
```
chat → 嵌入查询（本地 ONNX）→ 混合搜索（向量 + BM25 → RRF）
  → 重排序（bge-reranker）→ 构建 RAG Prompt → LLM chat_stream（SSE）
  → chat_token 事件 → chat_done → 持久化对话
```

---

## ⚡ 快速开始

### 前置条件

- **Rust** ≥ 1.85（Edition 2024）— [安装](https://rustup.rs/)
- **Node.js** ≥ 18（E2E 测试用，可选）

### 构建与运行

```bash
# 克隆仓库
git clone https://github.com/lisering/EchoMind.git
cd EchoMind

# 编译
cargo build

# 开发模式（热重载）
cargo tauri dev

# 编译全部功能
cargo build --features pro
```

> **注意**：首次编译需 5-10 分钟（ML 依赖以 opt-level=3 编译），后续增量编译很快。

### 测试

```bash
# Rust 单元测试 + 集成测试（987 个测试）
cargo test

# 代码检查（零警告策略）
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# 供应链安全
cargo audit
cargo deny check

# 前端类型检查
npx tsc --noEmit

# 前端构建
node scripts/build-ui.mjs
```

### 使用指南

1. **启动** EchoMind
2. **配置** — 进入设置 → 填写 LLM 服务商信息（API Key、Base URL、模型名称）
3. **导入** — 拖拽文件到窗口（支持所有格式）
4. **等待** — 索引完成（本地 ONNX 向量化，观察进度徽标）
5. **对话** — 在输入框输入问题，获得带引用来源的流式回答

### 支持的 LLM 服务商

任何 OpenAI 兼容的 API 端点均可使用：

| 服务商 | Base URL | 说明 |
|---|---|---|
| OpenAI | `https://api.openai.com/v1` | 默认 |
| Anthropic | `https://api.anthropic.com/v1` | 通过 OpenAI 兼容端点 |
| DeepSeek | `https://api.deepseek.com/v1` | 国产大模型 |
| Ollama（本地） | `http://localhost:11434/v1` | API Key 留空 |
| LM Studio | `http://localhost:1234/v1` | 本地模型运行器 |
| 本地 GGUF | — | 内置 mistral.rs 引擎，无需外部服务 |
| 任意 OpenAI 兼容 | 自定义 Base URL | 只要兼容 OpenAI API 即可 |

---

## 🛠 技术栈

| 层 | 技术 | 详情 |
|---|---|---|
| 语言 | Rust（Edition 2024） | 原生 `async fn` in trait，禁用 `async-trait` 宏 |
| 桌面框架 | Tauri v2 | 比 Electron 更小、更快、更安全 |
| 嵌入模型 | fastembed（ONNX Runtime） | all-MiniLM-L6-v2，384 维，约 30 MB |
| 本地大模型 | mistral.rs v0.9.0 | GGUF，Metal/CUDA，PagedAttention |
| 向量存储 | SQLite（rusqlite + r2d2） | WAL 模式，FTS5，SQLCipher AES-256 |
| 大模型 API | OpenAI 兼容 | SSE 流式，连接超时 30 秒 |
| 前端 | 原生 JS ES 模块 | esbuild IIFE 打包，无 React/Vue/Svelte |
| 渲染 | marked.js, DOMPurify, highlight.js | + Mermaid, KaTeX, Chart.js, D3.js |

### 代码质量

- **Clippy** — 零警告策略（`-D warnings`），deny lints 覆盖 `unwrap_used`、`expect_used`、`panic`、`unreachable`、`todo`、`unimplemented`
- **TDD 驱动** — 测试先行，987 个测试，单元测试与源码同文件
- **供应链安全** — 每次 CI 运行 `cargo audit` + `cargo deny check`
- **文档注释** — 所有公开类型有 `///` 文档注释
- **禁止 unsafe** — 生产 crate 使用 `forbid(unsafe_code)`

---

## 📄 许可证

**MIT 许可证** — 详见 [LICENSE](LICENSE)。

---

## 🙏 致谢

- [Tauri](https://tauri.app/) — 优秀的 Rust 桌面框架
- [fastembed](https://github.com/Anush008/fastembed-rs) — 让 ONNX 嵌入变得简单
- [SQLite](https://www.sqlite.org/) — 世界上最可靠的嵌入式数据库
- [mistral.rs](https://github.com/EricLBuehler/mistral.rs) — 纯 Rust LLM 推理
- Rust 社区 — 构建让软件又快又安全的工具

---

<p align="center">
  <a href="README.md">📖 English</a> ·
  <a href="LICENSE">MIT</a>
</p>

<p align="center">
  Made with ❤️ and Rust
</p>
