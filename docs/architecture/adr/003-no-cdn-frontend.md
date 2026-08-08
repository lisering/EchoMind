# ADR-003: 前端禁止 CDN，本地打包三件套

> **状态**：✅ 已采纳
> **日期**：2026-07-24
> **决策者**：Chief Architect / Security Officer / TPM

## 背景

EchoMind 的核心承诺是「隐私不出域」。如果前端通过 CDN 加载库（如 Tailwind CSS、marked.js、DOMPurify、highlight.js），会引入以下风险：
- CDN 服务器可追踪用户 IP 和使用行为
- CDN 被劫持时可注入恶意代码（供应链攻击）
- 离线环境下前端无法加载（违反「本地优先」承诺）

## 决策

前端三件套（marked.js + DOMPurify + highlight.js）和 Tailwind CSS 全部本地打包于 `ui/vendor/` 目录，**严禁引入 CDN**。同时禁止 React/Vue/Svelte/jQuery 等任何框架与运行时依赖。

## 考虑的替代方案

| 方案 | 优点 | 缺点 | 为什么没选 |
|---|---|---|---|
| Tailwind CDN | 开发方便，无需构建 | 隐私泄露、离线不可用 | 违反核心承诺 |
| npm + bundler | 依赖管理规范 | 引入构建工具链复杂度 | 过度工程 |
| 框架（React/Vue） | 组件化开发 | 违反禁止框架铁律 | 架构纯洁性 |

## 后果

- 正面：零网络请求，完全离线可用
- 正面：零隐私泄露，CDN 无法追踪
- 正面：供应链安全，无 CDN 劫持风险
- 负面：库版本更新需手动替换 `ui/vendor/` 文件
- 负面：无构建工具链，前端组织完全靠手动
- 中性：前端代码为单文件 SPA，结构简单可控
