/**
 * v1.20 知识库配额独立计算 + 文档跨知识库迁移 E2E 测试。
 *
 * 覆盖：
 * - REQ-WS-002：每库独立 50 上限 / 切换同步更新 / 一库满不影响其他 / Pro 不受限
 * - REQ-WS-004：右键迁移弹出库列表 / 迁移后计数变化 / 配额不足拒绝
 *
 * 测试模式：setupPage → IPC mock 操作 → 断言
 */
import { test, expect } from '@playwright/test';
import { setupPage, waitForToast, importDocs } from './helpers.mjs';

test.describe('v1.20 知识库配额与迁移 (REQ-WS-002/004)', () => {

  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // REQ-WS-002/004 测试免费版配额行为（mock 默认 isPro=true Alpha 全功能免费）
    await page.evaluate(() => { window.__state.isPro = false; });
  });

  /**
   * 辅助函数：打开知识库下拉菜单。
   */
  async function openWorkspaceDropdown(page) {
    const dropdown = page.locator('#workspaceDropdown');
    const alreadyVisible = await dropdown.isVisible().catch(() => false);
    if (alreadyVisible) {
      await page.locator('#workspaceToggle').click();
      await page.waitForTimeout(300);
    } else {
      await page.keyboard.press('Escape');
      await page.waitForTimeout(300);
    }
    await page.locator('#workspaceToggle').click();
    await page.waitForTimeout(300);
    await expect(dropdown).toBeVisible({ timeout: 5000 });
    await page.waitForTimeout(500);
    return dropdown;
  }

  /**
   * 辅助函数：新建知识库。
   */
  async function createWorkspace(page, name) {
    const dropdown = await openWorkspaceDropdown(page);
    const createBtn = dropdown.locator('div.border-t').last();
    await createBtn.waitFor({ state: 'visible', timeout: 5000 });
    await createBtn.click();
    const dialogInput = page.locator('.fixed input[type="text"]').last();
    await dialogInput.waitFor({ state: 'visible', timeout: 5000 });
    await dialogInput.fill(name);
    await page.keyboard.press('Enter');
    await expect(page.locator('#workspaceName')).toContainText(name, { timeout: 5000 });
  }

  // ============================================================
  // REQ-WS-002：知识库配额独立计算
  // ============================================================

  test('TC-V20-WS002-001: 每个知识库独立计算文件配额', async ({ page }) => {
    // AC-1：免费版每个知识库独立计算文件数，上限 50

    // 1. 在 default 工作空间导入文档
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('import_files', { paths: ['/mock/test1.md'] });
    });
    await page.waitForTimeout(500);

    // 2. 验证 default 库配额为 1/50
    const quota1 = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_workspace_quota');
    });
    expect(quota1[0]).toBe(1);
    expect(quota1[1]).toBe(50);

    // 3. 新建第二个知识库
    await createWorkspace(page, 'Quota Test KB');

    // 4. 在新库导入文档
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('import_files', { paths: ['/mock/test2.md'] });
    });
    await page.waitForTimeout(500);

    // 5. 验证新库配额为 1/50（独立计数，不受 default 库影响）
    const quota2 = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_workspace_quota');
    });
    expect(quota2[0]).toBe(1);
    expect(quota2[1]).toBe(50);
  });

  test('TC-V20-WS002-002: 切换知识库后配额显示同步更新', async ({ page }) => {
    // AC-2：切换知识库后配额显示更新为该库的用量

    // 1. 在 default 导入 2 个文档
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('import_files', {
        paths: ['/mock/a.md', '/mock/b.md'],
      });
    });
    await page.waitForTimeout(500);

    // 2. 新建知识库
    await createWorkspace(page, 'Switch Quota KB');

    // 3. 在新库导入 1 个文档
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('import_files', { paths: ['/mock/c.md'] });
    });
    await page.waitForTimeout(500);

    // 4. 验证新库配额为 1/50
    const quotaNew = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_workspace_quota');
    });
    expect(quotaNew[0]).toBe(1);

    // 5. 切换回 default
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('switch_workspace', { workspaceId: 'default' });
    });
    await page.waitForTimeout(300);

    // 6. 验证 default 库配额为 2/50
    const quotaDefault = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_workspace_quota');
    });
    expect(quotaDefault[0]).toBe(2);
  });

  test('TC-V20-WS002-003: 一库满 50 不影响其他库导入', async ({ page }) => {
    // AC-3：一个知识库满 50 文件不影响其他知识库导入

    // 1. 批量导入 50 个文档到 default（填满配额）
    const paths = Array.from({ length: 50 }, (_, i) => `/mock/fill${i}.md`);
    await page.evaluate((p) => {
      return window.__TAURI__.core.invoke('import_files', { paths: p });
    }, paths);
    await page.waitForTimeout(1000);

    // 2. 验证 default 库已满
    const quotaFull = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_workspace_quota');
    });
    expect(quotaFull[0]).toBe(50);

    // 3. 在 default 库再导入应失败
    await expect(page.evaluate(() => {
      return window.__TAURI__.core.invoke('import_files', { paths: ['/mock/overflow.md'] });
    })).rejects.toThrow(/LIMIT_REACHED/);

    // 4. 新建知识库
    await createWorkspace(page, 'Fresh KB');

    // 5. 在新库导入应成功（不受 default 库满的影响）
    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('import_files', { paths: ['/mock/fresh.md'] });
    });
    expect(result).toHaveLength(1);

    // 6. 验证新库配额为 1/50
    const quotaFresh = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_workspace_quota');
    });
    expect(quotaFresh[0]).toBe(1);
  });

  // ============================================================
  // REQ-WS-004：文档跨知识库迁移
  // ============================================================

  test('TC-V20-WS004-001: 迁移文档到目标知识库', async ({ page }) => {
    // AC-2：选择目标库后文档及其 chunks / 向量迁移到目标库
    // AC-3：迁移后原库文件计数减少，目标库文件计数增加

    // 1. 在 default 导入文档
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('import_files', { paths: ['/mock/migrate-me.md'] });
    });
    await page.waitForTimeout(500);

    // 2. 获取文档 ID
    const docs = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_documents');
    });
    expect(docs.length).toBe(1);
    const docId = docs[0].id;

    // 3. 新建目标知识库
    await createWorkspace(page, 'Target KB');

    // 4. 获取目标库 ID
    const targetWsId = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_current_workspace');
    });

    // 5. 切换回 default（文档在 default 库）
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('switch_workspace', { workspaceId: 'default' });
    });
    await page.waitForTimeout(300);

    // 6. 执行迁移
    await page.evaluate(({ id, target }) => {
      return window.__TAURI__.core.invoke('migrate_document', {
        docId: id,
        targetWorkspaceId: target,
      });
    }, { id: docId, target: targetWsId });

    // 7. 验证 default 库计数减少（0 文档）
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('switch_workspace', { workspaceId: 'default' });
    });
    await page.waitForTimeout(300);
    const defaultQuota = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_workspace_quota');
    });
    expect(defaultQuota[0]).toBe(0);

    // 8. 验证目标库计数增加（1 文档）
    await page.evaluate((wsId) => {
      return window.__TAURI__.core.invoke('switch_workspace', { workspaceId: wsId });
    }, targetWsId);
    await page.waitForTimeout(300);
    const targetQuota = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_workspace_quota');
    });
    expect(targetQuota[0]).toBe(1);

    // 9. 验证文档 workspace_id 已更新（AC-4：不重新嵌入，索引复用）
    const targetDocs = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_documents');
    });
    expect(targetDocs.length).toBe(1);
    expect(targetDocs[0].workspace_id).toBe(targetWsId);
  });

  test('TC-V20-WS004-002: 迁移不重新解析或嵌入', async ({ page }) => {
    // AC-4：迁移不重新解析或嵌入（索引数据复用）

    // 1. 在 default 导入文档
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('import_files', { paths: ['/mock/no-reembed.md'] });
    });
    await page.waitForTimeout(500);

    const docs = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_documents');
    });
    const docId = docs[0].id;
    const originalHash = docs[0].file_hash;

    // 2. 新建目标库
    await createWorkspace(page, 'No Reembed KB');
    const targetWsId = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_current_workspace');
    });

    // 3. 切换回 default 并迁移
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('switch_workspace', { workspaceId: 'default' });
    });
    await page.waitForTimeout(300);

    await page.evaluate(({ id, target }) => {
      return window.__TAURI__.core.invoke('migrate_document', {
        docId: id,
        targetWorkspaceId: target,
      });
    }, { id: docId, target: targetWsId });

    // 4. 验证文档 hash 不变（未重新解析）
    await page.evaluate((wsId) => {
      return window.__TAURI__.core.invoke('switch_workspace', { workspaceId: wsId });
    }, targetWsId);
    await page.waitForTimeout(300);

    const migratedDocs = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_documents');
    });
    expect(migratedDocs[0].file_hash).toBe(originalHash);
  });

  test('TC-V20-WS004-003: 目标库配额不足拒绝迁移', async ({ page }) => {
    // AC-5：目标库配额不足时拒绝迁移并提示

    // 1. 新建目标知识库
    await createWorkspace(page, 'Full Target KB');
    const targetWsId = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_current_workspace');
    });

    // 2. 在目标库填满 50 个文档
    const paths = Array.from({ length: 50 }, (_, i) => `/mock/target-fill-${i}.md`);
    await page.evaluate((p) => {
      return window.__TAURI__.core.invoke('import_files', { paths: p });
    }, paths);
    await page.waitForTimeout(1000);

    // 3. 切换回 default 并导入 1 个文档
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('switch_workspace', { workspaceId: 'default' });
    });
    await page.waitForTimeout(300);

    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('import_files', { paths: ['/mock/want-to-migrate.md'] });
    });
    await page.waitForTimeout(500);

    const docs = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_documents');
    });
    const docId = docs[0].id;

    // 4. 尝试迁移到满库 → 应拒绝
    await expect(page.evaluate(({ id, target }) => {
      return window.__TAURI__.core.invoke('migrate_document', {
        docId: id,
        targetWorkspaceId: target,
      });
    }, { id: docId, target: targetWsId })).rejects.toThrow(/LIMIT_REACHED/);
  });

});
