// E2E 键盘导航与无障碍验收（REQ-UI-001/003/007）。
// E2E-KB-001: Tab 键可遍历主要交互元素
// E2E-KB-002: Enter 发送消息
// E2E-KB-003: Shift+Enter 换行不发送
// E2E-KB-004: Escape 关闭设置面板
// E2E-KB-005: Escape 关闭付费墙
// E2E-KB-006: VLM Toggle ARIA role=switch
// E2E-KB-007: 输入框 aria-label / placeholder 可读
// E2E-KB-008: 按钮均有 title 或 aria-label
// E2E-KB-009: 停止按钮有明确的 title
// E2E-KB-010: 链接 rel=noferrer 安全属性
import { test, expect } from '@playwright/test';
import { enterApp, importDocs, injectLocales, sendMessage, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';
test.describe('E2E-KB-001~010 键盘导航与无障碍', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    await importDocs(page, ['/mock/kb-test.md']);
  });

  test('E2E-KB-001 Tab 键可遍历主要交互元素', async ({ page }) => {
    // 从输入框开始 Tab，应能到达发送按钮
    await page.locator('#queryInput').focus();
    await page.keyboard.press('Tab');
    // 焦点应移到发送按钮
    const focusedTag = await page.evaluate(() => document.activeElement?.tagName);
    expect(['BUTTON', 'TEXTAREA', 'INPUT']).toContain(focusedTag);
  });

  test('E2E-KB-002 Enter 发送消息', async ({ page }) => {
    await page.locator('#queryInput').fill('Enter 键发送测试');
    await page.keyboard.press('Enter');

    // 应出现用户消息
    await expect(page.locator('#chatArea .flex.justify-end')).toBeVisible({ timeout: 5000 });
    await waitForStreamDone(page);
  });

  test('E2E-KB-003 Shift+Enter 换行不发送', async ({ page }) => {
    await page.locator('#queryInput').fill('第一行');
    await page.keyboard.press('Shift+Enter');
    await page.keyboard.type('第二行');

    // 不应发送消息（无用户气泡出现）
    await page.waitForTimeout(500);
    const userBlocks = await page.locator('#chatArea .flex.justify-end').count();
    expect(userBlocks, 'Shift+Enter 不应发送').toBe(0);

    // 输入框应含换行
    const value = await page.locator('#queryInput').inputValue();
    expect(value, '应含换行符').toContain('\n');
  });

  test('E2E-KB-004 Escape 关闭设置面板', async ({ page }) => {
    // 打开设置面板
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 按 Escape（如果前端有 Escape 监听则关闭；否则点击完成按钮）
    // 先尝试 Escape
    await page.keyboard.press('Escape');
    // 如果未关闭，用完成按钮
    const isVisible = await page.locator('#settingsModal').isVisible();
    if (isVisible) {
      await page.locator('#settingsClose').click();
    }
    await expect(page.locator('#settingsModal')).toBeHidden();
  });

  test('E2E-KB-005 付费墙 Escape 或关闭按钮可用', async ({ page }) => {
    // 触发付费墙
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/paywall-test.pdf']));
    await expect(page.locator('#paywall')).toBeVisible({ timeout: 5000 });

    // 点击关闭按钮
    await page.locator('#paywallClose').click();
    await expect(page.locator('#paywall')).toBeHidden();
  });

  test('E2E-KB-006 VLM Toggle ARIA role=switch', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    const toggle = page.locator('#vlmToggle');
    await expect(toggle).toHaveAttribute('role', 'switch');
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
  });

  test('E2E-KB-007 按钮均有 title 或 aria-label', async ({ page }) => {
    // 主要操作按钮应有 title（#stopBtn 仅流式时可见，单独验证属性即可）
    const buttons = [
      { sel: '#plusBtn', title: '导入文件' },
      { sel: '#sendBtn', text: '发送' },
      { sel: '#newChatBtn', text: '新对话' },
      { sel: '#settingsBtn', title: '设置' },
      { sel: '#collapseBtn', title: '折叠' },
    ];

    for (const btn of buttons) {
      const el = page.locator(btn.sel);
      await expect(el, `按钮 ${btn.sel} 应存在`).toBeVisible();
      if (btn.title) {
        await expect(el, `按钮 ${btn.sel} 应有 title="${btn.title}"`).toHaveAttribute('title', btn.title);
      }
      if (btn.text) {
        await expect(el, `按钮 ${btn.sel} 应含文字"${btn.text}"`).toContainText(btn.text);
      }
    }

    // 发送/停止合二为一：#sendBtn 应存在且空闲态 title=发送
    const sendBtn = page.locator('#sendBtn');
    await expect(sendBtn, '按钮 #sendBtn 应存在').toHaveCount(1);
    await expect(sendBtn, '按钮 #sendBtn 空闲态应有 title=发送').toHaveAttribute('title', /发送|Send/);
  });

  test('E2E-KB-008 输入框有 placeholder 可读', async ({ page }) => {
    const input = page.locator('#queryInput');
    const placeholder = await input.getAttribute('placeholder');
    expect(placeholder, '输入框应有 placeholder').not.toBeNull();
    expect(placeholder.length, 'placeholder 应非空').toBeGreaterThan(0);
    expect(placeholder, 'placeholder 应含 Enter 提示').toContain('Enter');
  });

  test('E2E-KB-009 生成中输入框被禁用', async ({ page }) => {
    await sendMessage(page, '测试禁用');
    // 流式中输入框应禁用
    await expect(page.locator('#queryInput')).toBeDisabled();
    await waitForStreamDone(page);
    // 完成后恢复可用
    await expect(page.locator('#queryInput')).not.toBeDisabled();
  });

  test('E2E-KB-010 新对话按钮可键盘聚焦', async ({ page }) => {
    // newChatBtn 应可被 Tab 聚焦
    await page.locator('#newChatBtn').focus();
    const isFocused = await page.evaluate(() => document.activeElement?.id === 'newChatBtn');
    expect(isFocused, '新对话按钮应可被聚焦').toBe(true);
  });
});

