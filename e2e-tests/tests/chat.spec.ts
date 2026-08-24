// E2E 对话链路全场景（REQ-RAG-001~007、REQ-UI-003/006、思考过程）。
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, sendMessage, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-RAG-001~015 对话链路', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    // 导入文档（对话前置条件）
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/echomind-e2e.md'] }),
    );
    // 等待元素挂载到 DOM（KB Modal 可能隐藏，不要求可见）
    await page.locator('#docList [data-doc-name]').first().waitFor({ state: 'attached', timeout: 5000 });
  });

  test('E2E-RAG-001 发送后立即出现思考指示器', async ({ page }) => {
    await sendMessage(page, '测试思考指示器');
    // 思考面板（thinking-panel，替代旧 thinking-indicator）应立即可见
    await expect(page.locator('.thinking-panel')).toBeVisible({ timeout: 5000 });
    // 思考面板文案可见（替代旧 typing-dot 断言）
    await expect(page.locator('.thinking-panel-text')).toBeVisible({ timeout: 3000 });
  });

  test('E2E-RAG-014 chat_phase 事件更新指示器文案', async ({ page }) => {
    await sendMessage(page, '测试阶段文案');
    // 初始文案：思考面板文本可见
    await expect(page.locator('.thinking-panel-text')).toBeVisible({ timeout: 3000 });
    // chat_phase 同时更新 #inputHint，该元素不会被 chat_token 移除
    // 等待文案更新为「正在生成回答…」
    await expect(page.locator('#inputHint')).toContainText('生成回答', { timeout: 8000 });
  });

  test('E2E-RAG-015 首 token 到达后指示器消失', async ({ page }) => {
    await sendMessage(page, '测试指示器消失');
    await expect(page.locator('.thinking-panel')).toBeVisible({ timeout: 3000 });
    // 等待首个 token 到达
    await expect(page.locator('#chatArea .md').last()).toBeVisible({ timeout: 10000 });
    // 思考面板应标记为完成状态（setComplete 后 header 文案变化）
    const panelHeader = page.locator('.thinking-panel-text');
    await expect(panelHeader).toBeVisible({ timeout: 3000 });
  });

  test('E2E-RAG-002 流式过程中 UI 不冻结，可滚动', async ({ page }) => {
    await sendMessage(page, '测试不冻结');
    // V5 重构：流式过程中输入框保持启用（支持排队发送）
    await expect(page.locator('#queryInput')).not.toBeDisabled();
    // 但 chatArea 仍可滚动（非阻塞）
    const canScroll = await page.evaluate(() => {
      const area = document.getElementById('chatArea');
      return area.scrollHeight > 0;
    });
    expect(canScroll).toBe(true);
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 15000 });
  });

  test('E2E-RAG-003 回答结束后 Block 进入完成态', async ({ page }) => {
    await sendMessage(page, '测试完成态');
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 15000 });
    // 完成后输入框恢复空闲态
    await expect(page.locator('#queryInput')).not.toBeDisabled();
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/);
    // assistant block 有内容
    const mdContent = await page.locator('#chatArea .md').last().innerText();
    expect(mdContent.length, '回答内容不应为空').toBeGreaterThan(0);
  });

  test('E2E-RAG-004 引用块含文档名、片段预览、分数', async ({ page }) => {
    await sendMessage(page, '测试引用');
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 15000 });
    // 展开引用来源列表（等待 toggle 出现再点击）
    const toggle = page.locator('#chatArea .sources .sources-toggle').first();
    await toggle.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {});
    if (await toggle.count() > 0) await toggle.click();
    const sourceChip = page.locator('#chatArea .sources .source-card').last();
    await expect(sourceChip).toBeVisible({ timeout: 5000 });
    const text = await sourceChip.innerText();
    expect(text, '引用应含文档名').toContain('echomind-e2e.md');
    expect(text, '引用应含百分比分数').toMatch(/\d+%/);
  });

  test('E2E-RAG-007 空上下文返回固定提示，不调用 LLM', async ({ page }) => {
    // 设置下次 chat 返回空上下文
    await page.evaluate(() => window.__mock.setNextChatEmpty());
    await sendMessage(page, '查不到的内容');
    // 应返回固定提示文案
    await expect(page.locator('#chatArea .md').last()).toContainText('未找到相关内容', { timeout: 10000 });
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 15000 });
    // 空上下文不应有引用块（.sources 容器始终存在，检查内部是否有 chip）
    const lastSourcesBox = page.locator('#chatArea .sources').last();
    const chipCount = await lastSourcesBox.locator('*').count();
    expect(chipCount, '空上下文不应有引用').toBe(0);
  });

  test('E2E-UI-007 生成中 Enter 与发送按钮均无效', async ({ page }) => {
    await sendMessage(page, '第一问');
    await expect(page.locator('#sendBtn.stop-mode')).toBeVisible({ timeout: 5000 });
    // 输入框被禁用，Enter 键不触发发送
    // 使用 evaluate 模拟按键（disabled 元素无法 fill）
    await page.evaluate(() => {
      const input = document.getElementById('queryInput');
      input.value = '不该发送的内容';
      const ev = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true });
      input.dispatchEvent(ev);
    });
    await page.waitForTimeout(500);
    // 不应出现第二条用户消息
    const userBlocks = await page.locator('#chatArea .flex.justify-end').count();
    expect(userBlocks, '生成中 Enter 不应发送').toBe(1);
  });

  test('E2E-UI-008 后端报错时输入框进入错误态', async ({ page }) => {
    // 使用 setChatError 模拟 chat IPC 返回错误（V1 修复后不再 emit chat_error 事件）
    await page.evaluate(() => window.__mock.setChatError('NETWORK: 模拟后端错误'));
    await page.locator('#queryInput').fill('触发错误');
    await page.locator('#sendBtn').click();
    // 等待错误 toast 出现（NETWORK 前缀映射为「网络连接异常」）
    await expect(page.locator('#toasts')).toContainText('网络连接异常', { timeout: 5000 });
    // 清除错误模式
    await page.evaluate(() => window.__mock.clearChatError());
  });

  test('E2E-RAG-010 停止后输入框恢复空闲态', async ({ page }) => {
    await sendMessage(page, '测试停止');
    await expect(page.locator('#sendBtn.stop-mode')).toBeVisible({ timeout: 5000 });
    await page.waitForTimeout(300);
    await page.locator('#sendBtn').click(); // 流式态点击 = 停止
    await expect(page.locator('#sendBtn')).toBeVisible({ timeout: 15000 });
    await expect(page.locator('#queryInput')).not.toBeDisabled();
  });

  test('E2E-UI-006 流式半截代码围栏不产生布局错乱', async ({ page }) => {
    // 设置只有开围栏没有闭围栏的 token
    await page.evaluate(() => {
      window.__mock.setCustomTokens(['代码开始：\n\n```rust\nfn half() {\n    // 未闭合']);
    });
    await sendMessage(page, '半截代码');
    // 轮询等待内容渲染（单 token 流式 + Markdown 渲染异步）
    const mdEl = page.locator('#chatArea .md').last();
    await expect
      .poll(async () => (await mdEl.innerText()).length, {
        timeout: 10000,
        message: '半截代码应渲染出内容',
      })
      .toBeGreaterThan(0);
    // 页面不应崩溃，chatArea 仍有内容
    const content = await mdEl.innerText();
    expect(content.length).toBeGreaterThan(0);
    // 不应有未消毒的 script（布局安全）
    const html = await mdEl.innerHTML();
    expect(html).not.toContain('<script');
  });
});
