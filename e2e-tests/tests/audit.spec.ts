// E2E 文档一致性审计 UI 全场景（REQ-AUDIT-001~005）。
// E2E-AUDIT-001: Pro 版 Indexed 文档显示审计按钮
// E2E-AUDIT-002: 审计阶段事件驱动思考指示器文案
// E2E-AUDIT-003: 审计报告流式渲染为 Markdown
// E2E-AUDIT-004: 审计报告含矛盾清单表格
// E2E-AUDIT-005: 审计取消（stopBtn 调用 abort_audit）
import { test, expect } from '@playwright/test';
import { uiUrl, injectStub, enterApp, activatePro, importDocs, waitDone, injectLocales, openKbModal, setupPage } from './helpers.mjs';

test.describe('E2E-AUDIT-001~005 文档一致性审计 UI', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    await activatePro(page);
    await importDocs(page, ['/mock/audit-doc.md']);
    await openKbModal(page);
  });

  test('E2E-AUDIT-001 Pro 版 Indexed 文档显示审计按钮', async ({ page }) => {
    const docItem = page.locator('#docList [data-doc-name="audit-doc.md"]');
    await docItem.hover();
    // 审计按钮（🔍）应可见
    const auditBtn = docItem.locator('button[title="审计文档一致性"]');
    await expect(auditBtn).toBeVisible({ timeout: 5000 });
  });

  test.skip('E2E-AUDIT-001b 免费版不显示审计按钮（移至独立测试块）', async () => {});

  test('E2E-AUDIT-002 审计阶段事件驱动思考指示器文案', async ({ page }) => {
    const docItem = page.locator('#docList [data-doc-name="audit-doc.md"]');
    await docItem.hover();
    await docItem.locator('button[title="审计文档一致性"]').click();

    // 思考面板出现
    await expect(page.locator('.thinking-panel')).toBeVisible({ timeout: 3000 });
    // 思考文案：审计阶段文案或完成态（E2E_SPEED 加速时 mock 审计可能瞬时完成）
    await expect(page.locator('.thinking-panel-text')).toContainText(/审计|提取声明|比对矛盾|生成报告|思考完成/, { timeout: 5000 });

    // audit_phase 事件更新 #inputHint（inputHint 保留阶段文案，不被 setComplete 覆盖）
    await expect(page.locator('#inputHint')).toContainText(/提取声明|比对矛盾|生成报告/, { timeout: 8000 });
  });

  test('E2E-AUDIT-003 审计报告流式渲染为 Markdown', async ({ page }) => {
    const docItem = page.locator('#docList [data-doc-name="audit-doc.md"]');
    await docItem.hover();
    await docItem.locator('button[title="审计文档一致性"]').click();

    // 等待流式输出完成（sendBtn visible + 审计报告完整渲染）
    await waitDone(page, 20000);
    // 轮询等待审计报告完整（含「审计摘要」章节），替代固定 sleep
    const mdEl = page.locator('#chatArea .md').last();
    await expect(mdEl).toBeVisible();
    await expect
      .poll(async () => (await mdEl.innerText()).includes('审计摘要'), {
        timeout: 10000,
        message: '审计报告应流式渲染完整（含摘要章节）',
      })
      .toBe(true);
    const content = await mdEl.innerText();
    expect(content, '审计报告应含标题').toContain('审计报告');
    expect(content, '审计报告应含摘要').toContain('审计摘要');
  });

  test('E2E-AUDIT-004 审计报告含矛盾清单表格', async ({ page }) => {
    const docItem = page.locator('#docList [data-doc-name="audit-doc.md"]');
    await docItem.hover();
    await docItem.locator('button[title="审计文档一致性"]').click();

    // 等待审计完成（stop-mode 被移除表示完成）
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 20000 });

    // 审计报告应含 Markdown 表格
    const mdEl = page.locator('#chatArea .md').last();
    const table = mdEl.locator('table');
    await expect(table).toBeVisible({ timeout: 5000 });

    // 表格含矛盾数据行
    const rows = table.locator('tbody tr');
    const rowCount = await rows.count();
    expect(rowCount, '矛盾清单应有 2 行矛盾').toBe(2);

    // 含 contradiction 类型
    const tableText = await table.innerText();
    expect(tableText).toContain('contradiction');
  });

  test('E2E-AUDIT-005 审计取消 — stopBtn 调用 abort_audit', async ({ page }) => {
    const docItem = page.locator('#docList [data-doc-name="audit-doc.md"]');
    await docItem.hover();
    await docItem.locator('button[title="审计文档一致性"]').click();

    // 等待 sendBtn 进入 stop-mode（审计进行中，发送/停止合二为一）
    await expect(page.locator('#sendBtn.stop-mode')).toBeVisible({ timeout: 5000 });
    await page.waitForTimeout(500);

    // 通过 DOM 原生 click 触发 onclick 处理器（避免 toast 遮挡导致 click 未生效）
    await page.evaluate(() => document.getElementById('sendBtn').click());

    // 等待恢复空闲态
    await expect(page.locator('#sendBtn')).toBeVisible({ timeout: 10000 });

    // 审计模式标志应清除（stopBtn 在审计模式时调用 abort_audit）
    const auditAborted = await page.evaluate(() => window.__state.auditAborted);
    expect(auditAborted, 'abort_audit 应被调用').toBe(true);
  });

  test('E2E-AUDIT-006 审计中尝试发送消息被阻止', async ({ page }) => {
    const docItem = page.locator('#docList [data-doc-name="audit-doc.md"]');
    await docItem.hover();
    await docItem.locator('button[title="审计文档一致性"]').click();
    await expect(page.locator('#sendBtn.stop-mode')).toBeVisible({ timeout: 5000 });

    // 输入框应被禁用
    await expect(page.locator('#queryInput')).toBeDisabled();

    // 尝试强制发送（不应生效）
    await page.evaluate(() => {
      const input = document.getElementById('queryInput');
      input.value = '不应发送';
      const ev = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true });
      input.dispatchEvent(ev);
    });
    await page.waitForTimeout(300);

    // 不应出现用户消息块
    const userBlocks = await page.locator('#chatArea .flex.justify-end').count();
    expect(userBlocks, '审计中不应发送消息').toBe(0);
  });
});

test.describe('E2E-AUDIT-001b 免费版审计按钮', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // 不激活 Pro，保持免费版
    await importDocs(page, ['/mock/audit-doc.md']);
    await openKbModal(page);
  });

  test('E2E-AUDIT-001b 免费版不显示审计按钮', async ({ page }) => {
    const docItem = page.locator('#docList [data-doc-name="audit-doc.md"]');
    await docItem.hover();
    // 审计按钮应隐藏（isPro=false 时 display:none）
    const auditBtn = docItem.locator('button[title="审计文档一致性"]');
    await expect(auditBtn).toBeHidden();
  });
});
