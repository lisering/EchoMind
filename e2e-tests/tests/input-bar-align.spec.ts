// E2E 输入框视觉对齐验收（REQ-UI-003，DeepSeek 风格纵向布局）。
// 布局：textarea 在上（初始高度 48px），工具栏行在下（Toggle 左、按钮右）。
// 验证点：
// E2E-ALIGN-001: textarea 初始高度充足（≥44px，两行输入空间）
// E2E-ALIGN-002: textarea 与工具栏垂直分离；工具栏内按钮两两底部对齐
// E2E-ALIGN-003: 工具栏按钮中心 Y 一致
// E2E-ALIGN-004: textarea 初始高度充足（min-height 48px）
// E2E-ALIGN-005: plusBtn 与 sendBtn 高度一致
// E2E-ALIGN-006: 工具栏按钮两两底部对齐（同一行）
// E2E-ALIGN-007: 多行输入时 textarea 增高，按钮保持在工具栏行
// E2E-ALIGN-008: 空闲态按钮对齐不变
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';
/** 获取元素的 boundingBox（位置 + 尺寸）。 */
async function box(page, selector) {
  return page.locator(selector).boundingBox();
}

test.describe('E2E-ALIGN-001~007 输入框视觉对齐', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  test('E2E-ALIGN-001 textarea 初始高度充足（≥44px）', async ({ page }) => {
    const textarea = await box(page, '#queryInput');
    // 初始高度 48px（两行 leading-6），Shift+Enter 换行体验良好
    expect(textarea!.height, `textarea 高度 ${textarea!.height}px 应 ≥ 44px`)
      .toBeGreaterThanOrEqual(44);
  });

  test('E2E-ALIGN-002 textarea 与工具栏垂直分离，工具栏按钮底部对齐', async ({ page }) => {
    const textarea = await box(page, '#queryInput');
    const sendBtn = await box(page, '#sendBtn');
    const plusBtn = await box(page, '#plusBtn');

    const textareaBottom = textarea!.y + textarea!.height;
    // textarea 在工具栏上方（垂直分离，不同行）
    expect(textareaBottom, 'textarea 底部应高于按钮顶部（不同行）')
      .toBeLessThanOrEqual(sendBtn!.y);

    // 工具栏内按钮两两底部对齐（±2px 容差）
    const btnBottom = sendBtn!.y + sendBtn!.height;
    const plusBottom = plusBtn!.y + plusBtn!.height;
    expect(Math.abs(btnBottom - plusBottom),
      `sendBtn 底部 ${btnBottom}px 应≈plusBtn 底部 ${plusBottom}px`).toBeLessThanOrEqual(2);
  });

  test('E2E-ALIGN-003 工具栏按钮中心 Y 一致', async ({ page }) => {
    const alignment = await page.evaluate(() => {
      const sendBtn = document.getElementById('sendBtn');
      const plusBtn = document.getElementById('plusBtn');
      if (!sendBtn || !plusBtn) return null;
      const btnRect = sendBtn.getBoundingClientRect();
      const plusRect = plusBtn.getBoundingClientRect();
      return {
        sendBtnCenterY: btnRect.y + btnRect.height / 2,
        plusBtnCenterY: plusRect.y + plusRect.height / 2,
      };
    });

    expect(alignment, '应能获取元素位置').not.toBeNull();
    expect(Math.abs(alignment!.sendBtnCenterY - alignment!.plusBtnCenterY),
      `sendBtn 中心Y ${alignment!.sendBtnCenterY} 应≈plusBtn 中心Y ${alignment!.plusBtnCenterY}`)
      .toBeLessThanOrEqual(2);
  });

  test('E2E-ALIGN-004 textarea min-height 48px（两行空间）', async ({ page }) => {
    const minHeight = await page.evaluate(() => {
      const ta = document.getElementById('queryInput');
      if (!ta) return null;
      return parseFloat(getComputedStyle(ta).minHeight);
    });
    expect(minHeight, '应能获取 textarea min-height').not.toBeNull();
    expect(minHeight!, `textarea min-height ${minHeight}px 应 ≥ 44px`).toBeGreaterThanOrEqual(44);
  });

  test('E2E-ALIGN-005 plusBtn 与 sendBtn 高度一致', async ({ page }) => {
    const plusBtn = await box(page, '#plusBtn');
    const sendBtn = await box(page, '#sendBtn');

    expect(Math.abs(plusBtn!.height - sendBtn!.height),
      `plusBtn ${plusBtn!.height}px 应≈sendBtn ${sendBtn!.height}px`).toBeLessThanOrEqual(1);
  });

  test('E2E-ALIGN-006 工具栏行内按钮底部对齐', async ({ page }) => {
    const layout = await page.evaluate(() => {
      const bar = document.getElementById('inputBar');
      if (!bar) return null;
      // 工具栏行 = inputBar 的最后一个子元素（文本区在上，工具栏行在下）
      const toolbar = bar.lastElementChild;
      if (!toolbar) return null;
      const buttons = Array.from(toolbar.querySelectorAll('button')).filter((c) => {
        return c.getBoundingClientRect().width > 0;
      });
      return buttons.map((c) => {
        const r = c.getBoundingClientRect();
        return { id: c.id, bottom: r.y + r.height };
      });
    });

    expect(layout, '应能获取工具栏按钮').not.toBeNull();
    expect(layout!.length, '应有可见按钮').toBeGreaterThanOrEqual(2);
    const bottoms = layout!.map((l) => l.bottom);
    const bottomSpread = Math.max(...bottoms) - Math.min(...bottoms);
    expect(bottomSpread, `工具栏按钮底部坐标差应 ≤ 3px`).toBeLessThanOrEqual(3);
  });

  test('E2E-ALIGN-007 多行输入时 textarea 增高，按钮保持在工具栏行', async ({ page }) => {
    // RC1 修复：空 KB 时 queryInput 被禁用，需先导入文档
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() => window.__TAURI__.core.invoke('import_files', { paths: ['/mock/test.md'] }));
    await page.locator('#docList [data-doc-name]').first().waitFor({ timeout: 5000 });
    await page.locator('#kbCloseBtn').click();

    const initialHeight = (await box(page, '#queryInput'))!.height;

    // 输入多行文本触发 textarea 自动增高
    await page.locator('#queryInput').fill('第一行\n第二行\n第三行\n第四行');
    await page.waitForTimeout(200); // 等待 auto-resize

    const textarea = await box(page, '#queryInput');
    const sendBtn = await box(page, '#sendBtn');
    const plusBtn = await box(page, '#plusBtn');

    // textarea 应增高
    expect(textarea!.height, '多行时 textarea 应增高').toBeGreaterThan(initialHeight);

    // 按钮仍保持在工具栏行（在 textarea 下方），且两两底部对齐
    expect(textarea!.y + textarea!.height, '多行后 textarea 底部仍应高于按钮')
      .toBeLessThanOrEqual(sendBtn!.y);
    expect(Math.abs((sendBtn!.y + sendBtn!.height) - (plusBtn!.y + plusBtn!.height)),
      '多行时 sendBtn 与 plusBtn 底部应对齐').toBeLessThanOrEqual(2);
  });

  test('E2E-ALIGN-008 空闲态按钮对齐不变', async ({ page }) => {
    // 空闲态（非流式）：sendBtn 可见且非停止形态（发送/停止合二为一）
    await expect(page.locator('#sendBtn')).toBeVisible();
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/);

    // 按钮两两底部对齐
    const sendBtn = await box(page, '#sendBtn');
    const plusBtn = await box(page, '#plusBtn');
    expect(Math.abs((sendBtn!.y + sendBtn!.height) - (plusBtn!.y + plusBtn!.height)),
      `空闲态 sendBtn/plusBtn 底部应对齐：${sendBtn!.y + sendBtn!.height} vs ${plusBtn!.y + plusBtn!.height}`)
      .toBeLessThanOrEqual(2);
  });
});

