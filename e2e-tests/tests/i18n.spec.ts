// E2E 国际化功能（REQ-I18N-001~003）：
// E2E-I18N-001: 默认语言为 zh-CN
// E2E-I18N-002: 切换语言为 en
// E2E-I18N-003: 切换语言后 get_locale 返回新值
// E2E-I18N-004: 语言持久化——set_locale 后 get_locale 恢复
// E2E-I18N-005: 切换回 zh-CN
// E2E-I18N-006: 无效语言代码——仍设置成功（后端不校验）
// E2E-I18N-007: 语言切换不丢失其他状态
// E2E-I18N-008: 初始 locale 状态
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-I18N 国际化功能（REQ-I18N-001~003）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── 基本语言操作 ───

  test('E2E-I18N-001 默认语言为 zh-CN', async ({ page }) => {
    const locale = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_locale')
    );
    expect(locale).toBe('zh-CN');
  });

  test('E2E-I18N-002 切换语言为 en', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_locale', { locale: 'en' })
    );
    const locale = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_locale')
    );
    expect(locale).toBe('en');
  });

  test('E2E-I18N-003 切换语言后 get_locale 返回新值', async ({ page }) => {
    // 先设置为 en
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_locale', { locale: 'en' })
    );
    expect(await page.evaluate(() => window.__TAURI__.core.invoke('get_locale'))).toBe('en');

    // 再设置为 zh-CN
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_locale', { locale: 'zh-CN' })
    );
    expect(await page.evaluate(() => window.__TAURI__.core.invoke('get_locale'))).toBe('zh-CN');
  });

  test('E2E-I18N-004 语言持久化——set_locale 后状态保存', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_locale', { locale: 'en' })
    );
    // 验证状态中保存了 locale
    const stored = await page.evaluate(() => window.__mock.state.locale);
    expect(stored).toBe('en');
  });

  test('E2E-I18N-005 切换回 zh-CN', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_locale', { locale: 'en' })
    );
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_locale', { locale: 'zh-CN' })
    );
    const locale = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_locale')
    );
    expect(locale).toBe('zh-CN');
  });

  test('E2E-I18N-006 设置语言——支持多种语言代码', async ({ page }) => {
    // 测试设置 ja-JP
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_locale', { locale: 'ja-JP' })
    );
    expect(await page.evaluate(() => window.__TAURI__.core.invoke('get_locale'))).toBe('ja-JP');

    // 恢复
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_locale', { locale: 'zh-CN' })
    );
  });

  test('E2E-I18N-007 语言切换不丢失其他状态', async ({ page }) => {
    // 设置一些其他状态
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.hybrid_search', value: String(true) })
    );
    // 切换语言
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_locale', { locale: 'en' })
    );
    // 验证混合检索状态未丢失
    const hybrid = await page.evaluate(() => window.__mock.state.hybridSearch);
    expect(hybrid).toBe(true);
  });

  test('E2E-I18N-008 初始 locale 状态', async ({ page }) => {
    const locale = await page.evaluate(() => window.__mock.state.locale);
    expect(locale).toBe('zh-CN');
  });

  // ─── data-i18n 属性验证 ───

  test('E2E-I18N-009 页面包含 data-i18n 属性元素', async ({ page }) => {
    // 检查页面中是否有 data-i18n 属性的元素
    const count = await page.evaluate(() => {
      return document.querySelectorAll('[data-i18n]').length;
    });
    expect(count).toBeGreaterThan(0);
  });

  test('E2E-I18N-010 向导页面 i18n 属性——标题文案', async ({ page }) => {
    // 验证页面标题存在
    const title = await page.title();
    expect(title.length).toBeGreaterThan(0);
    expect(typeof title).toBe('string');
  });
});
