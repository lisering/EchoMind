# EchoMind 知识库测试文档

## 概述

EchoMind（灵犀）是一个本地 RAG 知识库桌面应用，专为律师、研究员等高隐私需求场景设计。
所有数据存储在本地，不发送到任何远程服务器（用户主动配置的 LLM 端点除外）。

## 核心特性

1. **本地向量化**：使用 all-MiniLM-L6-v2 ONNX 模型生成 384 维向量嵌入
2. **SQLite 存储**：WAL 模式 SQLite 数据库，支持 SQLCipher AES-256 透明加密
3. **混合检索**：向量相似度 + FTS5 关键词 BM25 排序，RRF 融合
4. **流式对话**：OpenAI 兼容 SSE 流式输出，支持中断
5. **隐私保护**：PII 检测脱敏 8 类（邮箱/电话/身份证/银行卡/IP/SSN/护照/国际电话）
6. **审计日志**：哈希链防篡改审计日志，可验证完整性

## 架构

EchoMind 采用六边形架构（端口与适配器），分为四层：

- `crates/models`：领域契约（纯数据结构）
- `crates/core`：业务逻辑与端口 Trait
- `crates/infra`：适配器实现（SQLite、ONNX、OpenAI）
- `crates/tauri-app`：Tauri 组装层

## 许可证

EchoMind 提供免费版和 Pro 版：

- 免费版：最多 50 个文件，不支持 PDF 导入
- Pro 版：无限制，支持 PDF、OCR、VLM、本地 LLM 推理
