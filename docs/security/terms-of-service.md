# EchoMind 服务条款

> **状态**：活跃 | **最后更新**：2026-07-23 | **适用版本**：v0.1.0+

---

## 1. 接受条款

通过下载、安装或使用 EchoMind（灵犀）软件（以下简称"本软件"），你同意接受本服务条款的约束。如果你不同意本条款的任何部分，请不要使用本软件。

---

## 2. 软件描述

EchoMind 是一款基于 Rust + Tauri v2 的**本地 RAG（检索增强生成）知识库桌面应用**，具有以下特征：

- **开源**：基于 MIT 许可证开源
- **本地优先**：所有文档和向量数据存储在用户本地设备
- **自配 LLM**：用户自行配置 OpenAI 兼容的 LLM API
- **跨平台**：支持 macOS、Windows、Linux

---

## 3. 许可证

### 3.1 开源许可证

本软件基于 **MIT 许可证** 开源：

- 你可以自由使用、复制、修改、合并、发布、分发、再授权和/或销售本软件
- 必须在所有副本中包含版权声明和许可声明
- 本软件按"现状"提供，不提供任何明示或暗示的保证

完整许可证文本见项目根目录的 [LICENSE](../../LICENSE) 文件。

### 3.2 商业使用

在 MIT 许可证下，你可以将本软件用于商业用途，包括：

- 在企业内部部署
- 作为商业产品的一部分（需遵守 MIT 许可证条款）
- 提供基于本软件的服务

### 3.3 Pro 许可证（计划中）

EchoMind 计划提供 Community（免费）和 Pro（付费）两个版本。Pro 版本的额外功能可能受单独的商业许可证约束。详见软件内的 License 管理界面。

---

## 4. 用户责任

### 4.1 LLM API 费用

EchoMind 本身是免费的开源软件，但使用本软件需要配置 LLM API。**你需自行承担 LLM API 的使用费用**。EchoMind 不对任何 API 提供商的收费负责。

### 4.2 API 配置安全

- 你有责任保管好自己的 API Key
- EchoMind 将 API Key 存储在本地数据库中（可选加密）
- EchoMind 不会将你的 API Key 发送给任何第三方（LLM API 提供商除外，用于认证）

### 4.3 内容责任

- 你导入的文档内容由你自行负责
- AI 生成的回答仅供参考，不构成专业建议
- 对于法律、医疗、金融等领域的回答，请咨询专业人士

### 4.4 合法使用

你同意不会将本软件用于：
- 任何违反适用法律法规的活动
- 侵犯他人知识产权的行为
- 处理或存储非法内容
- 对任何第三方造成损害的行为

---

## 5. 知识产权

### 5.1 软件所有权

EchoMind 软件的版权归其贡献者所有，基于 MIT 许可证发布。

### 5.2 第三方组件

本软件包含以下第三方开源组件，各自受其原始许可证约束：

| 组件 | 许可证 | 用途 |
|---|---|---|
| Tauri | MIT/Apache-2.0 | 桌面应用框架 |
| Rust 标准库 | MIT/Apache-2.0 | 编程语言 |
| SQLCipher | BSD-3-Clause | 数据库加密 |
| fastembed / ort | MIT/Apache-2.0 | ONNX 嵌入推理 |
| rusqlite / r2d2 | MIT | SQLite 连接池 |
| axum | MIT | REST API 服务器 |
| marked.js | MIT | Markdown 渲染 |
| DOMPurify | Apache-2.0/MPL-2.0 | XSS 防护 |
| highlight.js | BSD-3-Clause | 代码高亮 |
| Mermaid | MIT | 图表渲染 |
| KaTeX | MIT | 数学公式渲染 |

完整依赖列表及许可证合规信息见 `deny.toml` 配置和 `cargo deny check` 输出。

### 5.3 嵌入模型

本软件使用的嵌入模型（all-MiniLM-L6-v2 等）来自 HuggingFace，各自受其许可证约束：

| 模型 | 许可证 | 来源 |
|---|---|---|
| all-MiniLM-L6-v2 | Apache-2.0 | sentence-transformers |
| multilingual-e5-base | MIT | intfloat |
| bge-base-en-v1.5 | MIT | BAAI |
| bge-reranker-base | MIT | BAAI |

---

## 6. 免责声明

### 6.1 按现状提供

本软件按"现状"和"可用"的基础提供，不提供任何明示或暗示的保证，包括但不限于：

- 适销性
- 特定用途的适用性
- 不侵权
- AI 回答的准确性或可靠性

### 6.2 AI 回答

- AI 生成的回答可能包含错误、不准确或过时的信息
- AI 回答不构成法律、医疗、金融或其他专业建议
- 在做出重要决策前，请验证信息并咨询专业人士
- EchoMind 不对 AI 回答的内容承担责任

### 6.3 数据安全

虽然 EchoMind 提供了多层安全防御（SQLCipher 加密、PII 检测、审计日志等），但：

- 没有任何安全措施是 100% 可靠的
- 用户应自行评估安全风险
- EchoMind 不对因使用本软件而导致的数据泄露承担责任
- 建议用户启用数据库加密功能并定期备份

### 6.4 服务可用性

- EchoMind 是本地桌面应用，不依赖 EchoMind 团队的服务器运行
- LLM API 的可用性取决于用户选择的 API 提供商
- EchoMind 团队不保证 GitHub Releases 下载服务的持续可用性

---

## 7. 责任限制

在任何情况下，EchoMind 团队或其贡献者均不对以下事项承担责任：

- 任何直接的、间接的、附带的、特殊的或后果性的损害
- 数据丢失或损坏
- 利润损失或业务中断
- 因使用或无法使用本软件而产生的任何损失

---

## 8. 贡献

欢迎社区贡献。通过提交 Pull Request，你同意：

- 你的贡献将基于 MIT 许可证发布
- 你拥有提交代码的知识产权或已获得授权
- 你同意 EchoMind 团队使用和分发你的贡献

详见 [CONTRIBUTING.md](../../CONTRIBUTING.md)。

---

## 9. 出口管制

EchoMind 使用 SQLCipher（AES-256-CBC 加密），受美国出口管制条例约束。详见 [出口管制分类](export-control.md)。

---

## 10. 条款变更

本服务条款可能随时间更新。重大变更将通过 GitHub 仓库公告告知用户。

- **当前版本日期**：2026-07-23
- **变更历史**：见 Git 提交历史

---

## 11. 适用法律

本条款受中华人民共和国法律管辖。如发生争议，应首先通过友好协商解决。

---

## 12. 联系方式

- GitHub Issues: https://github.com/echomind/echomind/issues
- Email: legal@echomind.dev（如有）

---

## 相关文档

- [隐私政策](privacy-policy.md)
- [出口管制分类](export-control.md)
- [GDPR 合规](gdpr-compliance.md)
