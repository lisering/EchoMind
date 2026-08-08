// E2E PDF 导出功能（REQ-EXP-005）：
// TC-EXP-PDF-001: 导出菜单含「导出为 PDF」按钮（DOM 元素存在性断言）
// TC-EXP-PDF-002: 点击导出触发 window.print（mock 记录调用次数，断言被调用）
// TC-EXP-PDF-003: 打印内容包含对话消息（检查 __lastPrintHtml 包含消息文本）
// TC-EXP-PDF-004: @media print 隐藏 UI 元素（检查打印 HTML 含 @page 规则 + 隐藏选择器）
// TC-EXP-PDF-005: 空对话导出不崩溃（空对话导出显示提示 toast，不报错）
import { test, expect } from '@playwright/test';
import { setupPage, uiUrl } from './helpers.mjs';

test.describe('TC-EXP-PDF PDF 导出功能（REQ-EXP-005）', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  // ─── 基本导出功能 ───

  test('TC-EXP-PDF-001 导出菜单含「导出为 PDF」按钮', async ({ page }) => {
    // 断言 #exportPdfBtn 按钮存在于 DOM 中
    const btn = page.locator('#exportPdfBtn');
    await expect(btn).toBeAttached();
    // 按钮应有 title 或 aria-label 属性（i18n 绑定）
    const title = await btn.getAttribute('data-i18n-title');
    expect(title).toBe('export_pdf.title');
  });

  test('TC-EXP-PDF-002 点击导出触发 window.print', async ({ page }) => {
    // 创建会话并添加消息
    const convId = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('create_conversation');
    });
    await page.evaluate((cid) => {
      window.__mock.state.messages[cid] = [
        { role: 'user', content: '什么是 RAG？', sources: null },
        { role: 'assistant', content: 'RAG 是检索增强生成技术。', sources: [] },
      ];
    }, convId);

    // 记录调用前的 mock 计数
    const beforeCount = await page.evaluate(() => window.__printMockCalled || 0);

    // 直接调用导出函数（已暴露为全局函数）
    await page.evaluate((cid) => {
      return window.exportConversationToPdf(cid, '测试对话');
    }, convId);

    // 等待 printViaIframe 内部 setTimeout(100ms) 完成
    await page.waitForFunction(
      (before) => window.__printMockCalled > before,
      beforeCount,
      { timeout: 5000 },
    );

    const afterCount = await page.evaluate(() => window.__printMockCalled);
    expect(afterCount).toBeGreaterThan(beforeCount);
  });

  test('TC-EXP-PDF-003 打印内容包含对话消息', async ({ page }) => {
    // 创建会话并添加消息
    const convId = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('create_conversation');
    });
    await page.evaluate((cid) => {
      window.__mock.state.messages[cid] = [
        { role: 'user', content: '什么是向量检索？', sources: null },
        { role: 'assistant', content: '向量检索是通过余弦相似度匹配查询向量和文档向量的技术。', sources: [] },
      ];
    }, convId);

    // 调用导出函数
    await page.evaluate((cid) => {
      return window.exportConversationToPdf(cid, '向量检索对话');
    }, convId);

    // 等待打印 HTML 生成
    await page.waitForFunction(
      () => window.__lastPrintHtml !== null,
      { timeout: 5000 },
    );

    const html = await page.evaluate(() => window.__lastPrintHtml);
    expect(html).toBeTruthy();
    // 打印 HTML 应包含用户消息
    expect(html).toContain('什么是向量检索');
    // 打印 HTML 应包含助手消息
    expect(html).toContain('余弦相似度');
    // 打印 HTML 应包含标题
    expect(html).toContain('向量检索对话');
  });

  test('TC-EXP-PDF-004 @media print 隐藏 UI 元素', async ({ page }) => {
    // 创建会话并添加消息
    const convId = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('create_conversation');
    });
    await page.evaluate((cid) => {
      window.__mock.state.messages[cid] = [
        { role: 'user', content: '测试打印样式', sources: null },
        { role: 'assistant', content: '验证 @page 和隐藏规则。', sources: [] },
      ];
    }, convId);

    // 调用导出函数
    await page.evaluate((cid) => {
      return window.exportConversationToPdf(cid, '打印样式测试');
    }, convId);

    // 等待打印 HTML 生成
    await page.waitForFunction(
      () => window.__lastPrintHtml !== null,
      { timeout: 5000 },
    );

    const html = await page.evaluate(() => window.__lastPrintHtml);
    expect(html).toBeTruthy();

    // 检查 @page 规则存在（页面大小 + 页边距）
    expect(html).toContain('@page');
    expect(html).toMatch(/size:\s*(A4|Letter)/);

    // 检查隐藏选择器存在（侧栏/输入框/设置面板等非打印元素）
    expect(html).toContain('#sidebar');
    expect(html).toContain('#inputBar');
    expect(html).toContain('#settingsPanel');
    expect(html).toContain('display: none');

    // 检查分页控制存在
    expect(html).toContain('page-break-inside: avoid');

    // 检查打印排版优化（serif 字体 + 12pt 字号）
    expect(html).toContain('serif');
    expect(html).toContain('12pt');
  });

  test('TC-EXP-PDF-005 空对话导出不崩溃', async ({ page }) => {
    // 创建空会话（无消息）
    const convId = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('create_conversation');
    });

    // 记录调用前的 mock 计数
    const beforeCount = await page.evaluate(() => window.__printMockCalled || 0);

    // 调用导出函数（空对话应显示 toast 错误，不崩溃）
    await page.evaluate((cid) => {
      return window.exportConversationToPdf(cid);
    }, convId);

    // 等待 toast 出现（空对话导出应显示错误提示）
    await page.waitForFunction(
      () => {
        const toasts = document.getElementById('toasts');
        return toasts && toasts.children.length > 0;
      },
      { timeout: 5000 },
    );

    // 验证 toast 文本非空
    const toastText = await page.evaluate(() => {
      const toasts = document.getElementById('toasts');
      return toasts?.textContent || '';
    });
    expect(toastText.length).toBeGreaterThan(0);

    // 验证 window.print 未被调用（空对话不应触发打印）
    const afterCount = await page.evaluate(() => window.__printMockCalled);
    expect(afterCount).toBe(beforeCount);
  });
});
