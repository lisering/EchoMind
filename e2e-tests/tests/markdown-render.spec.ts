// E2E Markdown 渲染完整性验收（REQ-UI-002）。
// E2E-MD-001: 标题（h1~h4）正确渲染
// E2E-MD-002: 无序列表渲染
// E2E-MD-003: 有序列表渲染
// E2E-MD-004: 引用块渲染
// E2E-MD-005: 链接渲染（安全 href）
// E2E-MD-006: 行内代码渲染
// E2E-MD-007: 加粗与斜体渲染
// E2E-MD-008: Markdown 表格渲染
// E2E-MD-009: 嵌套列表渲染
// E2E-MD-010: 分隔线渲染
import { test, expect } from '@playwright/test';
import { enterApp, importDocs, injectLocales, sendMessage, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';
test.describe('E2E-MD-001~010 Markdown 渲染完整性', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    await importDocs(page, ['/mock/md-test.md']);
  });

  test('E2E-MD-001 标题（h1~h4）正确渲染', async ({ page }) => {
    const tokens = [
      '# 一级标题\n\n', '## 二级标题\n\n', '### 三级标题\n\n', '#### 四级标题\n\n', '正文内容。',
    ];
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);
    await sendMessage(page, '测试标题');
    await waitForStreamDone(page);

    const md = page.locator('#chatArea .md').last();
    await expect(md.locator('h1')).toHaveText('一级标题');
    await expect(md.locator('h2')).toHaveText('二级标题');
    await expect(md.locator('h3')).toHaveText('三级标题');
    await expect(md.locator('h4')).toHaveText('四级标题');
  });

  test('E2E-MD-002 无序列表渲染', async ({ page }) => {
    const tokens = ['列表：\n\n', '- 项目一\n', '- 项目二\n', '- 项目三\n'];
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);
    await sendMessage(page, '测试无序列表');
    await waitForStreamDone(page);

    const md = page.locator('#chatArea .md').last();
    const items = md.locator('ul li');
    await expect(items).toHaveCount(3);
    await expect(items.nth(0)).toHaveText('项目一');
    await expect(items.nth(1)).toHaveText('项目二');
    await expect(items.nth(2)).toHaveText('项目三');
  });

  test('E2E-MD-003 有序列表渲染', async ({ page }) => {
    const tokens = ['步骤：\n\n', '1. 第一步\n', '2. 第二步\n', '3. 第三步\n'];
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);
    await sendMessage(page, '测试有序列表');
    await waitForStreamDone(page);

    const md = page.locator('#chatArea .md').last();
    const items = md.locator('ol li');
    await expect(items).toHaveCount(3);
  });

  test('E2E-MD-004 引用块渲染', async ({ page }) => {
    const tokens = ['引用：\n\n', '> 这是一段引用文字。\n', '> 第二行引用。\n'];
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);
    await sendMessage(page, '测试引用');
    await waitForStreamDone(page);

    const md = page.locator('#chatArea .md').last();
    const blockquote = md.locator('blockquote');
    await expect(blockquote).toBeVisible();
    const text = await blockquote.innerText();
    expect(text).toContain('引用文字');
    expect(text).toContain('第二行');
  });

  test('E2E-MD-005 链接渲染且 href 安全', async ({ page }) => {
    const tokens = ['链接：[EchoMind 官网](https://echomind.dev)\n'];
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);
    await sendMessage(page, '测试链接');
    await waitForStreamDone(page);

    const md = page.locator('#chatArea .md').last();
    const link = md.locator('a');
    await expect(link).toBeVisible();
    await expect(link).toHaveText('EchoMind 官网');
    const href = await link.getAttribute('href');
    expect(href).toBe('https://echomind.dev');
  });

  test('E2E-MD-006 行内代码渲染', async ({ page }) => {
    const tokens = ['使用 `cargo build` 编译项目。\n'];
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);
    await sendMessage(page, '测试行内代码');
    await waitForStreamDone(page);

    const md = page.locator('#chatArea .md').last();
    const code = md.locator('code');
    await expect(code).toBeVisible();
    await expect(code).toHaveText('cargo build');
    // 行内代码不应在 <pre> 内
    const isInPre = await code.evaluate((el) => !!el.closest('pre'));
    expect(isInPre, '行内代码不应在 pre 标签内').toBe(false);
  });

  test('E2E-MD-007 加粗与斜体渲染', async ({ page }) => {
    const tokens = ['这是**加粗**文字，这是*斜体*文字。\n'];
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);
    await sendMessage(page, '测试加粗斜体');
    await waitForStreamDone(page);

    const md = page.locator('#chatArea .md').last();
    await expect(md.locator('strong')).toHaveText('加粗');
    await expect(md.locator('em')).toHaveText('斜体');
  });

  test('E2E-MD-008 Markdown 表格渲染', async ({ page }) => {
    const tokens = [
      '数据：\n\n',
      '| 名称 | 值 |\n', '|------|----|\n', '| A | 1 |\n', '| B | 2 |\n',
    ];
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);
    await sendMessage(page, '测试表格');
    await waitForStreamDone(page);

    const md = page.locator('#chatArea .md').last();
    const table = md.locator('table');
    await expect(table).toBeVisible();
    // 表头 + 2 数据行
    const rows = table.locator('tr');
    await expect(rows).toHaveCount(3);
    // 表头含「名称」「值」
    await expect(rows.first().locator('th').first()).toHaveText('名称');
  });

  test('E2E-MD-009 嵌套列表渲染', async ({ page }) => {
    const tokens = [
      '嵌套：\n\n',
      '- 外层一\n', '  - 内层 1\n', '  - 内层 2\n',
      '- 外层二\n',
    ];
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);
    await sendMessage(page, '测试嵌套列表');
    await waitForStreamDone(page);

    const md = page.locator('#chatArea .md').last();
    // 外层列表存在
    const outerUl = md.locator('ul').first();
    await expect(outerUl).toBeVisible();
    // 内层列表存在（嵌套 ul）
    const nestedUl = md.locator('ul ul');
    await expect(nestedUl).toBeVisible();
    const nestedItems = nestedUl.locator('li');
    await expect(nestedItems).toHaveCount(2);
  });

  test('E2E-MD-010 分隔线渲染', async ({ page }) => {
    const tokens = ['上方文字\n\n', '---\n\n', '下方文字\n'];
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);
    await sendMessage(page, '测试分隔线');
    await waitForStreamDone(page);

    const md = page.locator('#chatArea .md').last();
    const hr = md.locator('hr');
    await expect(hr).toBeVisible();
  });
});
