# EchoMind v1.0.0 发布说明

发布日期: 2026-08-06

## 概述

EchoMind（灵犀）是一个面向律师、研究员等高隐私场景的本地 RAG 知识库桌面应用。
基于 Rust + Tauri v2 构建，支持本地嵌入、本地 LLM 推理、SQLCipher 加密、PII 检测脱敏。

## 核心功能

- **文档导入**: Markdown / 纯文本 / PDF（多模态提取: 文本层 + OCR + VLM）
- **代码文件导入**: Rust / TypeScript / Python / Go（tree-sitter AST 符号感知分块）
- **语义检索**: fastembed ONNX (all-MiniLM-L6-v2, 384-dim) + 余弦相似度
- **混合检索**: 向量 + BM25 关键词 → RRF 融合
- **Cross-Encoder 重排序**: bge-reranker-base
- **HyDE 查询改写**: LLM 生成假设性文档增强检索
- **知识图谱**: 实体关系抽取 + 图遍历检索 + D3.js 可视化
- **Agent 多步推理**: ReAct 循环 + 并行工具执行 + 生命周期 Hooks
- **DAG 工作流**: 用户定义有向无环图 + 拓扑排序 + 并行执行
- **子代理舰队**: 多 Agent 并行 + mailbox 通信
- **代码执行沙箱**: Python / JavaScript 安全执行（Pro）
- **持久化记忆**: Wing/Hall/Room 三层 + LLM 辅助提取
- **极致性能**: 语义缓存金字塔 / Prompt 压缩 / Speculative RAG / 渐进式注入
- **安全**: SQLCipher AES-256 / Argon2id KDF / 8 类 PII 检测 / 审计哈希链 / 自动锁屏
- **本地 LLM**: mistral.rs v0.9.0 + GPU 加速 + 自研 GEMV 量化内核
- **国际化**: 中英双语

## Pro 功能

- 本地 LLM 推理（mistral.rs + Metal/CUDA）
- PDF 多模态提取（pdfium + ocrs + VLM）
- HNSW 近似最近邻索引
- ColBERT 多向量嵌入
- 代码符号引擎（tree-sitter）
- 代码执行沙箱
- 自研量化推理内核（GEMV + 权重重排 + 流式预取）

## 技术规格

- Rust Edition 2024 (≥1.85)
- Tauri v2
- 7 crate 六边形架构 (models / prompt / compact / core / infra / tauri-app / license-issuer)
- 49 个前端 ES module (vanilla JS, esbuild 打包)
- 131 个 IPC 命令
- 1721 个测试 (718 Free + 1003 Pro)
- 986 个 E2E 测试 (76 spec 文件)
- 407 个前端单元测试 (Vitest)

## 平台支持

- macOS (Intel + Apple Silicon)
- Windows 10/11
- Linux (Ubuntu 20.04+)
