# EchoMind UI 与交互测试标准

> **文档定位**：本文件定义 EchoMind 全部 UI 与交互测试的标准、方法论、覆盖矩阵和验收准则。
> 所有测试用例必须从本标准派生。
>
> **版本**：v1.0.0 | **更新日期**：2026-08-18 | **适用版本**：EchoMind v2.2.0+

---

## 1. 测试金字塔

| 层级 | 占比 | 工具 | 运行时间 | 用途 |
|---|---|---|---|---|
| L1 单元测试 (Vitest) | 60% | Vitest + jsdom | ~10s | 纯逻辑函数/状态管理/工具函数 |
| L2 桥接 E2E (Playwright) | 30% | Playwright + tauri-stub | ~10min | UI 交互/DOM 断言/事件流 |
| L3 真实 GUI E2E | 10% | WebdriverIO + tauri-driver | ~30min | 真实 IPC/原生窗口 |

> **注**：L3 不支持 macOS，仅 Linux/Windows CI。

---

## 2. 测试覆盖矩阵

### 2.1 功能域覆盖

| 域 | 模块数 | L1 覆盖目标 | L2 覆盖目标 | 总用例目标 |
|---|---|---|---|---|
| 聊天核心 | 8 | 80% | 100% | 200+ |
| 侧栏/导航 | 6 | 70% | 100% | 150+ |
| 设置面板 | 5 | 70% | 100% | 180+ |
| 文档管理 | 4 | 60% | 100% | 120+ |
| 安全/加密 | 3 | 60% | 100% | 80+ |
| 知识图谱 | 2 | 50% | 100% | 60+ |
| Agentic AI 面板 | 4 | 50% | 100% | 80+ |
| 输入区/快捷键 | 6 | 80% | 100% | 150+ |
| 国际化/主题 | 2 | 60% | 100% | 50+ |
| 导出/打印 | 1 | 50% | 100% | 40+ |
| 工具/面板 | 10 | 50% | 100% | 100+ |
| 无障碍/键盘 | 3 | 40% | 100% | 60+ |
| 错误处理 | 2 | 60% | 100% | 50+ |
| 状态管理 | 2 | 90% | 80% | 40+ |
| 工具函数 | 3 | 90% | 50% | 30+ |
| **合计** | **76** | **~65%** | **~95%** | **~1300+** |

### 2.2 交互类型覆盖

| 交互类型 | 覆盖要求 |
|---|---|
| 鼠标点击 | 100% 按钮/链接/可点击元素 |
| 键盘导航 | 100% 快捷键 + Tab 序列 |
| 拖拽 | 文件导入 + 图谱节点拖拽 |
| 滚动 | 聊天区 + 列表 + 面板内滚动 |
| 输入 | 文本输入 + IME 组合 + 粘贴 |
| 悬停 | 按钮 hover 效果 + tooltip |
| 右键 | 上下文菜单全部变体 |
| 触摸（预留） | 不要求（桌面应用） |

### 2.3 状态覆盖

| 状态类型 | 覆盖要求 |
|---|---|
| 空状态 | 空知识库 + 空会话 + 空消息 |
| 加载状态 | 骨架屏 + 进度条 + loading spinner |
| 错误状态 | 8 种错误分类全覆盖 |
| 流式状态 | preparing/retrieving/generating/token/done/error/abort |
| 边界状态 | 最大输入 + 空输入 + 特殊字符 |
| 并发状态 | 多面板叠加 + 排队发送 + 重复点击 |

---

## 3. 测试方法论

### 3.1 AAA 模式

所有测试遵循 Arrange-Act-Assert 模式：

```
// Arrange: 准备测试环境（Mock state / DOM / IPC）
// Act: 执行被测函数/交互
// Assert: 验证结果（DOM 断言 / state 断言 / IPC 调用断言）
```

### 3.2 测试命名规范

```
TC-<域>-<序号>: <场景描述>
```

示例：`TC-CHAT-001: 发送消息后显示用户气泡和AI回复`

### 3.3 Mock 策略

| 依赖 | Mock 方式 | 文件 |
|---|---|---|
| Tauri IPC | `window.__TAURI__` stub | `tauri-stub.js` |
| LLM API | Mock `chat_token`/`chat_done` 事件 | tauri-stub.js |
| 文件系统 | Mock `openDialog` 返回路径 | tauri-stub.js |
| 数据库 | 内存 Mock state | tauri-stub.js |
| 时间 | `vi.useFakeTimers()` | Vitest |
| DOM | jsdom + `document.body.innerHTML` | Vitest |

### 3.4 等待策略

