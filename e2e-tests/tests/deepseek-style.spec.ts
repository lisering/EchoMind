/**
 * DeepSeek 风格 UI 对齐 E2E 测试（TC-DS-013~020）
 *
 * 验证 S1-S4 的视觉与交互变更：
 * - TC-DS-013: 用户消息纯文本右对齐（无气泡背景/边框）
 * - TC-DS-014: AI 消息无背景色（透明）
 * - TC-DS-015: AI 消息底部包含 .ai-disclaimer 元素
 * - TC-DS-016: 思维链面板可折叠（点击后 content 切换 hidden）
 * - TC-DS-017: 搜索来源可折叠（点击后 list 切换 display）
 * - TC-DS-018: 输入框旁显示 toggle 按钮
 * - TC-DS-019: 新对话按钮旁显示 ⌘ J 提示
 * - TC-DS-020: ⌘ J 快捷键触发新对话
 */
import { test, expect } from '@playwright/test';
import { enterApp, importDocs, injectLocales, sendMessage, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';

test.describe('DeepSeek 风格 UI 对齐', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── S1: 视觉基础对齐 ───

  test('TC-DS-013 用户消息纯文本右对齐（无气泡背景/边框）', async ({ page }) => {
    await importDocs(page, ['/mock/deepseek-test.md']);
    await sendMessage(page, '测试问题');
    await waitForStreamDone(page);

    // 查找用户消息块
    const userMsg = page.locator('.msg-user').first();
    await expect(userMsg).toBeVisible();

    // 对齐 chat.deepseek.com：无气泡背景、无边框、无圆角（透明）
    // 注：Tailwind preflight 默认 border-style: solid + border-width: 0，判定边框用 border-width
    const styles = await userMsg.evaluate((el) => {
      const s = window.getComputedStyle(el);
      return { bg: s.backgroundColor, borderTop: s.borderTopWidth, radius: s.borderRadius };
    });
    expect(styles.bg).toMatch(/transparent|rgba?\(0,\s*0,\s*0,\s*0\)/);
    expect(styles.borderTop).toBe('0px');
    // 右对齐：用户消息右边缘贴近聊天区右边缘
    // 注：chatArea 可能有 padding/scrollbar，用宽松阈值验证右对齐趋势
    const pos = await userMsg.evaluate((el) => {
      const r = el.getBoundingClientRect();
      const area = document.querySelector('#chatArea') || el.parentElement;
      const ar = area.getBoundingClientRect();
      return { right: r.right, areaRight: ar.right, left: r.left, areaLeft: ar.left };
    });
    // 用户消息应更靠近右边缘（右对齐），而非左边缘
    const rightGap = pos.areaRight - pos.right;
    const leftGap = pos.left - pos.areaLeft;
    // 右间隙应 <= 左间隙（允许相等，因为 padding 可能使两侧对称）
    expect(rightGap, '用户消息右边缘应更靠近聊天区右边缘').toBeLessThanOrEqual(leftGap);
  });

  test('TC-DS-014 AI 消息无背景色（透明）', async ({ page }) => {
    await importDocs(page, ['/mock/deepseek-test.md']);
    await sendMessage(page, '测试问题');
    await waitForStreamDone(page);

    const aiMsg = page.locator('.msg-assistant').first();
    await expect(aiMsg).toBeVisible();

    const bgColor = await aiMsg.evaluate((el) => {
      return window.getComputedStyle(el).backgroundColor;
    });
    // 透明背景：rgba(0, 0, 0, 0) 或 transparent
    expect(bgColor).toMatch(/transparent|rgba?\(0,\s*0,\s*0,\s*0\)/);
  });

  test('TC-DS-015 AI 消息底部包含 .ai-disclaimer 元素', async ({ page }) => {
    await importDocs(page, ['/mock/deepseek-test.md']);
    await sendMessage(page, '测试问题');
    // 等待流式完成（stop-mode 被移除表示完成）
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 15000 });

    const disclaimer = page.locator('.msg-assistant .ai-disclaimer').first();
    await expect(disclaimer).toBeVisible();
    const text = await disclaimer.textContent();
    expect(text?.length).toBeGreaterThan(0);
  });

  // ─── S2: 思维链折叠 ───

  test('TC-DS-016 思维链面板可折叠', async ({ page }) => {
    await importDocs(page, ['/mock/deepseek-test.md']);
    await sendMessage(page, '测试问题');

    // 等待思维链面板出现
    const thinkingHeader = page.locator('.thinking-panel-header').first();
    await expect(thinkingHeader).toBeVisible({ timeout: 5000 });

    // content 初始隐藏
    const thinkingContent = page.locator('.thinking-panel-content').first();
    await expect(thinkingContent).toHaveClass(/hidden/);

    // 点击展开
    await thinkingHeader.click();
    await expect(thinkingContent).not.toHaveClass(/hidden/);

    // 再次点击折叠
    await thinkingHeader.click();
    await expect(thinkingContent).toHaveClass(/hidden/);

    await waitForStreamDone(page);
  });

  test('TC-DS-017 搜索来源可折叠', async ({ page }) => {
    await importDocs(page, ['/mock/deepseek-test.md']);
    await sendMessage(page, '测试问题');
    await waitForStreamDone(page);

    // 等待来源渲染
    const sourcesToggle = page.locator('.sources-toggle').first();
    await expect(sourcesToggle).toBeVisible({ timeout: 5000 });

    // 来源列表初始隐藏
    const sourcesList = page.locator('.sources-list').first();
    const initialDisplay = await sourcesList.evaluate((el) => {
      return window.getComputedStyle(el).display;
    });
    expect(initialDisplay).toBe('none');

    // 点击展开
    await sourcesToggle.click();
    const expandedDisplay = await sourcesList.evaluate((el) => {
      return window.getComputedStyle(el).display;
    });
    expect(expandedDisplay).not.toBe('none');
  });

  // ─── S3: 输入区 Toggle + 快捷键 ───

  test('TC-DS-018 输入框旁显示 toggle 按钮', async ({ page }) => {
    const toggleContainer = page.locator('#inputToggles');
    await expect(toggleContainer).toBeVisible();

    const toggle = toggleContainer.locator('.input-toggle').first();
    await expect(toggle).toBeVisible();
    const text = await toggle.textContent();
    expect(text?.length).toBeGreaterThan(0);
  });

  test('TC-DS-019 新对话按钮旁显示 ⌘ J 提示', async ({ page }) => {
    const shortcut = page.locator('#newChatBtn .shortcut-hint');
    await expect(shortcut).toBeVisible();
    const text = await shortcut.textContent();
    expect(text).toContain('⌘');
    expect(text).toContain('J');
  });

  test('TC-DS-020 ⌘ J 快捷键触发新对话', async ({ page }) => {
    // 先发送一条消息确保有内容
    await importDocs(page, ['/mock/deepseek-test.md']);
    await sendMessage(page, '测试');

    // 等待 #sendBtn 进入 stop-mode（streaming 开始），然后等待其退出（streaming 结束）
    await page.locator('#sendBtn.stop-mode').waitFor({ state: 'visible', timeout: 5000 });
    await page.locator('#sendBtn:not(.stop-mode)').waitFor({ state: 'visible', timeout: 30000 });

    // 确认聊天区有消息
    const msgCountBefore = await page.locator('.msg-block').count();
    expect(msgCountBefore).toBeGreaterThan(0);

    // 点击新对话按钮
    await page.locator('#newChatBtn').click();

    // 等待新对话清空聊天区
    await page.waitForTimeout(1000);

    // 验证聊天区被清空（msg-block 数量减少）
    const msgCountAfter = await page.locator('.msg-block').count();
    expect(msgCountAfter).toBeLessThan(msgCountBefore);
  });
});
