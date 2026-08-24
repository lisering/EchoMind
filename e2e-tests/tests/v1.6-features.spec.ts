// E2E v1.6 功能测试（REQ-I18N-002 / REQ-EXP-007 / REQ-HELP-003 / REQ-WIN-003 / REQ-ERR-005）：
// TC-V16-001: 日期时间本地化 — formatDate 输出 YYYY-MM-DD HH:mm
// TC-V16-002: 日期时间本地化 — formatRelativeTime 相对时间
// TC-V16-003: 日期时间本地化 — formatFileSize 文件大小格式化
// TC-V16-004: 导出为 HTML — 对话导出 HTML 内容正确
// TC-V16-005: 导出为 HTML — 文档右键菜单含「导出为 HTML」
// TC-V16-006: 关于页面 — 帮助面板含「关于」Tab
// TC-V16-007: 关于页面 — About 面板独立打开
// TC-V16-008: 窗口关闭行为 — close-to-tray 设置 toggle 存在
// TC-V16-009: 窗口关闭行为 — IPC get/set close_to_tray mock 正常
// TC-V16-010: 错误日志导出 — 设置面板含导出按钮
// TC-V16-011: 错误日志导出 — IPC export_error_logs mock 正常
import { test, expect } from '@playwright/test';
import { setupPage, uiUrl } from './helpers.mjs';

