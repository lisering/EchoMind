# License 与密钥管理

> **状态**：活跃 | **最后审阅**：2026-07-25
>
> 本文定义 EchoMind License 签发与校验的密钥管理策略。

---

## 1. 架构概述

EchoMind 采用 **Ed25519 离线签名验证**方案：

```
License 签发服务端（持有私钥）              客户端应用（内置公钥）
  ┌──────────────────────┐                  ┌──────────────────────┐
  │ 1. 生成 payload      │                  │ 4. 解析 Key           │
  │ 2. Ed25519 签名      │ ── Key 传输 ──→  │ 5. 用内置公钥验签     │
  │ 3. 拼接 Key 输出     │                  │ 6. 验签通过 → 激活 Pro │
  └──────────────────────┘                  └──────────────────────┘
```

- **公钥**：硬编码在 `crates/core/src/license.rs` 的 `BUILTIN_PUBKEY`
- **私钥**：**仅存在于 License 签发服务端**，不在本仓库、不在二进制中、不在 CI 中
- **算法**：Ed25519（RFC 8032），签名 64 字节

---

## 2. License Key 格式

```
base64(payload)-base64(signature)
```

- `payload`：签名的原始字节（如 `EchoMind-Pro-v1`）
- `signature`：64 字节 Ed25519 签名

---

## 3. 密钥管理策略

### 3.1 私钥管理

| 项 | 要求 |
|---|---|
| 存储位置 | License 签发服务端的安全密钥管理系统（如 KMS / HSM） |
| 仓库中 | **严禁** 入仓库（`.gitignore` 排除） |
| 二进制中 | **严禁** 编入二进制 |
| CI 中 | **严禁** 作为环境变量明文传递；如需 CI 签发，使用 GitHub Secrets + 短期凭据 |
| 访问控制 | 仅授权签发人员可访问；签发操作审计日志 |
| 轮换策略 | 如疑似泄露，立即轮换密钥对并发布应用更新 |

### 3.2 公钥管理

| 项 | 要求 |
|---|---|
| 存储位置 | 硬编码在 `crates/core/src/license.rs` 的 `BUILTIN_PUBKEY` 常量 |
| 替换时机 | 正式发布前替换为生产环境公钥（当前为开发环境密钥） |
| 版本管理 | 公钥变更时递增版本号，旧 License 失效 |

### 3.3 当前状态

> ⚠️ **开发环境密钥**：当前 `BUILTIN_PUBKEY` 是开发环境密钥对。
> 正式发布前**必须**替换为 License 服务器生成的生产密钥对中的公钥。

---

## 4. 安全分析

| 威胁 | 缓解措施 |
|---|---|
| 逆向工程提取公钥 | 公钥是公开信息，提取无法伪造签名（Ed25519 私钥不可推导） |
| 逆向工程提取私钥 | 私钥不在二进制中，无法提取 |
| 篡改 License Key | Ed25519 签名验证，篡改后签名不匹配 |
| 重放攻击 | License payload 包含版本标识，可区分不同版本 |
| 公钥替换攻击 | 公钥编译在二进制中，运行时不可修改；如需更新需发布新版本 |

---

## 5. 测试密钥对

`crates/core/src/license_tests.rs` 使用测试密钥对（与生产密钥不同）验证签名逻辑。

| 密钥 | 用途 |
|---|---|
| 测试私钥 | 签发测试 License Key |
| 测试公钥 | 注入 `verify_license_with_key()` 验证 |

测试密钥对**不得**用于生产签发。

---

## 6. 相关文件

| 文件 | 用途 |
|---|---|
| `crates/license-issuer` | License 签发 CLI 工具（genkey + issue 子命令，REQ-LIC-005） |
| `crates/core/src/license.rs` | License 校验实现 + 内置公钥 |
| `crates/core/src/license_tests.rs` | 校验逻辑测试（测试密钥对） |
| `crates/tauri-app/src/commands.rs` | `activate_pro` IPC 命令 |