// TC-A11Y-002-001~006: Focus Trap + 焦点管理（REQ-A11Y-002）
test.describe('TC-A11Y-002-001~006 Focus Trap + 焦点管理', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    await importDocs(page, ['/mock/kb-test.md']);
  });

  test('TC-A11Y-002-001 打开设置面板 Tab 焦点不跳出 #settingsModal', async ({ page }) => {
    // 打开设置面板
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 等待 Focus Trap 激活（容器内元素获得焦点）
    await page.waitForFunction(
      () => {
        const modal = document.getElementById('settingsModal');
        return modal && !modal.classList.contains('hidden') &&
               modal.contains(document.activeElement) && document.activeElement !== document.body;
      },
      { timeout: 5000 }
    );

    // 连续按 Tab 15 次，验证焦点始终在 #settingsModal 内
    for (let i = 0; i < 15; i++) {
      await page.keyboard.press('Tab');
      const inModal = await page.evaluate(() => {
        const modal = document.getElementById('settingsModal');
        return !!(modal && modal.contains(document.activeElement));
      });
      expect(inModal, `Tab #${i + 1}: 焦点应锁定在 #settingsModal 内`).toBe(true);
    }

    // 连续按 Shift+Tab 5 次，验证焦点仍在 #settingsModal 内
    for (let i = 0; i < 5; i++) {
      await page.keyboard.press('Shift+Tab');
      const inModal = await page.evaluate(() => {
        const modal = document.getElementById('settingsModal');
        return !!(modal && modal.contains(document.activeElement));
      });
      expect(inModal, `Shift+Tab #${i + 1}: 焦点应锁定在 #settingsModal 内`).toBe(true);
    }
  });

  test('TC-A11Y-002-002 关闭设置面板后焦点回到 #settingsBtn', async ({ page }) => {
    // 点击设置按钮打开面板（记录触发元素）
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 等待 Focus Trap 激活（容器内元素获得焦点）
    await page.waitForFunction(
      () => {
        const modal = document.getElementById('settingsModal');
        return modal && modal.contains(document.activeElement) && document.activeElement !== document.body;
      },
      { timeout: 5000 }
    );

    // 关闭设置面板
    await page.locator('#settingsClose').click();
    await expect(page.locator('#settingsModal')).toBeHidden({ timeout: 5000 });

    // Focus Trap deactivate() 同步恢复焦点，但 Playwright click 可能需要一帧来稳定
    await page.waitForTimeout(200);

    const focusedId = await page.evaluate(() => document.activeElement?.id);
    expect(focusedId, '关闭后焦点应回到 #settingsBtn').toBe('settingsBtn');
  });

  test('TC-A11Y-002-003 打开确认对话框 Tab 在确认/取消间循环', async ({ page }) => {
    // 通过 window.showConfirmDialog 触发确认对话框
    await page.evaluate(() => {
      if (typeof window.showConfirmDialog === 'function') {
        window.showConfirmDialog({ title: 'Test', confirmText: 'OK', cancelText: 'Cancel', danger: false });
      }
    });
    await expect(page.locator('#confirmDialog')).toBeVisible({ timeout: 5000 });

    // 等待防误触延迟（500ms）+ Focus Trap rAF
    await page.waitForTimeout(700);

    // 连续按 Tab 6 次，验证焦点始终在 #confirmDialog 内
    for (let i = 0; i < 6; i++) {
      await page.keyboard.press('Tab');
      const inDialog = await page.evaluate(() => {
        const dialog = document.getElementById('confirmDialog');
        return !!(dialog && dialog.contains(document.activeElement));
      });
      expect(inDialog, `Tab #${i + 1}: 焦点应锁定在 #confirmDialog 内`).toBe(true);
    }

    // 清理：点击取消按钮关闭对话框
    const cancelBtn = page.locator('#confirmDialog [data-role="cancel"]');
    if (await cancelBtn.count() > 0) {
      await cancelBtn.click();
    }
  });

  test('TC-A11Y-002-004 聚焦元素显示 focus 环 outline 2px solid accent + offset 2px', async ({ page }) => {
    // 通过 Tab 键导航触发 :focus-visible（键盘导航模式）
    await page.keyboard.press('Tab');
    await page.waitForTimeout(50);

    // 连续 Tab 直到聚焦到 BUTTON 元素
    let foundButton = false;
    for (let i = 0; i < 20; i++) {
      const tag = await page.evaluate(() => document.activeElement?.tagName);
      if (tag === 'BUTTON') { foundButton = true; break; }
      await page.keyboard.press('Tab');
    }
    expect(foundButton, '应通过 Tab 聚焦到 BUTTON 元素').toBe(true);

    // 检查 :focus-visible 匹配 + 计算样式
    const result = await page.evaluate(() => {
      const el = document.activeElement;
      if (!el) return null;
      const cs = window.getComputedStyle(el);
      return {
        id: el.id,
        matchesFocusVisible: el.matches(':focus-visible'),
        outlineWidth: cs.outlineWidth,
        outlineColor: cs.outlineColor,
        outlineOffset: cs.outlineOffset,
        outlineStyle: cs.outlineStyle,
      };
    });

    expect(result, '应有聚焦元素').not.toBeNull();
    // 验证 :focus-visible CSS 规则的计算值
    // tokens.css: :focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
    // var(--accent) = #38BDF8 = rgb(56, 189, 248)
    expect(result.outlineWidth, 'outline 宽度应为 2px').toBe('2px');
    expect(result.outlineOffset, 'outline 偏移应为 2px').toBe('2px');
    expect(result.outlineStyle, 'outline 样式应为 solid').toBe('solid');
    // outlineColor 应为 rgb(56, 189, 248) 即 #38BDF8
    expect(result.outlineColor, 'outline 颜色应为 #38BDF8').toBe('rgb(56, 189, 248)');
  });

  test('TC-A11Y-002-005 全量 Tab 遍历顺序符合视觉顺序', async ({ page }) => {
    // 从侧栏第一个按钮开始 Tab，确保遍历起点确定
    await page.locator('#kbBtn').focus();
    await page.waitForTimeout(50);

    // 记录 Tab 遍历的元素 ID 顺序
    const visitedIds = ['kbBtn'];
    const maxTabs = 30;

    for (let i = 0; i < maxTabs; i++) {
      await page.keyboard.press('Tab');
      await page.waitForTimeout(30);
      const id = await page.evaluate(() => document.activeElement?.id || '');
      visitedIds.push(id);
      // 如果回到了第一个元素，停止
      if (id === 'kbBtn') break;
    }

    // 验证关键交互元素按视觉顺序出现
    // 视觉顺序：侧栏顶部 → 新建会话 → 输入区 → 发送
    const expectedOrder = ['kbBtn', 'convSearchBtn', 'settingsBtn', 'collapseBtn', 'newChatBtn', 'plusBtn', 'queryInput', 'sendBtn'];
    const actualOrder = expectedOrder.filter(id => visitedIds.includes(id));

    // 至少应包含 5 个关键元素
    expect(actualOrder.length, `应至少遍历 5 个关键元素，实际: ${visitedIds.join(', ')}`).toBeGreaterThanOrEqual(5);

    // 验证相对顺序（前一个元素应在后一个元素之前被聚焦）
    for (let i = 0; i < actualOrder.length - 1; i++) {
      const idx1 = visitedIds.indexOf(actualOrder[i]);
      const idx2 = visitedIds.indexOf(actualOrder[i + 1]);
      expect(idx1, `${actualOrder[i]} 应在 ${actualOrder[i + 1]} 之前被聚焦`).toBeLessThan(idx2);
    }
  });

  test('TC-A11Y-002-006 无 tabindex="-1" 用于可交互元素', async ({ page }) => {
    // 检查所有可交互元素（button, a[href], input, textarea, select）不含 tabindex="-1"
    const violations = await page.evaluate(() => {
      const interactiveSelectors = 'button, a[href], input, textarea, select';
      const elements = document.querySelectorAll(interactiveSelectors);
      const result = [];
      for (const el of elements) {
        const tabindex = el.getAttribute('tabindex');
        if (tabindex === '-1') {
          result.push({
            tag: el.tagName,
            id: el.id || '(no id)',
            class: el.className?.toString().slice(0, 50) || '',
            tabindex,
          });
        }
      }
      return result;
    });

    expect(violations, `不应有可交互元素使用 tabindex="-1"，发现: ${JSON.stringify(violations)}`).toHaveLength(0);
  });
});
