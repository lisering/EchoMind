# EchoMind v1.22.0 变更日志

> **发布日期**: 2026-08-14
> **版本**: 1.20.0 → 1.22.0

---

## 🎯 本版本核心

**S5/S6/S7 七轮审计闭环完成 + E2E 205→0 修复 + DeepSeek Harness 架构分析**

本版本标志着 EchoMind 完成 S1-S7 七轮验收评估审计的全部行动清单（P0/P1/P2 共 16 项），E2E 测试从 205 个失败修复至 0 failed，并完成了 DeepSeek Harness 项目的全面架构分析，产出 10 项借鉴评估。

---

## ✅ 新增与改进

### 前端 UI 改进

| 改进 | 来源 | 详情 |
|------|------|------|
| 智能模式单一开关 | S3 P0-4 + S4 U4 | 12 个学术术语开关 → 1 个智能模式开关（默认开启混合搜索 + Rerank + HyDE + 知识图谱 + 质量门控 + 压缩 2x） |
| 输入区 5→2 toggle 精简 | S3 P0-2 | 保留混合搜索 + Agent 模式，移除 sub_agent/memory/web_search |
| 设置面板 35→8 分区重整 | S3 P0-1 | 模型配置/外观语言/知识库/安全隐私/智能检索/高级调优/应用设置/快捷指令 |
| 20 术语通俗化 | S3 P0-3 | Speculative RAG → 快速预生成，HyDE → 查询改写等 |
| 11 幻影调用修复 | S2 P0 | 前端 ipc.js 删除旧封装 + 迁移到新命令名 |
| 4 开发者工具移入隐藏菜单 | S3 P0-5 | AutoDream/符号搜索/工作流/trace 移入开发者菜单 |
| 顶栏 8→5 按钮精简 | S3 P1-1 | 移除 3 个开发者工具按钮 |
| 5 个 F 级模块删除 | S1 P1 | workflow-panel/code-executor/durable-panel/session-strip/perf-settings |
| HTML 导出增强 (P2-1) | S4 N2 | hljs 语法高亮 + Mermaid 标注 + CSS 主题 |
| VLM 分区调整 (P2-2) | S4 N7 | 从智能检索移入模型配置 |
| 设置 Tab 滚动优化 (P2-3) | S3/S4 | scroll-padding-top: 52px |
| 斜杠命令键盘导航 (P2-4) | S3 | Tab 选择 + Home/End 跳转 |

### 错误处理改进

| 改进 | 来源 | 详情 |
|------|------|------|
| 错误信息统一分类 | S3 P1-2 | EMBED:/PRO_REQUIRED 前缀移除，60% 技术化错误 → 人类可理解 |
| 网页搜索替代方案 | S4 N8 | 输入区 toggle → 设置面板开关 |
| Agent 发现性改善 | S4 N4/N5 | 设置面板入口 + 快捷键 |
| 导出发现性改善 | S4 N1 | 导出菜单入口优化 |

### 代码质量

| 改进 | 详情 |
|------|------|
| ThinkingPanelHandle 类型完善 | 新增 startThinking/ensureMinLoadingDelay/getThinkDuration/markFirstTokenReceived/isFirstTokenReceived 方法声明 |
| setComplete 异步化 | ensureMinLoadingDelay 需 await，测试同步适配 |
| Vitest 测试修复 | TC-DS-009/010c/010d 适配 async setComplete |

---

## 📊 测试基线

| 指标 | v1.20.0 | v1.22.0 | 变化 |
|------|---------|---------|------|
| Rust Free | 1282 passed | 1282 passed | — |
| Rust Pro | 1573 passed | 1573 passed | — |
| Clippy (Free+Pro) | 零警告 | 零警告 | — |
| cargo fmt | 通过 | 通过 | — |
| TypeScript | 0 errors | 0 errors | — |
| esbuild | 1622 行 | 1626 行 | +4 行 |
| Vitest | 440 passed | 440 passed | — |
| E2E | 1354 passed, 0 failed | 1354 passed, 0 failed | — |
| IPC 命令 | 242 | 242 | — |
| 幻影调用 | 0 | 0 | — |
| 僵尸命令 | 0 | 0 | — |

---

## 🔍 DeepSeek Harness 架构分析

本版本期间完成了 `deepseek-ai/deepseek-harness` 项目的全面源码分析，产出 10 项借鉴评估：

| # | 借鉴点 | 优先级 | 预估工作量 |
|---|--------|--------|-----------|
| DH-01 | Event Sourcing 会话持久化 | 高 | 3-4 Session |
| DH-02 | 沙箱 Per-call 策略 + Fail-closed | 高 | 2 Session |
| DH-03 | 子智能体 Provider 注册表 | 高 | 3 Session |
| DH-04 | 审计对 + Turn-Enclosed | 中 | 1-2 Session |
| DH-05 | 权限预设组合 | 中 | 1 Session |
| DH-06 | 拒绝签名分类 | 中 | 1 Session |
| DH-07 | 修订令牌乐观并发 | 中 | 1 Session |
| DH-08 | Packed Chunks 压缩 | 低 | 1 Session |
| DH-09 | Effect 系统 | 低 | 2 Session |
| DH-10 | Waterfall 事件模式 | 低 | 1 Session |

**下一阶段实施路径**：DH-03（子智能体 Provider 注册表）→ DH-02（沙箱 Per-call）→ DH-04（审计对）

---

## 📁 修改文件清单

| 文件 | 变更 |
|------|------|
| `crates/tauri-app/tauri.conf.json` | 版本号 1.20.0 → 1.22.0 |
| `types/dom-ext.d.ts` | ThinkingPanelHandle 新增 5 个方法声明 |
| `e2e-tests/tests/unit/thinking-panel.test.js` | 3 个测试适配 async setComplete |
| `CHANGELOG_v1.22.md` | 新增 |

---

## 🚀 下一阶段方向

1. **DeepSeek Harness 借鉴实施**（DH-03 → DH-02 → DH-04）
2. **Servo 浏览器集成**（重新评估触发条件待定）
3. **v1.3 版本剩余功能**（DS-04~DS-07 功率管理/Tool Call 续接等）
