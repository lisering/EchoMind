// E2E 文件大小限制与警告（REQ-ING-013）。
// AC-1: 导入 >100MB 文件时弹出警告对话框
// AC-2: 导入 >500MB 文件时直接拒绝
// AC-3: 警告对话框可取消
// AC-4: 批量导入时每个超限文件分别处理
//
// 注意：必须通过 simulateDragDrop 触发前端 importPaths() 流程，
// 这样才会经过 checkFileSizes() 文件大小检查。
// 直接调用 invoke('import_files') 会绕过前端校验逻辑。
import { test, expect } from '@playwright/test';
import { setupPage, openKbModal } from './helpers.mjs';

test.describe('E2E-ING-013 文件大小限制与警告', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('AC-1 大文件(>100MB)导入前弹出警告对话框', async ({ page }) => {
    // 通过拖拽模拟触发前端 importPaths → checkFileSizes 流程
    // mock 中 'large' → 150MB（>100MB 警告阈值）
    await page.evaluate((paths) => {
      window.__mock.simulateDragDrop(paths);
    }, ['/mock/large-doc.md']);

    // 警告对话框应出现
    await expect(page.locator('#fileSizeWarningDialog')).toBeVisible({ timeout: 5000 });
    // 对话框中应显示文件名
    await expect(page.locator('#fileSizeWarningList')).toContainText('large-doc.md');
    // 对话框中应显示文件大小（formatBytes 返回 "150.0 MB" 格式）
    await expect(page.locator('#fileSizeWarningList')).toContainText(/150\.?\d*\s*MB/i);
    // 应有确认和取消按钮
    await expect(page.locator('#fileSizeWarningOk')).toBeVisible();
    await expect(page.locator('#fileSizeWarningCancel')).toBeVisible();
  });

  test('AC-2 超大文件(>500MB)直接拒绝', async ({ page }) => {
    // 通过拖拽模拟触发前端 importPaths → checkFileSizes 流程
    // mock 中 'huge' → 600MB（>500MB 硬上限）
    await page.evaluate((paths) => {
      window.__mock.simulateDragDrop(paths);
    }, ['/mock/huge-doc.md']);

    // 不弹出警告对话框（直接拒绝）
    await expect(page.locator('#fileSizeWarningDialog')).toBeHidden({ timeout: 3000 });
    // 应有错误 toast 提示文件过大
    await expect(page.locator('#toasts')).toContainText(/too large|过大|500\s*MB/i, { timeout: 5000 });
  });

  test('AC-3 警告对话框可取消', async ({ page }) => {
    // 导入大文件（>100MB 触发警告对话框）
    await page.evaluate((paths) => {
      window.__mock.simulateDragDrop(paths);
    }, ['/mock/large-doc.md']);

    await expect(page.locator('#fileSizeWarningDialog')).toBeVisible({ timeout: 5000 });

    // 点击取消
    await page.locator('#fileSizeWarningCancel').click();
    await expect(page.locator('#fileSizeWarningDialog')).toBeHidden({ timeout: 3000 });

    // 文档不应被导入 — 打开 KB Modal 检查
    await openKbModal(page);
    const docs = page.locator('#docList [data-doc-name]');
    await expect(docs).toHaveCount(0);
  });

  test('AC-4 批量导入时每个超限文件分别处理', async ({ page }) => {
    // 同时导入正常文件 + 大文件 + 超大文件
    // normal-doc.md → 1MB（正常）
    // large-doc.md → 150MB（>100MB 警告）
    // huge-doc.md → 600MB（>500MB 拒绝）
    await page.evaluate((paths) => {
      window.__mock.simulateDragDrop(paths);
    }, ['/mock/normal-doc.md', '/mock/large-doc.md', '/mock/huge-doc.md']);

    // 警告对话框应出现（large-doc.md > 100MB）
    await expect(page.locator('#fileSizeWarningDialog')).toBeVisible({ timeout: 5000 });
    // 对话框应只显示警告文件（large-doc.md），不含 huge-doc.md（已直接拒绝）和 normal-doc.md（正常）
    await expect(page.locator('#fileSizeWarningList')).toContainText('large-doc.md');
    await expect(page.locator('#fileSizeWarningList')).not.toContainText('huge-doc.md');
    await expect(page.locator('#fileSizeWarningList')).not.toContainText('normal-doc.md');

    // 确认导入大文件
    await page.locator('#fileSizeWarningOk').click();
    await expect(page.locator('#fileSizeWarningDialog')).toBeHidden({ timeout: 3000 });

    // 等待导入完成（normal-doc.md + large-doc.md 进入 import_files 流程）
    await page.locator('#docList [data-doc-name]').first().waitFor({ state: 'attached', timeout: 5000 });

    // 打开 KB Modal 检查结果
    await openKbModal(page);
    const docs = page.locator('#docList [data-doc-name]');
    // 应有 2 个文档（normal-doc.md + large-doc.md），huge-doc.md 被拒绝
    await expect(docs).toHaveCount(2);

    const names = await docs.evaluateAll((els) => els.map((el) => el.dataset.docName));
    expect(names).toContain('normal-doc.md');
    expect(names).toContain('large-doc.md');
    expect(names).not.toContain('huge-doc.md');
  });
});
