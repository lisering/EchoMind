# EchoMind 隐私政策

> **状态**：活跃 | **最后更新**：2026-07-23 | **适用版本**：v0.1.0+

---

## 概述

EchoMind（灵犀）是一款**本地优先 (Local-First)** 的 RAG 知识库桌面应用。我们的核心隐私承诺是：**你的文档永远留在你的设备上**。

本隐私政策详细说明 EchoMind 如何收集、使用、存储和保护你的数据。

---

## 1. 数据存储

### 1.1 本地存储的数据

以下数据**完全存储在你的本地设备**上，使用 SQLite 数据库：

| 数据类型 | 说明 | 加密 |
|---|---|---|
| 导入的文档 | 你导入的 `.md`、`.txt`、`.pdf`、`.docx` 文件内容 | 可选（SQLCipher AES-256） |
| 文档分块 | 文档切分后的文本片段 | 可选（SQLCipher AES-256） |
| 向量嵌入 | 文档内容的向量表示（ONNX 本地推理） | 可选（SQLCipher AES-256） |
| 对话历史 | 你与 AI 的聊天记录 | 可选（SQLCipher AES-256） |
| 会话信息 | 会话标题、创建时间、消息计数 | 可选（SQLCipher AES-256） |
| 知识图谱 | 提取的实体和关系 | 可选（SQLCipher AES-256） |
| 应用设置 | 语言、主题、嵌入模型偏好等 | 否（非敏感数据） |
| 审计日志 | 敏感操作记录（PII 检测、文档删除等） | 可选（SQLCipher AES-256） |
| API Key | 你配置的 LLM API 密钥 | 是（数据库加密后） |

### 1.2 数据库加密

EchoMind 支持使用 SQLCipher 对数据库进行 **AES-256-CBC** 透明加密：

- **密钥派生**：Argon2id（64MB 内存 / 3 轮 / 4 并行度）
- **向后兼容**：旧版本使用 PBKDF2-HMAC-SHA256（100K 迭代）
- **密码不存储**：仅存储随机 salt，密码通过 `zeroize` 擦零
- **恒定时间比较**：防止时序攻击

加密是**可选的**——由用户在设置面板中主动启用。

---

## 2. 数据传输

### 2.1 发送到外部服务的数据

EchoMind 是本地优先应用，但以下功能需要网络通信：

| 功能 | 发送的数据 | 接收方 | 用户控制 |
|---|---|---|---|
| **RAG 对话** | 用户问题 + 检索到的文档片段（上下文） | 用户配置的 LLM API（如 OpenAI、DeepSeek、Ollama） | 用户必须手动配置 API |
| **OCR**（可选） | 图片的 Base64 编码数据 | 用户配置的视觉 LLM API（如 GPT-4o、GLM-4V） | 默认关闭，用户主动启用 |
| **远程嵌入**（可选） | 文本片段 | 用户配置的 Embedding API | 默认使用本地 ONNX 模型 |
| **模型下载** | 无用户数据，仅下载请求 | HuggingFace（hf-mirror.com 备用） | 首次使用嵌入功能时 |
| **更新检查** | 无用户数据，仅 HTTP GET 请求 | GitHub Releases API | 每 24 小时一次，可关闭 |

### 2.2 不发送的数据

以下数据**永远不会**被发送到任何外部服务：

- ❌ 你的原始文档文件（仅发送检索到的片段）
- ❌ 你的 API Key（仅用于认证，存储在本地）
- ❌ 你的数据库内容（除非你主动导出）
- ❌ 你的设备信息（更新检查不包含设备标识）
- ❌ 崩溃报告（EchoMind 不收集崩溃报告）
- ❌ 使用统计（EchoMind 不收集使用分析数据）

### 2.3 OCR 隐私说明

OCR 功能使用云端视觉模型处理图片。**启用 OCR 前，请确认你了解**：

- 图片内容将被发送到你配置的 LLM API
- 图片可能包含敏感信息（身份证、合同、医疗记录等）
- 建议：对隐私敏感的文档，**不要启用 OCR**，或使用本地 LLM（如 Ollama）

---

## 3. PII 检测与脱敏

EchoMind 内置 PII（个人身份信息）检测功能，可识别以下类型：

