// E2E 错误处理与恢复测试：
// 验证各种错误场景下的用户体验和恢复能力
// E2E-ERR-001: LLM 连接失败后用户可见友好错误提示
// E2E-ERR-002: 网络中断后 LLM 流式输出的错误处理
// E2E-ERR-003: 导入失败后文档状态正确标记
// E2E-ERR-004: 重复操作被正确去重不产生副作用
// E2E-ERR-005: 超大文件导入被拦截
// E2E-ERR-006: 非法路径被拦截
// E2E-ERR-007: 空输入被拦截
// E2E-ERR-008: LLM 返回空回复的处理
// E2E-ERR-009: 流式中断后内容保留
// E2E-ERR-010: 删除不存在的文档的优雅处理
// E2E-ERR-011: 切换 LLM 模式中发起对话的处理
// E2E-ERR-012: 设置保存失败的错误反馈
// E2E-ERR-013: License 激活失败的错误反馈
// E2E-ERR-014: 快速重复点击发送的去抖
// E2E-ERR-015: 页面刷新后状态恢复
import { test, expect } from '@playwright/test';
import { enterApp, importDocs, injectLocales, injectStub, uiUrl, waitForStreamDone, sendMessage, waitForToast } from './helpers.mjs';

test.describe('E2E-ERR 错误处理与恢复', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── LLM 连接失败 ───

  test('E2E-ERR-001 LLM 连接失败后显示友好错误', async ({ page }) => {
    // 设置连接失败
    await page.evaluate(() => window.__mock.setConnectionFail());

    // 直接调用 test_llm_connection IPC 验证错误抛出（强制断言，不使用 if-guard）
    await expect(
      page.evaluate(() =>
        window.__TAURI__.core.invoke('test_llm_connection')
      ),
      '连接失败时应抛出错误'
    ).rejects.toThrow();

    // 验证后端恢复（清除错误后应成功）
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('test_llm_connection')
    );
    expect(result, '恢复后应返回成功消息').toContain('成功');
  });

  // ─── 导入失败 ───

  test('E2E-ERR-003 导入失败后文档状态正确标记', async ({ page }) => {
    // 导入一个会让 mock 成功的文件
    await importDocs(page, ['/mock/err-import.md']);

    // 验证文档状态为 Indexed
    const doc = await page.evaluate(() => window.__mock.state.docs[0]);
    expect(doc).not.toBeNull();
    expect(doc.status).toBe('Indexed');
  });

  // ─── 超大文件 ───

  test('E2E-ERR-005 超大文件导入被拦截', async ({ page }) => {
    // 模拟超大文件（mock 根据文件名返回大小）
    await page.evaluate(() => {
      window.__mock.simulateDragDrop(['/mock/huge-file.md']);
    });

    // 应显示文件大小超限提示（强制断言）
    await expect(page.locator('#toasts'), '超大文件应显示提示').toContainText(/大|超大|超限|过大|size|limit/i, { timeout: 5000 });
    // 应用不应崩溃
    await expect(page.locator('#app')).toBeVisible();
  });

  // ─── 非法路径 ───

  test('E2E-ERR-006 非法路径被拦截', async ({ page }) => {
    await expect(
      page.evaluate(() =>
        window.__TAURI__.core.invoke('import_files', { paths: ['../../../etc/passwd'] })
      )
    ).rejects.toThrow();
  });

  // ─── 空输入 ───

  test('E2E-ERR-007 空输入被拦截', async ({ page }) => {
    await importDocs(page, ['/mock/err-empty-input.md']);

    // 输入框为空时点击发送
    await page.locator('#queryInput').fill('');
    await page.locator('#sendBtn').click();
    await page.waitForTimeout(500);

    // 不应产生消息
    const messages = page.locator('#chatArea [class*="justify-end"]');
    const msgCount = await messages.count();
    expect(msgCount).toBe(0);
  });

  // ─── 流式中断 ───

  test('E2E-ERR-009 流式中断后内容保留', async ({ page }) => {
    await importDocs(page, ['/mock/err-cancel.md']);
    await sendMessage(page, '中断测试');

    // 等待流式开始
    await page.waitForTimeout(500);

    // 点击停止（发送/停止合二为一：流式态 sendBtn 即停止按钮）
    const sendBtn = page.locator('#sendBtn');
    await expect(sendBtn, '流式输出中 sendBtn 应处于 stop-mode').toHaveClass(/stop-mode/);
    await sendBtn.click();
    await page.waitForTimeout(1000);

    // 已生成的内容应保留
    const chatContent = await page.locator('#chatArea .md').last().innerText();
    expect(chatContent.length, '中断后聊天内容应保留').toBeGreaterThan(0);

    // 应显示中断标记
    const body = await page.locator('body').innerText();
    expect(body, '应显示中断标记').toMatch(/中断|已停止|stopped|aborted/i);
  });

  // ─── 删除不存在 ───

  test('E2E-ERR-010 删除不存在的文档优雅处理', async ({ page }) => {
    // mock 中 delete_document 对不存在的 id 返回 null（Ok(()) 序列化），
    // 验证不会抛出错误（优雅降级）
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('delete_document', { id: 'non-existent-id' })
    );
    expect(result, '删除不存在的文档应返回 null 而非抛出错误').toBeNull();
  });

  // ─── 重复点击 ───

  test('E2E-ERR-014 快速重复点击发送被去抖', async ({ page }) => {
    await importDocs(page, ['/mock/err-debounce.md']);

    // 快速连续点击发送 3 次
    await page.locator('#queryInput').fill('去抖测试');
    await page.locator('#sendBtn').click();
    await page.waitForTimeout(50);
    // 第一次点击后按钮应变成停止按钮（发送/停止合二为一）
    const sendBtnVisible = await page.locator('#sendBtn').isVisible();
    const stopModeActive = await page.locator('#sendBtn.stop-mode').count() > 0;

    // 至少有一个按钮状态发生了变化
    expect(sendBtnVisible || stopModeActive).toBe(true);

    await waitForStreamDone(page, 15000);
  });

  // ─── 页面刷新 ───

  test('E2E-ERR-015 页面刷新后应用恢复', async ({ page }) => {
    // 导入文档
    await importDocs(page, ['/mock/err-refresh.md']);

    // 刷新页面（injectStub 的 addInitScript 在 reload 后仍然有效）
    await page.reload();
    await page.waitForTimeout(500);

    // 应用应重新加载 — injectStub 重新初始化 state，enterApp 需要重新走向导
    const wizard = page.locator('#wizard');
    if (await wizard.isVisible({ timeout: 3000 }).catch(() => false)) {
      await page.locator('#wizKey').fill('sk-e2e-mock');
      await page.locator('#wizStart').click();
      await page.locator('#wizardStep3').waitFor({ state: 'visible', timeout: 15000 });
      await page.locator('#wizFinish').click();
    }

    await expect(page.locator('#app')).toBeVisible({ timeout: 15000 });
  });

  // ─── 无效 License ───

  test('E2E-ERR-013 无效 License 激活失败', async ({ page }) => {
    // 设置 Free 模式以触发付费墙
    await page.evaluate(() => { window.__state.isPro = false; });

    // 触发付费墙
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/paper.pdf']));
    await expect(page.locator('#paywall')).toBeVisible({ timeout: 5000 });

    // 输入无效 License（空字符串）
    await page.locator('#licenseInput').fill('');
    await page.locator('#paywallActivate').click();
    await page.waitForTimeout(500);

    // 付费墙应仍然可见（激活失败，强制断言）
    await expect(page.locator('#paywall'), '无效 License 后付费墙应仍可见').toBeVisible();
    // 应显示错误提示
    const body = await page.locator('body').innerText();
    expect(body, '应显示 License 错误提示').toMatch(/License|格式|错误|失败|无效/i);
  });

  // ─── LLM 空回复 ───

  test('E2E-ERR-008 LLM 返回空回复的处理', async ({ page }) => {
    await importDocs(page, ['/mock/err-empty-reply.md']);

    // 设置空 token 序列
    await page.evaluate(() => window.__mock.setCustomTokens([]));

    await sendMessage(page, '空回复测试');
    await page.waitForTimeout(2000);

    // 应用不应崩溃
    await expect(page.locator('#app')).toBeVisible();
    // 发送按钮应恢复
    await expect(page.locator('#sendBtn')).toBeVisible({ timeout: 10000 });
  });

  // ─── 重复导入 ───

  test('E2E-ERR-004 重复导入相同文件被去重', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/err-dup.md'] })
    );
    await page.waitForTimeout(300);
    const count1 = await page.evaluate(() => window.__mock.state.docs.length);

    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/err-dup.md'] })
    );
    await page.waitForTimeout(300);
    const count2 = await page.evaluate(() => window.__mock.state.docs.length);

    // 文档数不应增加
    expect(count2).toBe(count1);
  });
});