// ============================================================
// REQ-IX-006 输入框多行与自适应高度 — E2E 验收
// AC-1: Shift+Enter 插入换行不触发发送
// AC-2: Enter 发送消息（消息出现在对话区）
// AC-3: 输入多行后高度增长至 max-h-40（160px）
// AC-4: 发送后高度重置为初始值
// ============================================================

test.describe('REQ-IX-006 输入框多行与自适应高度', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  test('REQ-IX-006-AC1 Shift+Enter 插入换行不触发发送', async ({ page }) => {
    // RC1 修复：空 KB 时 queryInput 被禁用，需先导入文档
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() => window.__TAURI__.core.invoke('import_files', { paths: ['/mock/test.md'] }));
    await page.locator('#docList [data-doc-name]').first().waitFor({ timeout: 5000 });
    await page.locator('#kbCloseBtn').click();

    const textarea = page.locator('#queryInput');
    await textarea.focus();
    await textarea.type('第一行');
    // Shift+Enter 应插入换行而非发送
    await page.keyboard.press('Shift+Enter');
    await textarea.type('第二行');

    const value = await textarea.inputValue();
    // textarea 值应包含换行符
    expect(value).toContain('\n');
    expect(value).toContain('第一行');
    expect(value).toContain('第二行');

    // 不应有用户消息出现在聊天区（未发送）
    const userBlocks = page.locator('#chatArea [class*="justify-end"]');
    expect(await userBlocks.count(), 'Shift+Enter 不应触发发送').toBe(0);
  });

  test('REQ-IX-006-AC2 Enter 发送消息', async ({ page }) => {
    // 先导入文档以满足聊天前置条件
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/rust-guide.md'] })
    );
    await page.waitForTimeout(300);

    const textarea = page.locator('#queryInput');
    await textarea.fill('测试 Enter 发送');
    // Enter 发送
    await page.keyboard.press('Enter');

    // 应出现用户消息（justify-end 右对齐 = 用户消息）
    await page.waitForTimeout(500);
    const userBlocks = page.locator('#chatArea [class*="justify-end"]');
    expect(await userBlocks.count(), 'Enter 应发送消息').toBeGreaterThan(0);
  });

  test('REQ-IX-006-AC3 输入多行后高度增长至 max-h-40', async ({ page }) => {
    const textarea = page.locator('#queryInput');
    await textarea.focus();

    // 记录初始高度
    const initialHeight = (await textarea.boundingBox())!.height;

    // 直接设置多行文本并触发 input 事件以触发 auto-resize
    // （headless 浏览器中 Shift+Enter 不一定能可靠触发 input 事件链）
    const lines = [];
    for (let i = 0; i < 8; i++) {
      lines.push(`第${i + 1}行测试文本内容`);
    }
    await page.evaluate((text) => {
      const ta = document.getElementById('queryInput');
      if (!ta) return;
      ta.value = text;
      ta.dispatchEvent(new Event('input', { bubbles: true }));
    }, lines.join('\n'));
    await page.waitForTimeout(300); // 等待 auto-resize

    const multiHeight = (await textarea.boundingBox())!.height;

    // 高度应增长
    expect(multiHeight, '多行输入后高度应增长').toBeGreaterThan(initialHeight);

    // 高度不应超过 max-h-40（160px）+ 容差
    // max-h-40 = 10rem = 160px（在 Tailwind CSS 中）
    expect(multiHeight, '高度应受 max-h-40 限制（≤165px）').toBeLessThanOrEqual(165);
  });

  test('REQ-IX-006-AC4 发送后高度重置为初始值', async ({ page }) => {
    // 先导入文档
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/rust-guide.md'] })
    );
    // V3.1 P2-2：doc-status-changed 合流刷新（500ms debounce）——等发送守卫放行
    await page.waitForTimeout(800);

    const textarea = page.locator('#queryInput');
    await textarea.focus();

    // 记录初始高度
    const initialHeight = (await textarea.boundingBox())!.height;

    // 直接设置多行文本并触发 input 事件以触发 auto-resize
    const lines = [];
    for (let i = 0; i < 6; i++) {
      lines.push(`行${i + 1}内容文本`);
    }
    await page.evaluate((text) => {
      const ta = document.getElementById('queryInput');
      if (!ta) return;
      ta.value = text;
      ta.dispatchEvent(new Event('input', { bubbles: true }));
    }, lines.join('\n'));
    await page.waitForTimeout(200);

    const expandedHeight = (await textarea.boundingBox())!.height;
    expect(expandedHeight, '多行输入后高度应增长').toBeGreaterThan(initialHeight);

    // Enter 发送
    await page.keyboard.press('Enter');
    await page.waitForTimeout(500);

    // 高度应重置
    const resetHeight = (await textarea.boundingBox())!.height;
    // 重置后高度应接近初始值（±3px 容差，因为 auto 计算）
    expect(Math.abs(resetHeight - initialHeight),
      `发送后高度 ${resetHeight}px 应≈初始高度 ${initialHeight}px`).toBeLessThanOrEqual(3);

    // textarea 值应被清空
    const value = await textarea.inputValue();
    expect(value, '发送后输入框应清空').toBe('');
  });
});
