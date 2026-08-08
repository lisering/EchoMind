# 用户视角验收用例模板（Test Design Template）

> **目的**：消除根因 V1（实现驱动测试）和 V3（空状态覆盖缺失），确保每个功能需求从用户视角设计测试。

---

## 一、四路径测试设计法

每个功能需求必须覆盖以下四条测试路径：

### 路径 1：正向路径（Happy Path）
- **定义**：所有前置条件满足时的正常操作流程
- **验证目标**：核心功能按预期工作
- **示例**：有文档 → 输入问题 → 发送 → 收到回答

### 路径 2：空状态路径（Empty State Path）
- **定义**：前置条件不满足时的 UI 行为
- **验证目标**：用户看到引导而非错误
- **示例**：无文档 → 输入框禁用 + placeholder 提示"请先导入文档"

### 路径 3：状态转换路径（State Transition Path）
- **定义**：从前置条件不满足到满足的转换过程
- **验证目标**：状态变化后 UI 正确响应
- **示例**：无文档 → 导入文档 → 输入框恢复可用 + placeholder 恢复正常

### 路径 4：错误恢复路径（Error Recovery Path）
- **定义**：后端返回错误时前端的行为
- **验证目标**：错误有用户可见反馈，UI 恢复到可用状态
- **示例**：发送消息 → 后端返回错误 → toast 提示 → 输入框恢复 idle 态

---

## 二、测试设计 Checklist

编写新 E2E 测试时，必须回答以下问题：

### 2.1 需求验证

- [ ] 测试验证的是**用户需求**还是**当前实现**？
  - ❌ 错误示例：`expect(sidebar).toHaveClass(/sidebar-collapsed/)` — 验证 CSS 类名
  - ✅ 正确示例：`expect(sidebar.getBoundingClientRect().width).toBe(0)` — 验证用户可见效果
- [ ] 如果实现改变（如 48px → 0px），测试是否需要修改？
  - 如果需要，说明测试耦合了实现细节，应重构为验证用户可见行为

### 2.2 路径覆盖

- [ ] 正向路径：所有前置条件满足时，功能是否正常？
- [ ] 空状态路径：前置条件不满足时，UI 行为是否正确？
- [ ] 状态转换路径：前置条件从不满足变为满足时，UI 是否正确响应？
- [ ] 错误恢复路径：后端出错时，前端是否有可见反馈 + 恢复到可用状态？

### 2.3 真实性

- [ ] 测试是否使用 mock？如果是，mock 是否准确模拟了后端行为？
- [ ] mock 返回的空状态（如 `state.docs = []`）是否被测试验证了 UI 影响？
- [ ] 是否有至少一个测试使用真实数据（非 mock）验证核心流程？

### 2.4 防御性

- [ ] 前端是否有对应后端前置条件的 UX 防御（禁用/拦截）？
- [ ] 前端守卫函数是否有独立测试？
- [ ] 守卫失败时是否有用户可见反馈（toast/placeholder/disabled）？

---

## 三、E2E 测试命名规范

```
E2E-{MODULE}-{NUMBER} {用户视角描述}
```

### 示例

| ✅ 正确命名（用户视角） | ❌ 错误命名（实现视角） |
|------------------------|------------------------|
| 折叠后侧栏完全不可见 | sidebar-collapsed 类存在 |
| 空知识库时输入框禁用 | queryInput.disabled === true |
| 导入文档后输入框恢复可用 | docCount 状态从 0 变为 1 |
| 发送消息后流式回答显示 | chat_token 事件触发 |

---

## 四、测试代码模式

### 4.1 正向路径模式

```typescript
test('E2E-XXX-001 有文档时发送消息正常', async ({ page }) => {
  // 前置：确保有文档
  await page.evaluate(() => {
    window.__TAURI__.core.invoke('import_files', { paths: ['test.md'] });
  });
  await page.waitForTimeout(500);

  // 操作：输入并发送
  await page.locator('#queryInput').fill('什么是 Lisp？');
  await page.locator('#sendBtn').click();

  // 验证：回答显示
  await expect(page.locator('.message-in').last()).toBeVisible({ timeout: 10000 });
});
```

### 4.2 空状态路径模式

```typescript
test('E2E-XXX-002 空知识库时输入框和发送按钮禁用', async ({ page }) => {
  // 前置：确保无文档（tauri-stub 默认返回空）
  // 操作：检查 UI 状态
  const input = page.locator('#queryInput');
  await expect(input).toBeDisabled({ timeout: 10000 });
  await expect(page.locator('#sendBtn')).toBeDisabled();

  // 验证：placeholder 显示引导文案
  const placeholder = await input.getAttribute('placeholder');
  expect(placeholder).toBeTruthy();
  expect(placeholder!.length).toBeGreaterThan(0);
});
```

### 4.3 状态转换路径模式

```typescript
test('E2E-XXX-003 导入文档后输入框恢复可用', async ({ page }) => {
  // 前置：确认初始为禁用态
  await expect(page.locator('#queryInput')).toBeDisabled();

  // 操作：导入文档
  await page.evaluate(() => {
    window.__TAURI__.core.invoke('import_files', { paths: ['test.md'] });
  });
  await page.waitForTimeout(1000);

  // 验证：输入框恢复可用
  await expect(page.locator('#queryInput')).toBeEnabled({ timeout: 10000 });
  await expect(page.locator('#sendBtn')).toBeEnabled();
});
```

### 4.4 错误恢复路径模式

```typescript
test('E2E-XXX-004 后端报错时输入框进入错误态', async ({ page }) => {
  // 前置：注入 chat handler 返回错误
  await page.evaluate(() => {
    window.__ECHOMIND_MOCK_CONTROL__ = { chatError: 'EMBED: timeout' };
  });

  // 操作：发送消息
  await page.locator('#queryInput').fill('test');
  await page.locator('#sendBtn').click();

  // 验证：toast 显示错误 + 输入框恢复可用
  await expect(page.locator('.toast-error')).toBeVisible({ timeout: 5000 });
  await expect(page.locator('#queryInput')).toBeEnabled({ timeout: 10000 });
});
```
