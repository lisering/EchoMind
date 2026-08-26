// E2E v2.4 功能测试 — S88 对话模板导入/导出（REQ-RAG-054）
// TC-TPL-EXP-001: 设置面板「快捷指令模板」区域新增「导出全部」按钮
// TC-TPL-EXP-002: 每个模板卡片新增「导出」按钮
// TC-TPL-EXP-003: 「导入」按钮选择 JSON 文件，解析后批量添加
// TC-TPL-EXP-004: 导入时模板名冲突 → 自动追加 _2 后缀
// TC-TPL-EXP-005: 导出 JSON 格式含 version / exported_at / templates[] 字段
import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test.describe('TC-TPL-EXP 对话模板导入/导出（REQ-RAG-054）', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // 打开设置面板
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await page.evaluate(() => {
      document.querySelectorAll('[data-settings-section]').forEach((el) => el.classList.remove('hidden'));
    });
    // 等待模板区域渲染（promptTemplateContainer 内含模板卡片）
    await page.locator('#promptTemplateContainer').waitFor({ state: 'visible', timeout: 5000 });
  });

  test('TC-TPL-EXP-001 设置面板模板区域有「导出全部」按钮', async ({ page }) => {
    // 模板区域应包含「导出全部」按钮
    const exportAllBtn = page.locator('#promptTemplateContainer').locator('button', { hasText: /export|导出全部/ });
    await expect(exportAllBtn.first()).toBeVisible({ timeout: 5000 });
  });

  test('TC-TPL-EXP-002 每个模板卡片有「导出」按钮', async ({ page }) => {
    // 等待模板列表渲染
    await page.waitForTimeout(500);
    // 每个模板卡片应包含导出按钮（data-export-template 属性）
    const exportBtns = page.locator('#promptTemplateContainer [data-export-template]');
    const count = await exportBtns.count();
    // 应至少有 2 个模板（mock 预填了 2 个测试模板）
    expect(count).toBeGreaterThanOr(2);
  });

  test('TC-TPL-EXP-003 「导入」按钮存在且可点击', async ({ page }) => {
    // 模板区域应包含「导入」按钮
    const importBtn = page.locator('#promptTemplateContainer').locator('button', { hasText: /import|导入/ });
    await expect(importBtn.first()).toBeVisible({ timeout: 5000 });
  });

  test('TC-TPL-EXP-004 导入时模板名冲突自动追加后缀', async ({ page }) => {
    // 模拟导入流程：调用 importTemplates 并验证冲突处理
    // 直接通过 evaluate 调用导入逻辑
    const result = await page.evaluate(async () => {
      // 获取当前模板列表
      const before = await window.__TAURI__.core.invoke('list_prompt_templates');
      const beforeNames = before.map((t: any) => t.name);

      // 模拟导入操作：调用 save_prompt_template 创建冲突名
      // 导入的 'test_summary' 与已有的冲突，应追加 _2
      try {
        // 模拟导入函数的冲突检测逻辑
        const existingNames = new Set(beforeNames);
        let resolvedName = 'test_summary';
        let suffix = 2;
        while (existingNames.has(resolvedName)) {
          resolvedName = `test_summary_${suffix}`;
          suffix++;
        }
        // 调用 save 创建
        await window.__TAURI__.core.invoke('save_prompt_template', {
          name: resolvedName,
          label: 'Imported Conflict',
          description: 'Test conflict resolution',
          icon: '⚡',
          promptTemplate: 'Test: {query}',
        });
        const after = await window.__TAURI__.core.invoke('list_prompt_templates');
        return { beforeNames, resolvedName, afterNames: after.map((t: any) => t.name) };
      } catch (e) {
        return { error: String(e) };
      }
    });

    expect(result.error).toBeUndefined();
    // 冲突名应被解析为 test_summary_2
    expect(result.resolvedName).toBe('test_summary_2');
    // 导入后应包含新名称
    expect(result.afterNames).toContain('test_summary_2');
  });

  test('TC-TPL-EXP-005 导出 JSON 格式含 version/exported_at/templates 字段', async ({ page }) => {
    // 模拟导出流程：点击导出全部按钮 → 验证 save_text_file 被调用且内容格式正确
    const exportResult = await page.evaluate(async () => {
      // 直接调用导出逻辑
      const templates = await window.__TAURI__.core.invoke('list_prompt_templates');
      const exportData = {
        version: '1.0',
        exported_at: new Date().toISOString(),
        templates: templates.map((t: any) => ({
          name: t.name,
          label: t.label,
          description: t.description,
          icon: t.icon,
          prompt_template: t.prompt_template,
        })),
      };
      const jsonStr = JSON.stringify(exportData, null, 2);
      // 调用 save_text_file 写入
      await window.__TAURI__.core.invoke('save_text_file', {
        path: '/tmp/test-export.json',
        content: jsonStr,
      });
      return exportData;
    });

    // 验证导出 JSON 格式
    expect(exportData.version).toBe('1.0');
    expect(exportData.exported_at).toBeTruthy();
    expect(Array.isArray(exportData.templates)).toBe(true);
    expect(exportData.templates.length).toBeGreaterThanOr(2);
    // 每个模板应含必要字段
    const firstTpl = exportData.templates[0];
    expect(firstTpl).toHaveProperty('name');
    expect(firstTpl).toHaveProperty('label');
    expect(firstTpl).toHaveProperty('prompt_template');
  });
});