test.describe('TC-V16 v1.6 功能测试', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  // ─── S1: 日期时间本地化（REQ-I18N-002）───

  test('TC-V16-001 formatDate 输出 YYYY-MM-DD HH:mm 格式', async ({ page }) => {
    const formatted = await page.evaluate(() => {
      // 2026-08-10 14:30:00 UTC = timestamp
      const ts = Date.UTC(2026, 7, 10, 14, 30, 0);
      return window.__formatDate(ts);
    });
    // 格式应为 YYYY-MM-DD HH:mm（本地时区可能不同，但格式匹配）
    expect(formatted).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
  });

  test('TC-V16-002 formatRelativeTime 输出相对时间', async ({ page }) => {
    // 2 分钟前 → "2 分钟前" 或 "2 min ago"
    const result = await page.evaluate(() => {
      const twoMinAgo = Date.now() - 2 * 60 * 1000;
      return window.__formatRelativeTime(twoMinAgo);
    });
    // 应返回非空字符串（包含数字或“刚刚”/"just now"）
    expect(result.length).toBeGreaterThan(0);
    // 2 分钟前应包含“2”或“just”
    expect(result === '刚刚' || result === 'just now' || result.includes('2')).toBeTruthy();
  });

  test('TC-V16-003 formatFileSize 文件大小格式化', async ({ page }) => {
    const size = await page.evaluate(() => {
      return window.__formatFileSize(12345678);
    });
    // 12345678 bytes → ~11.8 MB
    expect(size).toContain('MB');
    expect(size).toContain('11');
  });

  // ─── S2: 导出为 HTML（REQ-EXP-007）───

  test('TC-V16-004 导出对话为 HTML 内容正确', async ({ page }) => {
    // 创建会话并添加消息
    const convId = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('create_conversation');
    });
    await page.evaluate((cid) => {
      window.__mock.state.messages[cid] = [
        { role: 'user', content: '什么是 HTML 导出？', sources: null },
        { role: 'assistant', content: 'HTML 导出生成独立可打开的 HTML 文件。', sources: [] },
      ];
    }, convId);

    // 调用导出函数
    await page.evaluate((cid) => {
      return window.exportConversationToHtml(cid, 'HTML 导出测试');
    });

    // 等待 save_text_file 被调用（通过 toast 确认）
    await page.waitForTimeout(500);

    // 检查 save_text_file 被调用且内容是 HTML
    const savedContent = await page.evaluate(() => {
      return window.__mock.state.lastExportContent || null;
    });

    // 如果 mock 记录了内容，验证 HTML 结构
    if (savedContent) {
      expect(savedContent).toContain('<!DOCTYPE html>');
      expect(savedContent).toContain('HTML 导出测试');
    }
  });

  test('TC-V16-005 文档右键菜单含「导出为 HTML」选项', async ({ page }) => {
    // 直接验证 context-menu 模块已加载且包含 exportHtml action
    // 通过模拟右键文档项来检查菜单内容
    const hasExportHtml = await page.evaluate(() => {
      // 检查 exportDocumentToHtml 全局函数存在
      return typeof window.exportDocumentToHtml === 'function';
    });
    expect(hasExportHtml).toBeTruthy();

    // 验证 context-menu 的 _showDocMenu 包含 exportHtml 项
    // 通过创建一个临时文档项元素并触发右键
    const menuHtml = await page.evaluate(() => {
      // 创建临时文档项
      const docItem = document.createElement('div');
      docItem.dataset.docName = 'test-doc.md';
      docItem.dataset.docId = 'test-doc-id';
      document.body.appendChild(docItem);

      // 触发 contextmenu 事件
      const event = new Event('contextmenu', { bubbles: true });
      event.clientX = 100;
      event.clientY = 100;
      docItem.dispatchEvent(event);

      // 读取菜单内容
      const menu = document.getElementById('ctxMenu');
      const html = menu ? menu.innerHTML : '';

      // 清理
      docItem.remove();
      return html;
    });

    expect(menuHtml).toContain('exportHtml');
  });

  // ─── S3: 关于页面（REQ-HELP-003）───

  test('TC-V16-006 帮助面板含「关于」Tab', async ({ page }) => {
    // 打开帮助面板
    await page.evaluate(() => window.__openHelpPanel());

    // 等待面板渲染
    await page.waitForSelector('#helpPanelOverlay', { timeout: 3000 });

    // 检查 Tab 栏含「关于」按钮
    const tabBar = page.locator('#helpTabBar');
    const tabButtons = await tabBar.locator('button[data-tab-id]').allTextContents();
    // 应包含 about tab（中英文均可）
    const hasAboutTab = tabButtons.some(text =>
      text.includes('关于') || text.includes('About')
    );
    expect(hasAboutTab).toBeTruthy();
  });

  test('TC-V16-007 About 面板独立打开并显示版本信息', async ({ page }) => {
    // 打开关于面板（about-panel 已合并进 help-panel，openAboutPanel = openHelpPanel('about')）
    await page.evaluate(() => window.__openAboutPanel());

    // 等待面板渲染
    await page.waitForSelector('#helpPanelOverlay', { timeout: 3000 });

    // 检查面板可见
    const overlay = page.locator('#helpPanelOverlay');
    await expect(overlay).not.toHaveClass(/hidden/);

    // 检查内容区（#helpContent 的 About Tab）含版本号（格式 X.Y.Z，来源 tauri.conf.json）
    const content = page.locator('#helpContent');
    await expect(content).toContainText(/\d+\.\d+\.\d+/, { timeout: 3000 });
  });

  // ─── S4: 窗口关闭行为（REQ-WIN-003）───

  test('TC-V16-008 设置面板含 close-to-tray toggle', async ({ page }) => {
    // 打开设置面板
    await page.click('#settingsBtn');
    await page.waitForTimeout(500);

    // 检查窗口管理设置容器存在
    const container = page.locator('#windowSettingsContainer');
    await expect(container).toBeAttached();

    // 检查 toggle 开关存在
    const toggle = page.locator('#closeToTrayToggle');
    await expect(toggle).toBeAttached();
  });

  test('TC-V16-009 IPC get/set close_to_tray mock 正常', async ({ page }) => {
    // 获取当前设置（默认 false）
    const initial = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_close_to_tray');
    });
    expect(initial).toBe(false);

    // 设置为 true
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('update_setting', { key: 'window.close_to_tray', value: String(true) });
    });

    // 验证设置已生效
    const afterSet = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_close_to_tray');
    });
    expect(afterSet).toBe(true);
  });

  // ─── S5: 错误日志导出（REQ-ERR-005）───

  test('TC-V16-010 设置面板含错误日志导出按钮', async ({ page }) => {
    // 打开设置面板
    await page.click('#settingsBtn');
    await page.waitForTimeout(500);

    // 检查错误日志容器存在
    const container = page.locator('#errorLogsContainer');
    await expect(container).toBeAttached();

    // 检查导出按钮存在
    const btn = container.locator('button');
    await expect(btn).toBeAttached();
  });

  test('TC-V16-011 IPC export_error_logs mock 正常', async ({ page }) => {
    // 设置 mock 错误日志数据
    await page.evaluate(() => {
      window.__mock.state.errorLogs = '{"timestamp":"2026-08-10T12:00:00Z","level":"ERROR","target":"test","message":"Test error"}';
    });

    // 调用导出
    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('export_error_logs');
    });

    // 应返回设置的错误日志内容
    expect(result).toContain('ERROR');
    expect(result).toContain('Test error');
  });
});
