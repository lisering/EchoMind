// E2E 会话持久化全场景（REQ-RAG-004/006）。
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, sendMessage, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-RAG-008~013 会话持久化', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    // 打开知识库弹框并导入文档（新 UI 中 #docList 在 KB Modal 内）
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/echomind-e2e.md'] }),
    );
    await page.locator('#docList [data-doc-name]').first().waitFor({ timeout: 5000 });
    await page.locator('#kbCloseBtn').click();
    await expect(page.locator('#kbModal')).toBeHidden();
  });

  test('E2E-RAG-011 会话 CRUD：新建 → 列表 → 删除', async ({ page }) => {
    // RC6 修复：newChat() 是懒创建（不立即写 DB），列表计数不变
    // 初始有一个自动创建的会话
    const initialCount = await page.locator('#convList > div').count();
    expect(initialCount).toBeGreaterThanOrEqual(1);

    // 新建会话（懒创建，不写 DB）
    await page.locator('#newChatBtn').click();
    await page.waitForTimeout(500);
    // 聊天区应重置为空状态
    await expect(page.locator('#chatArea .text-5xl')).toBeVisible({ timeout: 3000 });
    // 列表计数不变（懒创建不写 DB）
    const afterCreate = await page.locator('#convList > div').count();
    expect(afterCreate, '懒创建不写 DB，列表计数不变').toBe(initialCount);

    // 发送一条消息使会话落库
    await sendMessage(page, '测试会话持久化');
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 15000 });
    await page.waitForTimeout(500);
    // 现在列表应有 +1 个会话
    const afterMessage = await page.locator('#convList > div').count();
    expect(afterMessage, '发送消息后会话落库，列表 +1').toBe(initialCount + 1);

    // 删除第二个会话（非当前会话）
    const secondConv = page.locator('#convList > div').nth(1);
    await secondConv.hover();
    const delBtn = secondConv.locator('button:has-text("×")');
    await delBtn.click();
    await page.waitForTimeout(500);
    const afterDelete = await page.locator('#convList > div').count();
    expect(afterDelete, '删除后会话数 -1').toBe(initialCount);
  });

  test('E2E-RAG-012 首轮问答后标题自动提取', async ({ page }) => {
    await sendMessage(page, '灵犀是什么产品？');
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 15000 });

    // 等待会话列表刷新（chat_done 后 loadConversations）
    await page.waitForTimeout(1000);
    // 标题应从问题提取（mock 在 chat 命令中更新标题）
    // RC6 修复：#convList 内有分组头和会话项，用 .group 选择会话项
    const title = await page.locator('#convList .group span.truncate').first().innerText();
    expect(title, '标题应从问题提取').toContain('灵犀');
  });

  test('E2E-RAG-013 切换会话加载历史消息', async ({ page }) => {
    // 会话 A：发送一条消息
    await sendMessage(page, '会话A的问题');
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 15000 });
    const messagesA = await page.locator('#chatArea > div').count();
    expect(messagesA, '会话A应有消息').toBeGreaterThanOrEqual(2);

    // 新建会话 B
    await page.locator('#newChatBtn').click();
    await page.waitForTimeout(500);
    // 会话 B 应为空状态
    const messagesB = await page.locator('#chatArea .h-full').count();
    expect(messagesB, '新会话应为空状态').toBe(1);

    // 切回会话 A：点击侧栏第二个会话（最新的在前）
    await page.locator('#convList > div').nth(1).click();
    await page.waitForTimeout(1000);
    // 切回应恢复历史消息（至少 2 条：user + assistant）
    const userBlocks = await page.locator('#chatArea .flex.justify-end').count();
    expect(userBlocks, '切回应恢复用户消息').toBeGreaterThanOrEqual(1);
    const assistantBlocks = await page.locator('#chatArea .md-block').count();
    expect(assistantBlocks, '切回应恢复助手消息').toBeGreaterThanOrEqual(1);
  });

  test('E2E-RAG-008 多轮上下文携带历史', async ({ page }) => {
    // 第一轮问答
    await sendMessage(page, '第一轮问题');
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 15000 });

    // 第二轮问答
    await sendMessage(page, '第二轮问题');
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 15000 });

    // 验证 chatArea 有 4 条消息（2 user + 2 assistant）
    const userBlocks = await page.locator('#chatArea .flex.justify-end').count();
    expect(userBlocks, '应有两轮用户消息').toBe(2);
    const assistantBlocks = await page.locator('#chatArea .md-block').count();
    expect(assistantBlocks, '应有两轮助手消息').toBe(2);
  });

  test('E2E-RAG-009 新会话不携带旧历史', async ({ page }) => {
    // 会话 A 发消息
    await sendMessage(page, '旧会话问题');
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 15000 });

    // 新建会话
    await page.locator('#newChatBtn').click();
    await page.waitForTimeout(300);

    // 新会话应为空状态，不含旧消息
    const emptyState = await page.locator('#chatArea .h-full').count();
    expect(emptyState, '新会话应为空状态引导').toBe(1);
    const oldMsgCount = await page.locator('#chatArea .flex.justify-end').count();
    expect(oldMsgCount, '新会话不应含旧用户消息').toBe(0);
  });
});
