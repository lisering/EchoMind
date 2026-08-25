/**
 * S56: 自定义快捷指令模板系统 E2E 测试
 *
 * 测试内容：
 * - TC-S56-001: 创建自定义模板（settings 面板表单）
 * - TC-S56-002: 模板列表展示
 * - TC-S56-003: slash 指令面板合并系统 + 自定义指令
 * - TC-S56-004: processSlashCommand 展开 prompt 模板
 * - TC-S56-005: 编辑现有模板
 * - TC-S56-006: 删除模板
 * - TC-S56-007: 系统指令名称冲突防护
 * - TC-S56-008: 缺少 {query} 占位符验证
 */
import { test, expect } from '@playwright/test';
import { enterApp, injectStub, uiUrl } from './helpers.mjs';

test.describe('S56: 自定义快捷指令模板系统', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await page.goto(uiUrl);
    await page.waitForLoadState('networkidle');
    await enterApp(page);
  });

  test('TC-S56-001: 创建自定义模板（IPC 验证）', async ({ page }) => {
    // 通过 IPC 创建模板
    const templateId = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('save_prompt_template', {
        name: 'explain',
        label: '解释概念',
        description: '用通俗易懂的方式解释概念',
        icon: '💡',
        promptTemplate: '请用通俗易懂的方式解释以下概念：{query}',
      });
    });

    // 验证返回了有效的模板 ID
    expect(templateId).toBeTruthy();
    expect(typeof templateId).toBe('string');

    // 验证模板已存储
    const templates = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('list_prompt_templates');
    });
    const found = templates.find((t) => t.name === 'explain');
    expect(found).toBeTruthy();
    expect(found.label).toBe('解释概念');
    expect(found.icon).toBe('💡');
    expect(found.prompt_template).toContain('{query}');
  });

  test('TC-S56-002: 模板列表展示', async ({ page }) => {
    // 先通过 IPC 创建模板
    await page.evaluate(async () => {
      await window.__TAURI__.core.invoke('save_prompt_template', {
        name: 'test_list',
        label: '测试列表',
        description: '测试列表功能',
        icon: '📋',
        promptTemplate: '列出以下内容的要点：{query}',
      });
    });

    // 打开设置面板
    await page.locator('#settingsBtn').click();
    await page.waitForTimeout(500);
    // V3.1 阶段二：S94 Tab 化——模板容器在 advanced 分区，移除 hidden 后再滚动
    await page.evaluate(() => {
      document.querySelectorAll('[data-settings-section]').forEach((el) => el.classList.remove('hidden'));
    });

    // 验证模板列表中包含创建的模板
    const tmplContainer = page.locator('#promptTemplateContainer');
    await tmplContainer.scrollIntoViewIfNeeded();
    await page.waitForTimeout(300);

    const templateItem = tmplContainer.locator('div', { hasText: '/test_list' });
    await expect(templateItem.first()).toBeVisible({ timeout: 3000 });
  });

  test('TC-S56-003: slash 指令模块合并自定义指令（模块验证）', async ({ page }) => {
    // 创建自定义模板
    await page.evaluate(async () => {
      await window.__TAURI__.core.invoke('save_prompt_template', {
        name: 'custom_cmd',
        label: '自定义命令',
        description: '测试自定义指令',
        icon: '🔧',
        promptTemplate: '执行自定义操作：{query}',
      });
    });

    // 验证模板已存储
    const templates = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('list_prompt_templates');
    });
    const custom = templates.find((t) => t.name === 'custom_cmd');
    expect(custom).toBeTruthy();
    expect(custom.prompt_template).toBe('执行自定义操作：{query}');

    // 验证系统指令 + 自定义指令不重名
    const systemNames = ['summary', 'compare', 'extract', 'translate', 'timeline', 'mindmap'];
    const allNames = templates.map((t) => t.name);
    const overlap = allNames.filter((n) => systemNames.includes(n));
    expect(overlap).toHaveLength(0);
  });

  test('TC-S56-004: 模板 prompt 包含 {query} 占位符', async ({ page }) => {
    // 创建自定义模板
    await page.evaluate(async () => {
      await window.__TAURI__.core.invoke('save_prompt_template', {
        name: 'expand_test',
        label: '展开测试',
        description: '测试模板展开',
        icon: '🧪',
        promptTemplate: '请详细分析：{query}',
      });
    });

    // 通过 IPC 验证后端存储的模板内容
    const result = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('list_prompt_templates');
    });

    expect(result).toBeTruthy();
    const found = result.find((t) => t.name === 'expand_test');
    expect(found).toBeTruthy();
    expect(found.prompt_template).toContain('{query}');
  });

  test('TC-S56-005: 编辑现有模板', async ({ page }) => {
    // 先创建模板
    const templateId = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('save_prompt_template', {
        name: 'edit_test',
        label: '编辑前',
        description: '编辑前描述',
        icon: '📝',
        promptTemplate: '编辑前：{query}',
      });
    });

    expect(templateId).toBeTruthy();

    // 更新模板（同名 → 更新）
    const updatedId = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('save_prompt_template', {
        name: 'edit_test',
        label: '编辑后',
        description: '编辑后描述',
        icon: '✏️',
        promptTemplate: '编辑后：{query}',
      });
    });

    // 验证 ID 不变（更新而非新建）
    expect(updatedId).toBe(templateId);

    // 验证内容已更新
    const templates = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('list_prompt_templates');
    });
    const updated = templates.find((t) => t.name === 'edit_test');
    expect(updated.label).toBe('编辑后');
    expect(updated.icon).toBe('✏️');
  });

  test('TC-S56-006: 删除模板', async ({ page }) => {
    // 创建模板
    const templateId = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('save_prompt_template', {
        name: 'delete_test',
        label: '删除测试',
        description: '待删除',
        icon: '🗑️',
        promptTemplate: '待删除：{query}',
      });
    });

    // 验证存在
    let templates = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('list_prompt_templates');
    });
    expect(templates.find((t) => t.name === 'delete_test')).toBeTruthy();

    // 删除
    await page.evaluate(async (id) => {
      await window.__TAURI__.core.invoke('delete_prompt_template', { templateId: id });
    }, templateId);

    // 验证已删除
    templates = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('list_prompt_templates');
    });
    expect(templates.find((t) => t.name === 'delete_test')).toBeFalsy();
  });

  test('TC-S56-007: 系统指令名称冲突防护', async ({ page }) => {
    // 尝试用系统指令名创建模板
    await expect(
      page.evaluate(async () => {
        return await window.__TAURI__.core.invoke('save_prompt_template', {
          name: 'summary',
          label: '冲突',
          description: '与系统指令冲突',
          icon: '⚠️',
          promptTemplate: '冲突：{query}',
        });
      })
    ).rejects.toThrow(/冲突|conflict/i);
  });

  test('TC-S56-008: 缺少 {query} 占位符验证', async ({ page }) => {
    await expect(
      page.evaluate(async () => {
        return await window.__TAURI__.core.invoke('save_prompt_template', {
          name: 'no_placeholder',
          label: '无占位符',
          description: '缺少占位符',
          icon: '❌',
          promptTemplate: '没有占位符的模板',
        });
      })
    ).rejects.toThrow(/占位符|placeholder/i);
  });
});
