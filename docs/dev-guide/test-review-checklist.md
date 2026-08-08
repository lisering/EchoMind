# 测试设计评审制度（Test Design Review Checklist）

> **目的**：消除根因 V1（实现驱动测试）和 V3（空状态覆盖缺失），建立可持续的测试质量保障机制。

---

## 一、PR 提交前自检 Checklist

新增或修改 E2E 测试时，提交者必须在 PR 描述中回答以下问题：

### 1.1 测试视角

- [ ] **用户视角**：测试名称描述的是用户可见行为，还是实现细节？
  - ❌ `E2E-XX: sidebar-collapsed 类存在`
  - ✅ `E2E-XX: 折叠后侧栏完全不可见`

- [ ] **断言用户可见效果**：断言的是 DOM 结构/类名，还是用户能感知的效果？
  - ❌ `expect(sidebar).toHaveClass(/sidebar-collapsed/)`
  - ✅ `expect(sidebar.getBoundingClientRect().width).toBe(0)`

- [ ] **实现无关性**：如果实现改变（如 CSS 类名从 `sidebar-collapsed` 改为 `sidebar-hidden`），测试是否需要修改？
  - 如果需要修改 → 测试耦合了实现，应重构

### 1.2 路径覆盖

- [ ] **正向路径**：前置条件满足时功能正常？
- [ ] **空状态路径**：前置条件不满足时 UI 行为正确？
- [ ] **状态转换路径**：前置条件从不满足变为满足时 UI 正确响应？
- [ ] **错误恢复路径**：后端出错时前端有可见反馈 + 恢复可用？

### 1.3 Mock 真实性

- [ ] **Mock 准确性**：mock 返回值是否准确模拟后端行为？
- [ ] **空状态 mock**：mock 返回空数组/null 时，是否验证了 UI 影响？
- [ ] **Mock 控制接口**：是否使用 `window.__ECHOMIND_MOCK_CONTROL__` 控制 mock 状态？（而非 ad-hoc evaluate 注入）

### 1.4 防御性

- [ ] **前端守卫**：操作是否通过 `guards.js` 守卫检查？
- [ ] **守卫失败反馈**：守卫失败时是否有用户可见反馈（toast/placeholder/disabled）？

---

## 二、CI 门禁规则

### 2.1 强制门禁

```yaml
# .github/workflows/e2e.yml 门禁规则
 gates:
   - name: "cargo clippy"
     command: "cargo clippy --all-targets -- -D warnings"
     must_pass: true
   
   - name: "cargo fmt"
     command: "cargo fmt --check"
     must_pass: true
   
   - name: "cargo test"
     command: "cargo test"
     must_pass: true
   
   - name: "E2E sidebar-behavior"
     command: "npx playwright test tests/sidebar-behavior.spec.ts"
     must_pass: true
   
   - name: "E2E chat"
     command: "npx playwright test tests/chat.spec.ts"
     must_pass: true
```

### 2.2 可选门禁（手动触发）

```yaml
   - name: "真实数据集成测试"
     command: "ECHOMIND_E2E_REAL_DATA=1 npx playwright test tests/real-data.spec.ts"
     must_pass: false  # 需要真实 API Key + Tauri dev 运行
     trigger: "manual"
```

---

## 三、测试文件命名规范

```
e2e-tests/tests/{module}.spec.ts
```

| 模块前缀 | 描述 | 示例 |
|---------|------|------|
| `E2E-SB-` | 侧栏行为 (Sidebar Behavior) | E2E-SB-001 |
| `E2E-RAG-` | RAG 对话链路 | E2E-RAG-001 |
| `E2E-ING-` | 文档导入 | E2E-ING-001 |
| `E2E-SEC-` | 安全加密 | E2E-SEC-001 |
| `E2E-UI-` | UI 交互 | E2E-UI-007 |
| `RD-` | 真实数据集成 | RD-001 |

---

## 四、Mock 分层策略

| 层级 | 描述 | 适用场景 | 文件 |
|------|------|---------|------|
| L1 单元 | 纯函数 mock，无 DOM | 工具函数测试 | `e2e-tests/tests/unit/*.test.js` |
| L2 E2E Mock | tauri-stub 全量 mock | UI 流程测试 | `e2e-tests/tests/bridge/tauri-stub.js` |
| L3 真实数据 | 真实 Tauri + 真实文件 + 真实 embedder | RAG 核心验证 | `e2e-tests/tests/real-data.spec.ts` |
| L4 真实 LLM | 真实 Tauri + 真实 LLM API | 端到端验证 | `e2e-tests/tests/real-llm.spec.ts` |

### Mock 控制接口

测试通过 `window.__ECHOMIND_MOCK_CONTROL__` 控制 L2 mock 状态：

```typescript
// 设置空文档列表（测试空状态）
window.__ECHOMIND_MOCK_CONTROL__.clearDocs();

// 添加文档（测试有文档状态）
window.__ECHOMIND_MOCK_CONTROL__.addDoc('test.md', 'Indexed');

// 设置 chat 错误（测试错误恢复）
window.__ECHOMIND_MOCK_CONTROL__.setChatError('EMBED: timeout');

// 重置所有 mock 状态
window.__ECHOMIND_MOCK_CONTROL__.reset();
```
