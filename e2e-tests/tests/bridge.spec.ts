// E2E-UI-001-lite 桥接层全链路冒烟（Playwright + 契约级 Mock __TAURI__）。
// 与 L3 真实层（specs/app.smoke.spec.mjs）断言逐条对应；本机（含 macOS）headless 可跑。
import { test, expect } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const specDir = path.dirname(fileURLToPath(import.meta.url));
const uiUrl = 'file://' + path.resolve(specDir, '../../ui/index.html');
const stubPath = path.resolve(specDir, 'bridge/tauri-stub.js');

test.describe('E2E-UI-001-lite 桥接层冒烟', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript({ path: stubPath });
    await page.goto(uiUrl);
  });

  test('六断言全链路（向导→配置→导入→流式→停止→删除）', async ({ page }) => {
    // 01 首次启动向导 UI 可见
    await expect(page.locator('#wizard')).toBeVisible();

    // 02 经向导真实流程注入配置（test_llm_connection + update_llm_config）并进入主界面
    await page.locator('#wizKey').fill('sk-e2e-mock');
    await page.locator('#wizStart').click();
    await expect(page.locator('#app')).toBeVisible();

    // 03 导入文件后文档列表渲染文件名（doc-status-changed 事件驱动刷新）
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/echomind-e2e.md'] }),
    );
    const docItem = page.locator('#docList [data-doc-name="echomind-e2e.md"]');
    await expect(docItem).toBeVisible({ timeout: 15000 });
    await expect(docItem).toContainText('echomind-e2e.md');

    // 04 流式对话：气泡出现、innerText 增长、代码块与复制按钮
    await page.locator('#queryInput').fill('EchoMind 支持哪些格式？');
    await page.locator('#sendBtn').click();
    const md = page.locator('#chatArea .md').last();
    await expect(md).toBeVisible();
    const len1 = (await md.innerText()).length;
    await page.waitForTimeout(600);
    const len2 = (await md.innerText()).length;
    expect(len2).toBeGreaterThan(len1);
    await expect(page.locator('#chatArea pre code').last()).toBeVisible({ timeout: 30000 });
    await page.locator('#chatArea pre').last().hover();
    await expect(page.locator('#chatArea .copy-btn').last()).toBeVisible();

    // 05 停止生成：输出中断并出现「已中断」标记
    await page.locator('#queryInput').fill('EchoMind 支持哪些格式？请再回答一次');
    await page.locator('#sendBtn').click();
    await expect(page.locator('#stopBtn')).toBeVisible();
    await page.waitForTimeout(500);
    await page.locator('#stopBtn').click();
    await expect(page.locator('#sendBtn')).toBeVisible({ timeout: 15000 });
    await expect(page.locator('text=⏹ 已中断')).toBeVisible();

    // 06 删除文档：DOM 元素从列表中消失
    await docItem.hover();
    await docItem.locator('button').click();
    await expect(page.locator('#docList [data-doc-name="echomind-e2e.md"]')).toHaveCount(0);
  });

  test('E2E-LIC-001 付费墙弹出→激活→状态更新（REQ-LIC-003）', async ({ page }) => {
    // 01 经向导进入主界面
    await page.locator('#wizKey').fill('sk-e2e-mock');
    await page.locator('#wizStart').click();
    await expect(page.locator('#app')).toBeVisible();

    // 02 免费版导入 PDF → PRO_REQUIRED 错误 → 付费墙 Modal 弹出
    // 通过前端 drag-drop 监听器触发 importPaths（错误处理在 importPaths 内）
    await page.evaluate(() => {
      const listeners = window.__state.listeners['tauri://drag-drop'] || [];
      listeners.forEach((cb) => cb({ payload: { paths: ['/mock/paper.pdf'] } }));
    });
    await expect(page.locator('#paywall')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#paywallReason')).toContainText('PDF');

    // 03 输入 License Key 并激活 → Modal 关闭
    await page.locator('#licenseInput').fill('test-license-key');
    await page.locator('#paywallActivate').click();
    await expect(page.locator('#paywall')).toBeHidden({ timeout: 5000 });

    // 04 侧栏授权状态更新为 Pro
    await expect(page.locator('#proStatus')).toContainText('Pro 版');

    // 05 Pro 版可正常导入 PDF（不再拦截）
    await page.evaluate(() => {
      const listeners = window.__state.listeners['tauri://drag-drop'] || [];
      listeners.forEach((cb) => cb({ payload: { paths: ['/mock/paper.pdf'] } }));
    });
    await expect(page.locator('#docList [data-doc-name="paper.pdf"]')).toBeVisible({ timeout: 5000 });
  });
});
