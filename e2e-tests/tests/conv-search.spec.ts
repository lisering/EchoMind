/**
 * E2E 测试：对话全文搜索（REQ-RAG-040, S62）
 *
 * TC-RAG-SEARCH-005: 前端搜索 UI — 切换「对话」模式后显示搜索结果
 *
 * 验证搜索弹框中的模式切换按钮工作正常：
 * 1. 默认为「会话」模式
 * 2. 点击「对话」切换到内容搜索模式
 * 3. 输入关键词后调用 search_conversations IPC
 * 4. 搜索结果点击后跳转到对应会话
 */

import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';

test.describe('对话全文搜索（REQ-RAG-040）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  test('TC-RAG-SEARCH-005a 搜索模式切换按钮存在', async ({ page }) => {
    // 创建会话
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('create_conversation')
    );
    await page.waitForTimeout(200);

    // 打开搜索弹框（通过命令面板或搜索按钮）
    await page.evaluate(() => {
      const btn = document.querySelector('#convSearchBtn');
      if (btn) btn.click();
    });
    await page.waitForTimeout(300);

    // 检查搜索弹框可见
    const popup = page.locator('#convSearchPopup');
    await expect(popup).not.toHaveClass(/\bhidden\b/);

    // 检查模式切换按钮存在
    await expect(page.locator('#convSearchModeTitle')).toBeVisible();
    await expect(page.locator('#convSearchModeContent')).toBeVisible();

    // 默认应为 title 模式（title 按钮高亮）
    const titleBtn = page.locator('#convSearchModeTitle');
    const classAttr = await titleBtn.getAttribute('class');
    expect(classAttr).toContain('bg-accent');
  });

  test('TC-RAG-SEARCH-005b 切换到对话模式后更新 placeholder', async ({ page }) => {
    // 创建会话并打开搜索弹框
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('create_conversation')
    );
    await page.waitForTimeout(200);

    await page.evaluate(() => {
      const btn = document.querySelector('#convSearchBtn');
      if (btn) btn.click();
    });
    await page.waitForTimeout(300);

    // 记录初始 placeholder（会话模式）
    const input = page.locator('#convSearchPopupInput');
    const initialPlaceholder = await input.getAttribute('placeholder');

    // 点击「对话」模式
    await page.locator('#convSearchModeContent').click();
    await page.waitForTimeout(100);

    // placeholder 应变化
    const newPlaceholder = await input.getAttribute('placeholder');
    expect(newPlaceholder).not.toBe(initialPlaceholder);

    // 切回「会话」模式
    await page.locator('#convSearchModeTitle').click();
    await page.waitForTimeout(100);
    const restoredPlaceholder = await input.getAttribute('placeholder');
    expect(restoredPlaceholder).toBe(initialPlaceholder);
  });

  test('TC-RAG-SEARCH-005c 对话模式搜索返回结果', async ({ page }) => {
    // 注入 mock 消息数据
    await page.evaluate(() => {
      window.__mock.state.messages = [
        { id: 'msg-1', conversation_id: 'conv-1', role: 'user', content: '什么是 Rust 语言？', created_at: 1000 },
        { id: 'msg-2', conversation_id: 'conv-1', role: 'assistant', content: 'Rust 是系统编程语言', created_at: 1001 },
        { id: 'msg-3', conversation_id: 'conv-2', role: 'user', content: 'Python 有什么优点？', created_at: 1002 },
      ];
      window.__mock.state.conversations = [
        { id: 'conv-1', title: 'Rust 讨论' },
        { id: 'conv-2', title: 'Python 讨论' },
      ];
    });

    // 打开搜索弹框
    await page.evaluate(() => {
      const btn = document.querySelector('#convSearchBtn');
      if (btn) btn.click();
    });
    await page.waitForTimeout(300);

    // 切换到对话模式
    await page.locator('#convSearchModeContent').click();
    await page.waitForTimeout(100);

    // 输入搜索关键词
    await page.locator('#convSearchPopupInput').fill('Rust');
    await page.waitForTimeout(300);

    // 应显示搜索结果
    const results = page.locator('#convSearchResults .cursor-pointer');
    const count = await results.count();
    expect(count).toBeGreaterThan(0);

    // 结果应包含消息内容
    const firstResult = results.first();
    const text = await firstResult.textContent();
    expect(text).toContain('Rust');
  });

  test('TC-RAG-SEARCH-005d 对话模式空输入显示提示', async ({ page }) => {
    // 创建会话并打开搜索弹框
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('create_conversation')
    );
    await page.waitForTimeout(200);

    await page.evaluate(() => {
      const btn = document.querySelector('#convSearchBtn');
      if (btn) btn.click();
    });
    await page.waitForTimeout(300);

    // 切换到对话模式
    await page.locator('#convSearchModeContent').click();
    await page.waitForTimeout(100);

    // 不输入任何内容，应显示提示信息
    const hintText = page.locator('#convSearchResults');
    await expect(hintText).not.toBeEmpty();
  });

  test('TC-RAG-SEARCH-005e 切换模式按钮高亮正确', async ({ page }) => {
    // 创建会话并打开搜索弹框
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('create_conversation')
    );
    await page.waitForTimeout(200);

    await page.evaluate(() => {
      const btn = document.querySelector('#convSearchBtn');
      if (btn) btn.click();
    });
    await page.waitForTimeout(300);

    // 初始状态：title 高亮
    const titleBtn = page.locator('#convSearchModeTitle');
    const contentBtn = page.locator('#convSearchModeContent');

    let titleClass = await titleBtn.getAttribute('class') || '';
    let contentClass = await contentBtn.getAttribute('class') || '';
    expect(titleClass).toContain('bg-accent');
    expect(contentClass).not.toContain('bg-accent');

    // 点击切换到 content
    await contentBtn.click();
    await page.waitForTimeout(100);

    titleClass = await titleBtn.getAttribute('class') || '';
    contentClass = await contentBtn.getAttribute('class') || '';
    expect(titleClass).not.toContain('bg-accent');
    expect(contentClass).toContain('bg-accent');
  });
});