| PII 类型 | 检测模式 | 脱敏示例 |
|---|---|---|
| 邮箱地址 | RFC 标准格式 | `j***@example.com` |
| 中国手机号 | 1[3-9]xxxxxxxxx | `138****1234` |
| 身份证号 | 18 位（GB 11643） | `110***********1234` |
| 银行卡号 | 16-19 位（Luhn 校验） | `6222****1234` |
| IP 地址 | IPv4 | `192.168.*.*` |
| 国际手机号 | E.164 格式 | `+86**********` |
| 美国社会安全号 | XXX-XX-XXXX | `XXX-XX-XXXX` |
| 中国护照号 | E/G + 8 位数字 | `E********` |

所有 PII 检测操作均记录在本地审计日志中。

---

## 4. 第三方服务

EchoMind 不自带任何第三方分析、广告或追踪 SDK。但用户主动配置的 LLM API 提供商可能有自己的隐私政策：

| 提供商 | 用途 | 隐私政策 |
|---|---|---|
| OpenAI | LLM 对话 / OCR / 嵌入 | https://openai.com/privacy |
| Anthropic | LLM 对话 | https://www.anthropic.com/privacy |
| DeepSeek | LLM 对话 | https://www.deepseek.com/privacy |
| HuggingFace | 模型下载 | https://huggingface.co/privacy |
| GitHub | 更新检查 | https://docs.github.com/en/site-policy/privacy-policies |

**EchoMind 不控制第三方服务的隐私实践**。请查阅你所使用的 LLM 提供商的隐私政策。

---

## 5. 数据安全

### 5.1 七层安全防御

| 层级 | 防御措施 |
|---|---|
| 第一层 | SQLCipher AES-256-CBC 数据库加密 |
| 第二层 | Argon2id 密钥派生（抗 GPU/ASIC 暴力破解） |
| 第三层 | 自动锁屏 + IPC 拦截（guard_lock） |
| 第四层 | 内存擦零（zeroize）— 敏感数据用后即焚 |
| 第五层 | 剪贴板卫生 — 30 秒后自动清除 |
| 第六层 | 紧急销毁 — 连续输错密码触发数据擦除 |
| 第七层 | 审计溯源 — 哈希链验证的防篡改日志 |

### 5.2 安全删除

EchoMind 支持 **Crypto Shredding** 安全删除：

1. 使用 `randomblob()` 随机数据覆写
2. 使用 `zeroblob()` 清零
3. 执行 `VACUUM` 回收磁盘空间

确保被删除的文档无法通过磁盘恢复工具还原。

---

## 6. 数据权利

### 6.1 你的权利

作为本地优先应用，你对自己的数据拥有完全控制权：

| 权利 | 实现方式 |
|---|---|
| **访问权** | 所有数据在你的设备上，随时可查看 |
| **更正权** | 直接修改文档或对话内容 |
| **删除权** | 删除单条文档/对话，或使用安全删除 |
| **导出权** | 导出对话（MD/JSON/HTML）、导出知识库包、导出配置 |
| **清除全部** | 删除数据库文件即可清除所有数据 |
| **拒绝权** | 不启用 OCR / 远程嵌入 / 更新检查即可 |

### 6.2 数据导出

EchoMind 支持以下导出格式：

- 对话导出：Markdown、JSON、HTML
- 知识库包：包含文档+向量+设置的完整移植包
- 配置导出：LLM 配置、嵌入模型偏好等
- 诊断日志：系统信息 + 数据库状态（不含文档内容）

### 6.3 数据位置

所有数据存储在操作系统的应用数据目录：

| 操作系统 | 路径 |
|---|---|
| macOS | `~/Library/Application Support/com.echomind.app/` |
| Windows | `%APPDATA%\com.echomind.app\` |
| Linux | `~/.local/share/com.echomind.app/` |

---

## 7. 儿童隐私

EchoMind 不面向 13 岁以下儿童，不收集儿童的个人信息。

---

## 8. 隐私政策变更

本隐私政策可能随产品迭代而更新。重大变更将通过应用内通知告知用户。

- **当前版本日期**：2026-07-23
- **变更历史**：见 Git 提交历史

---

## 9. 联系我们

如有隐私相关问题，请通过以下方式联系：

- GitHub Issues: https://github.com/echomind/echomind/issues
- Email: privacy@echomind.dev（如有）

---

## 相关文档

- [服务条款](terms-of-service.md)
- [出口管制分类](export-control.md)
- [GDPR 合规](gdpr-compliance.md)
- [供应链安全](../security/supply-chain.md)
