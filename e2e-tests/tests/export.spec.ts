// E2E 导出功能（REQ-EXP-001）：
// E2E-EXP-001: 导出对话为 Markdown——基本格式验证
// E2E-EXP-002: 导出包含用户消息
// E2E-EXP-003: 导出包含助手消息
// E2E-EXP-004: 导出空对话——返回仅含标题的 Markdown
// E2E-EXP-005: save_text_file——保存文件路径记录
// E2E-EXP-006: 导出 Markdown 含时间戳
// E2E-EXP-007: 导出多轮对话——消息顺序正确
// E2E-EXP-008: 导出内容含来源引用标记
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, sendMessage, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';
test.describe('E2E-EXP 导出功能（REQ-EXP-001）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── 基本导出功能 ───

  test('E2E-EXP-001 导出对话为 Markdown——基本格式验证', async ({ page }) => {
    // 创建会话并发送消息
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('create_conversation')
    );
    const convId = await page.evaluate(() =>
      window.__mock.state.conversations[0]?.id
    );
    expect(convId).not.toBeNull();
    expect(typeof convId).toBe('string');
    expect(convId.length).toBeGreaterThan(0);

    // 添加一条用户消息
    await page.evaluate((cid) => {
      window.__mock.state.messages[cid] = [
        { role: 'user', content: '什么是 Rust？', sources: null },
        { role: 'assistant', content: 'Rust 是一门系统编程语言。', sources: [] },
      ];
    }, convId);

    // 导出
    const md = await page.evaluate((cid) =>
      window.__TAURI__.core.invoke('export_conversation_markdown', { conversationId: cid })
    , convId);

    expect(md).toContain('# 对话导出');
    expect(md).toContain('什么是 Rust');
    expect(md).toContain('Rust 是一门系统编程语言');
  });

  test('E2E-EXP-002 导出包含用户消息', async ({ page }) => {
    await page.evaluate(() => {
      window.__TAURI__.core.invoke('create_conversation');
    });
    const convId = await page.evaluate(() => window.__mock.state.conversations[0].id);

    await page.evaluate((cid) => {
      window.__mock.state.messages[cid] = [
        { role: 'user', content: '用户提问内容', sources: null },
      ];
    }, convId);

    const md = await page.evaluate((cid) =>
      window.__TAURI__.core.invoke('export_conversation_markdown', { conversationId: cid })
    , convId);

    expect(md).toContain('用户提问内容');
    expect(md).toContain('用户');
  });

  test('E2E-EXP-003 导出包含助手消息', async ({ page }) => {
    await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    const convId = await page.evaluate(() => window.__mock.state.conversations[0].id);

    await page.evaluate((cid) => {
      window.__mock.state.messages[cid] = [
        { role: 'assistant', content: '助手回答内容', sources: [] },
      ];
    }, convId);

    const md = await page.evaluate((cid) =>
      window.__TAURI__.core.invoke('export_conversation_markdown', { conversationId: cid })
    , convId);

    expect(md).toContain('助手回答内容');
    expect(md).toContain('助手');
  });

  test('E2E-EXP-004 导出空对话——返回仅含标题的 Markdown', async ({ page }) => {
    await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    const convId = await page.evaluate(() => window.__mock.state.conversations[0].id);

    const md = await page.evaluate((cid) =>
      window.__TAURI__.core.invoke('export_conversation_markdown', { conversationId: cid })
    , convId);

    expect(md).toContain('# 对话导出');
    expect(md).toContain('导出时间');
    // 空对话不应包含消息体
    expect(md).not.toContain('用户');
    expect(md).not.toContain('助手');
  });

  test('E2E-EXP-005 save_text_file——保存文件路径记录', async ({ page }) => {
    const testPath = '/mock/export/conversation.md';
    const testContent = '# Test Export\n\nContent here.';

    await page.evaluate(({ path, content }) =>
      window.__TAURI__.core.invoke('save_text_file', { path, content })
    , { path: testPath, content: testContent });

    const saved = await page.evaluate(() => ({
      path: window.__mock.state.lastExportPath,
      content: window.__mock.state.lastExportContent,
    }));
    expect(saved.path).toBe(testPath);
    expect(saved.content).toBe(testContent);
  });

  test('E2E-EXP-006 导出 Markdown 含时间戳', async ({ page }) => {
    await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    const convId = await page.evaluate(() => window.__mock.state.conversations[0].id);

    const md = await page.evaluate((cid) =>
      window.__TAURI__.core.invoke('export_conversation_markdown', { conversationId: cid })
    , convId);

    // 应包含 ISO 格式时间戳
    expect(md).toMatch(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/);
  });

  test('E2E-EXP-007 导出多轮对话——消息顺序正确', async ({ page }) => {
    await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    const convId = await page.evaluate(() => window.__mock.state.conversations[0].id);

    await page.evaluate((cid) => {
      window.__mock.state.messages[cid] = [
        { role: 'user', content: '第一个问题', sources: null },
        { role: 'assistant', content: '第一个回答', sources: [] },
        { role: 'user', content: '第二个问题', sources: null },
        { role: 'assistant', content: '第二个回答', sources: [] },
      ];
    }, convId);

    const md = await page.evaluate((cid) =>
      window.__TAURI__.core.invoke('export_conversation_markdown', { conversationId: cid })
    , convId);

    const q1Pos = md.indexOf('第一个问题');
    const a1Pos = md.indexOf('第一个回答');
    const q2Pos = md.indexOf('第二个问题');
    const a2Pos = md.indexOf('第二个回答');

    expect(q1Pos).toBeLessThan(a1Pos);
    expect(a1Pos).toBeLessThan(q2Pos);
    expect(q2Pos).toBeLessThan(a2Pos);
  });
});
