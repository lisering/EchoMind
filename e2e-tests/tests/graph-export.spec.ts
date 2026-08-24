/**
 * E2E 测试：知识图谱导出（TC-EXP-GRAPH-004~005）。
 *
 * 验证 REQ-EXP-006 知识图谱导出功能：
 * - 图谱面板含「导出 GraphML」和「导出 JSON-LD」按钮
 * - 点击导出按钮触发 Blob 下载（mock URL.createObjectURL）
 */

import { test, expect } from '@playwright/test';
import { setupPage, clickToolButton } from './helpers.mjs';

test.describe('知识图谱导出 (TC-EXP-GRAPH)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // 等待工具按钮渲染（S5 P1-1：graphBtn 收纳到工具下拉菜单）
    await page.waitForSelector('#toolsBtn', { timeout: 5000 });
    // 打开图谱面板
    await clickToolButton(page, 'graphBtn');
    await expect(page.locator('#graphOverlay')).toBeVisible({ timeout: 5000 });
  });

  test('TC-EXP-GRAPH-004: 导出按钮存在 — 图谱面板含「导出 GraphML」和「导出 JSON-LD」按钮', async ({ page }) => {
    // GraphML 导出按钮
    const graphmlBtn = page.locator('#graphExportGraphml');
    await expect(graphmlBtn).toBeVisible({ timeout: 5000 });
    await expect(graphmlBtn).toBeEnabled();

    // 验证按钮有 aria-label（非空）
    const graphmlAria = await graphmlBtn.getAttribute('aria-label');
    expect(graphmlAria).not.toBe('');
    expect(graphmlAria!.length).toBeGreaterThan(0);

    // JSON-LD 导出按钮
    const jsonldBtn = page.locator('#graphExportJsonld');
    await expect(jsonldBtn).toBeVisible({ timeout: 5000 });
    await expect(jsonldBtn).toBeEnabled();

    // 验证按钮有 aria-label（非空）
    const jsonldAria = await jsonldBtn.getAttribute('aria-label');
    expect(jsonldAria).not.toBe('');
    expect(jsonldAria!.length).toBeGreaterThan(0);
  });

  test('TC-EXP-GRAPH-005: 点击导出触发下载 — mock URL.createObjectURL 被调用，文件扩展名正确', async ({ page }) => {
    // 在页面中注入 mock 来追踪 createObjectURL 和下载文件名
    await page.evaluate(() => {
      (window as any).__exportCalls = [];
      const origCreate = URL.createObjectURL;
      URL.createObjectURL = function (blob: Blob) {
        const url = origCreate.call(URL, blob);
        (window as any).__exportCalls.push({
          type: blob.type,
          size: blob.size,
        });
        return url;
      };
      // mock anchor click to capture download filename
      const origClick = HTMLElement.prototype.click;
      HTMLElement.prototype.click = function () {
        if (this.tagName === 'A' && this.download) {
          (window as any).__exportCalls.push({
            filename: this.download,
          });
        }
        return origClick.call(this);
      };
    });

    // 点击 GraphML 导出按钮
    await page.locator('#graphExportGraphml').click();

    // 等待导出完成
    await page.waitForTimeout(500);

    const graphmlCalls = await page.evaluate(() => (window as any).__exportCalls);
    expect(graphmlCalls.length).toBeGreaterThanOrEqual(1);

    // 验证有 GraphML 文件名
    const graphmlFilenames = graphmlCalls.filter((c: any) => c.filename);
    expect(graphmlFilenames.length).toBeGreaterThanOrEqual(1);
    expect(graphmlFilenames[0].filename).toBe('knowledge-graph.graphml');

    // 重置调用记录
    await page.evaluate(() => {
      (window as any).__exportCalls = [];
    });

    // 点击 JSON-LD 导出按钮
    await page.locator('#graphExportJsonld').click();

    // 等待导出完成
    await page.waitForTimeout(500);

    const jsonldCalls = await page.evaluate(() => (window as any).__exportCalls);
    expect(jsonldCalls.length).toBeGreaterThanOrEqual(1);

    // 验证有 JSON-LD 文件名
    const jsonldFilenames = jsonldCalls.filter((c: any) => c.filename);
    expect(jsonldFilenames.length).toBeGreaterThanOrEqual(1);
    expect(jsonldFilenames[0].filename).toBe('knowledge-graph.jsonld');
  });
});
