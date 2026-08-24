# ADR-008: Ed25519 数字签名许可证验证

> **状态**：✅ 已采纳
> **日期**：2026-07-26
> **决策者**：Chief Architect / TPM

## 背景

EchoMind 采用 Free + Pro 双层许可模式。需要：

- **离线验证**：用户无需联网即可验证 Pro 许可证
- **防伪造**：用户无法自行生成有效许可证
- **紧凑格式**：License Key 便于复制粘贴（非长串十六进制）
- **信息携带**：License 包含用户邮箱、到期时间、功能标志
- **独立 CLI**：许可证签发工具与主应用分离

## 决策

采用 Ed25519 数字签名 + Base64 编码：

- **算法**：Ed25519（Edwards-curve Digital Signature Algorithm）
- **密钥对**：项目维护者持有私钥（`genkey` CLI 生成），公钥编译进应用
- **License 格式**：`base64(payload)-base64(signature)`
  - payload：JSON `{ email, expiry, features }` → Base64
  - signature：Ed25519 签名 → Base64
- **验证流程**：`verify_license(key)` → 解析 payload + 用公钥验证签名
- **签发工具**：`crates/license-issuer/` 独立 CLI（`genkey` / `issue`）
- **存储**：License Key 存储在 settings 表（`license.key` + `license.is_pro`）
- **运行时**：`AppState.is_pro` 从 settings 表读取，影响功能门控

## 考虑的替代方案

| 方案 | 优点 | 缺点 | 为什么没选 |
|---|---|---|---|
| RSA 签名 | 广泛使用 | 密钥体积大，签名长 | Key 过长 |
| 在线验证 | 可实时吊销 | 需要服务器 + 网络 | 违反离线优先 |
| JWT | 标准 | 依赖外部库，格式冗余 | 过度工程 |
| 对称加密 HMAC | 简单 | 无法防伪造（私钥泄露=全部失效） | 安全性不足 |

## 后果

- 正面：完全离线验证，用户无需联网
- 正面：Ed25519 签名紧凑（64 字节），Base64 后 ~88 字符
- 正面：公钥编译进应用，无法被用户替换
- 正面：独立签发 CLI 与主应用解耦
- 负面：无法吊销已签发的许可证（除非加入过期时间检查）
- 负面：私钥泄露 = 所有许可证可被伪造（需妥善保管）
- 中性：License 过期需用户重新获取
