# ADR-005: 采用 fastembed ONNX 向量化引擎

> **状态**：✅ 已采纳
> **日期**：2026-07-25
> **决策者**：Chief Architect

## 背景

EchoMind 是本地优先的 RAG 知识库，向量化（embedding）是核心管线。需要满足：

- **完全本地推理**：不依赖外部 API（如 OpenAI Embeddings API）
- **CPU 推理**：不要求 GPU，桌面应用在普通笔记本上可用
- **模型体积小**：下载 < 100MB，不占用过多磁盘
- **嵌入维度适中**：存储和检索效率平衡
- **Rust 原生绑定**：避免 Python 依赖

## 决策

采用 `fastembed` crate（基于 ONNX Runtime）：

- **模型**：`all-MiniLM-L6-v2`（384 维，~30MB）
- **推理引擎**：ONNX Runtime（Microsoft 开源，CPU 推理优化）
- **Rust 绑定**：`fastembed` crate（封装 ONNX Runtime + HuggingFace tokenizers）
- **延迟初始化**：`tokio::sync::OnceCell` — 首次使用时才下载模型
- **并行推理**：`rayon` 线程池用于批量嵌入
- **向量存储**：f32 BLOB，余弦相似度在 Rust 中计算

## 考虑的替代方案

| 方案 | 优点 | 缺点 | 为什么没选 |
|---|---|---|---|
| OpenAI Embeddings API | 模型质量高 | 网络依赖 + API 费用 + 隐私风险 | 违反本地优先 |
| candle + sentence-transformers | 纯 Rust | 生态不成熟，模型加载复杂 | 稳定性不足 |
| Ollama embeddings | 已有本地推理 | 需额外安装 Ollama + 模型体积大 | 依赖过重 |
| text-embeddings-rs | Rust 原生 | 依赖 candle，编译复杂 | 编译问题 |

## 后果

- 正面：完全本地推理，零网络依赖
- 正面：30MB 模型体积小，首次下载快
- 正面：384 维存储效率高，余弦相似度计算快
- 正面：ONNX Runtime 跨平台支持好
- 负面：`all-MiniLM-L6-v2` 是 2023 年模型，检索质量非最优（P1-5 待升级）
- 负面：fastembed 依赖 ONNX Runtime C 库，增加二进制体积
- 负面：HuggingFace 下载在国内可能超时（需代理，铁律一开发调研允许）