| 场景 | 策略 |
|---|---|
| DOM 元素出现 | `page.locator(selector).waitFor({ state: 'visible' })` |
| 流式完成 | `waitForStreamDone(page)` |
| Toast 消失 | `page.locator('.toast').waitFor({ state: 'hidden' })` |
| 动画完成 | `page.waitForTimeout(300)` (duration-normal) |
| IPC 响应 | Mock 延迟 × `E2E_SPEED` |

---

## 4. 验收准则

### 4.1 功能验收

| 准则 | 要求 |
|---|---|
| 每个交互规格均有测试 | 100% |
| 每个边界规格均有测试 | 100% |
| 每个状态规格均有测试 | 100% |
| 每个错误分类均有测试 | 100% |
| 每个快捷键均有测试 | 100% |

### 4.2 无障碍验收

| 准则 | 要求 |
|---|---|
| WCAG AA 对比度 | 4.5:1（正常文本）/ 3:1（大文本） |
| 键盘可达性 | 所有操作可通过键盘完成 |
| Focus Trap | 所有 modal 面板激活 Focus Trap |
| aria-label | 所有图标按钮有 aria-label |
| aria-live | 流式状态和错误有 aria-live |
| 屏幕阅读器 | 状态变化有 SR 播报 |

### 4.3 性能验收

| 准则 | 要求 |
|---|---|
| 首次内容绘制 | < 1s |
| 首启向导可交互 | < 2s |
| 面板打开动画 | 250ms ± 50ms |
| Toast 消失 | 3-5s |
| 流式首 token | < 2s（Mock） |
| 列表滚动 | 60fps（无卡顿） |
| 侧栏折叠动画 | 300ms ± 50ms |

### 4.4 视觉验收

| 准则 | 要求 |
|---|---|
| 暗色主题 | 所有元素可读 |
| 浅色主题 | 所有元素可读，对比度 ≥ 4.5:1 |
| Z-index 分层 | 无遮挡冲突 |
| 圆角一致性 | 按设计令牌 |
| 阴影一致性 | 按设计令牌 |
| 动效一致性 | 按设计令牌 |

---

## 5. 测试分类定义

### 5.1 冒烟测试 (Smoke)

快速验证核心路径可用性。~50 个用例，< 2 分钟。

### 5.2 功能测试 (Functional)

逐功能验证交互规格。~800 个用例，~15 分钟。

### 5.3 边界测试 (Boundary)

验证边界条件和异常输入。~150 个用例，~5 分钟。

### 5.4 状态测试 (State)

验证状态转换和状态持久化。~100 个用例，~5 分钟。

### 5.5 并发测试 (Concurrency)

验证并发操作和竞态条件。~50 个用例，~5 分钟。

### 5.6 无障碍测试 (Accessibility)

验证 WCAG AA 合规。~50 个用例，~5 分钟。

### 5.7 视觉回归测试 (Visual)

截图对比验证视觉一致性。~20 个快照，~5 分钟。

### 5.8 性能测试 (Performance)

验证响应时间达标。~30 个用例，~3 分钟。

### 5.9 压力测试 (Stress)

大数据量/长时间运行。~20 个用例，~10 分钟。

### 5.10 端到端测试 (E2E)

完整用户流程验证。~50 个用例，~10 分钟。

**总预计**：~1300+ 用例，~65 分钟（桥接模式），加上 Vitest ~10 分钟，总计 ~75 分钟。

> 10 小时+ 的测试覆盖通过以下方式实现：
> 1. 桥接 E2E 全量运行 ~75 分钟
> 2. 多轮回归（至少 8 轮 = 10 小时）
> 3. 真实 GUI E2E（L3）~30 分钟/轮
> 4. 真实 LLM E2E ~30 分钟/轮
> 5. Chaos/模糊测试 ~10 分钟/轮

---

## 6. 测试文件组织

```
e2e-tests/tests/
├── unit/                           # L1 Vitest 单元测试
│   ├── *.test.js                   # 62+ 个单元测试文件
├── bridge/                         # L2 桥接测试基础设施
│   └── tauri-stub.js               # Mock IPC
├── *.spec.ts                       # L2 E2E 测试
├── helpers.mjs                     # 共享辅助函数
├── comprehensive-ux.spec.ts        # 综合UX测试
├── edge-cases.spec.ts              # 边界测试
├── stress-edge.spec.ts             # 压力测试
├── accessibility.spec.ts           # 无障碍测试
├── visual-regression.spec.ts       # 视觉回归测试
├── ui-latency.spec.ts              # 性能测试
└── ...                             # 功能域测试
```

---

## 7. CI 集成

```yaml
# .github/workflows/ci.yml
test-ui:
  - npm run test:unit        # L1 Vitest
  - npm run test:bridge      # L2 Playwright
  # L3 不在 CI 运行（macOS 不支持 tauri-driver）
```

---

> **文档结束** — 本标准定义了 EchoMind 全部 UI 测试的方法论、覆盖矩阵和验收准则。
