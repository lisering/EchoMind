# EchoMind 出口管制分类

> **状态**：活跃 | **最后更新**：2026-07-23
>
> ⚠️ **重要**：EchoMind 使用 SQLCipher（AES-256-CBC 加密），属于美国出口管制条例管辖范围内的加密软件。本文档提供出口管制分类信息。

---

## 1. 概述

EchoMind 包含加密功能（SQLCipher AES-256-CBC 数据库加密），因此在美国出口管理条例（EAR, Export Administration Regulations）下属于"加密软件"类别。在向 GitHub 公开发布时，需要遵守相关出口管制要求。

---

## 2. 加密技术详情

### 2.1 加密算法

| 算法 | 用途 | 密钥长度 | 标准 |
|---|---|---|---|
| AES-256-CBC | 数据库透明加密 | 256 位 | FIPS 197 |
| Argon2id | 密钥派生 (KDF) | 256 位输出 | RFC 9106 |
| PBKDF2-HMAC-SHA256 | 密钥派生（向后兼容） | 256 位输出 | RFC 2898 |
| HMAC-SHA256 | 哈希链审计日志 | 256 位 | FIPS 198-1 |

### 2.2 加密实现

- **SQLCipher**：提供 AES-256-CBC 透明数据库加密
  - 来源：[Zetetic LLC](https://www.zetetic.net/sqlcipher/)
  - 许可证：BSD-3-Clause（社区版）
  - 集成方式：`rusqlite` crate 的 `bundled-sqlcipher` feature

- **Argon2id**：密钥派生
  - 来源：[`argon2` Rust crate](https://crates.io/crates/argon2)
  - 参数：64MB 内存 / 3 轮 / 4 并行度

### 2.3 加密用途

EchoMind 中的加密功能**仅用于保护用户本地数据**（数据库加密），**不用于通信加密**。

---

## 3. ECCN 分类

### 3.1 分类号

**ECCN（Export Control Classification Number）**: `5D002.c.1`

### 3.2 分类理由

| 分类层级 | 值 | 说明 |
|---|---|---|
| Category 5 | 5 | 信息技术 |
| Product Group D | D | 软件 |
| Control | 002 | 信息安全 |
| Paragraph c | c | 使用非对称算法的加密软件 |
| Sub-paragraph 1 | 1 | 已在大众市场零售渠道公开销售 |

### 3.3 例外情况

EchoMind 可能符合 **ENC 例外**（License Exception ENC）下的 **§740.17(b)(1)** — 即"公开可用"的加密软件：

- ✅ 源代码已公开在 GitHub（https://github.com/echomind/echomind）
- ✅ 免费提供（MIT 许可证）
- ✅ 可被公众自由下载
- ✅ 加密功能用于民用数据保护

---

## 4. 合规要求

### 4.1 GitHub 发布要求

根据 [GitHub 关于加密的文档](https://docs.github.com/en/github/site-policy/github-and-export-controls)，在 GitHub 上公开发布加密软件需要：

1. **源代码公开**：✅ 已满足（MIT 开源）
2. **ECCN 分类声明**：本文档即满足此要求
3. **出口管制声明**：在 README 或 LICENSE 中标注

### 4.2 二进制分发

当 EchoMind 发布预编译二进制文件（`.dmg`、`.msi`、`.AppImage`）时：

- 二进制文件包含编译进的可执行加密代码
- 发布到 GitHub Releases 时受 EAR 管辖
- 需在 Release 说明中标注出口管制信息

### 4.3 推荐的 README 声明

在 README.md 和 Release Notes 中包含以下声明：

```
## Export Control Notice

This software contains cryptographic components (AES-256, Argon2id) subject to 
the U.S. Export Administration Regulations (EAR). The ECCN classification is 
5D002.c.1. This software is publicly available under the MIT License and may 
qualify for License Exception ENC §740.17(b)(1).

See [docs/legal/export-control.md](docs/legal/export-control.md) for details.
```

---

## 5. 各司法管辖区注意事项

### 5.1 美国

- **BIS（工业与安全局）**：管辖加密软件出口
- **要求**：公开可用的加密源代码无需出口许可证，但需提交 BIS 分类请求（鼓励性）
- **URL**: https://www.bis.doc.gov/

### 5.2 中国

- **密码法**（2020 年 1 月 1 日生效）：商用密码产品管理
- **要求**：商业加密产品需遵守《中华人民共和国密码法》
- **URL**: http://www.npc.gov.cn/npc/c2/c30834/202001/t20200107_300581.html

### 5.3 欧盟

- **欧盟双用途物项出口管制条例** (Regulation (EU) 2021/821)
- **要求**：加密软件在欧盟内的分发和使用一般不受限制
- **URL**: https://trade.ec.europa.eu/doclib/docs/2021/april/tradoc_159583.pdf

### 5.4 其他国家

各国家对加密软件的进口和使用有不同法规。用户应自行了解所在国家/地区的相关法律。

---

## 6. BIS 自分类报告（可选）

虽然公开可用的开源加密软件无需提交 BIS 自分类报告，但建议提交以获得正式确认：

- **表格**：BIS Form 740.17
- **提交方式**：通过 [Snap-R](https://snapr.bis.doc.gov/) 系统
- **费用**：免费
- **处理时间**：约 2-4 周

---

## 7. 免责声明

本文档提供的信息仅供参考，不构成法律建议。出口管制法律复杂且可能随时变化。如需正式的出口管制分类意见，建议咨询专业律师或联系 BIS。

---

## 相关文档

- [隐私政策](privacy-policy.md)
- [服务条款](terms-of-service.md)
- [GDPR 合规](gdpr-compliance.md)
- [安全防御体系](../prd/01-functional/14-military-grade-privacy.md)
