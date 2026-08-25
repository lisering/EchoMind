// E2E 桥接测试共享辅助：UI URL 解析、Mock 注入、通用等待工具。
import path from 'node:path';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect } from '@playwright/test';

const specDir = path.dirname(fileURLToPath(import.meta.url));
export const uiUrl = 'file://' + path.resolve(specDir, '../../ui/index.html');
export const uiDir = path.resolve(specDir, '../../ui');
export const stubPath = path.resolve(specDir, 'bridge/tauri-stub.js');
const localeDir = path.resolve(specDir, '../../ui/locales');

/**
 * 注入 E2E 速度倍率到页面上下文。
 * 必须在 stub 注入之前调用（addInitScript 按顺序执行）。
 * tauri-stub.js 中的 delay(ms) 会乘以此倍率，CI 设 0.2 可加速 5 倍。
 * @param {import('@playwright/test').Page} page
 */
export async function injectSpeed(page) {
  const speed = Number(process.env.E2E_SPEED ?? 1);
  await page.addInitScript((s) => { window.__E2E_SPEED__ = s; }, speed);
}

/**
 * 注入 stub + 速度倍率（组合便捷方法，等效于先 injectSpeed 再 addInitScript stub）。
 * @param {import('@playwright/test').Page} page
 */
export async function injectStub(page) {
  await injectSpeed(page);
  await page.addInitScript({ path: stubPath });
}

/**
 * 注入 locale 数据到页面（patch fetch 拦截 locales/*.json 请求）。
 * 解决 file:// 协议下 fetch 被 Chromium CORS 阻止、i18n 语言包无法加载的问题。
 * 必须在 page.goto 之前调用。
 * @param {import('@playwright/test').Page} page
 */
export async function injectLocales(page) {
  const enJson = readFileSync(path.join(localeDir, 'en.json'), 'utf-8');
  const zhCnJson = readFileSync(path.join(localeDir, 'zh-CN.json'), 'utf-8');
  await page.addInitScript(([en, zhCn]) => {
    window.__localeData = { en: JSON.parse(en), 'zh-CN': JSON.parse(zhCn) };
    const origFetch = window.fetch;
    window.fetch = async function(url, ...args) {
      const urlStr = String(url);
      if (urlStr.includes('locales/') && urlStr.endsWith('.json')) {
        const locale = urlStr.split('/').pop().replace('.json', '');
        const data = window.__localeData[locale];
        if (data) {
          return new Response(JSON.stringify(data), {
            status: 200,
            headers: { 'Content-Type': 'application/json' },
          });
        }
      }
      return origFetch.call(this, url, ...args);
    };
  }, [enJson, zhCnJson]);
}

/**
 * 设置 E2E 测试页面：注入 Mock + 注入 locale 数据 + 预配置 LLM + 导航。
 *
 * 预配置模式：通过 addInitScript 在页面脚本执行前设置 state.configured = true，
 * 使 boot() 中 get_settings 返回 has_llm_config: true，直接调用 showApp() 跳过向导。
 * 这比走向导 UI 流程更快更可靠，避免 boot() 异步初始化与 enterApp 的竞态条件。
 *
 * 根因：boot() 首先执行 await initI18n()（涉及多次 fetch + JSON.parse 微任务），
 * 此时 enterApp 已开始执行 fill('#wizKey')，但 initWizard() 尚未被调用，
 * #wizStart 无 onclick handler → click 无效 → #app 保持 hidden → 超时。
 *
 * @param {import('@playwright/test').Page} page
 */
export async function setupPage(page) {
  await injectStub(page);
  await page.addInitScript(() => {
    // 预配置 LLM：boot() 将直接调用 showApp()，跳过向导
    window.__state.configured = true;
  });
  await injectLocales(page);
  await page.goto(uiUrl);
  // 等待 boot() 完成：initI18n → sync init → get_settings → showApp()
  await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
}

/**
 * 设置 E2E 测试页面（不进入应用）：注入 Mock + 注入 locale 数据 + 导航。
 * 用于测试配置向导等进入应用前的 UI 状态。
 * @param {import('@playwright/test').Page} page
 */
export async function setupPageWizard(page) {
  await injectStub(page);
  await injectLocales(page);
  await page.goto(uiUrl);
}

/**
 * 经向导快速进入主界面（配置 Mock LLM + 跳过向导）。
 *
 * 修复竞态条件：等待 boot() 异步初始化完成（initI18n → sync init → initWizard）
 * 后再与向导 UI 交互。initWizard() 设置 #wizStart.onclick handler，
 * 以此作为 boot 同步初始化完成的信号。
 *
 * @param {import('@playwright/test').Page} page
 */
export async function enterApp(page) {
  // 等待 boot() 完成同步初始化（initI18n 解析后 initWizard 设置 onclick handler）
  await page.waitForFunction(
    () => {
      const btn = document.getElementById('wizStart');
      return btn && btn.onclick !== null;
    },
    { timeout: 15000 },
  );
  // 等待向导 Step 2 可见（boot() 会根据 embedder 状态决定从哪步开始）
  await page.locator('#wizardStep2').waitFor({ state: 'visible', timeout: 15000 });
  await page.locator('#wizKey').waitFor({ state: 'visible', timeout: 15000 });
  await page.locator('#wizKey').fill('sk-e2e-mock');
  await page.locator('#wizStart').click();
  // Step 2 验证成功后进入 Step 3（导入文档），点击"开始使用"完成向导
  await page.locator('#wizardStep3').waitFor({ state: 'visible', timeout: 15000 });
  await page.locator('#wizFinish').click();
  await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
}

/**
 * 等待流式输出完成（chat_done 事件触发后 sendBtn 重新可见）。
 * @param {import('@playwright/test').Page} page
 * @param {number} timeout
 */
export async function waitForStreamDone(page, timeout = 15000) {
  await page.locator('#sendBtn').waitFor({ state: 'visible', timeout });
}

/**
 * 发送一条消息并等待首个 token 到达。
 * @param {import('@playwright/test').Page} page
 * @param {string} query
 */
export async function sendMessage(page, query) {
  await page.locator('#queryInput').fill(query);
  await page.locator('#sendBtn').click();
}

/**
 * 设置 Free 模式（isPro = false），用于测试付费墙/配额限制。
 * 必须在 page.goto 之后、测试操作之前调用。
 * @param {import('@playwright/test').Page} page
 */
export async function setFreeMode(page) {
  await page.evaluate(() => { window.__state.isPro = false; });
}

/**
 * 激活 Pro 版（直接调用 IPC，确保 mock state 和前端状态同步更新）。
 * 兼容 isPro 已为 true 的情况（直接刷新前端状态）。
 * @param {import('@playwright/test').Page} page
 */
export async function activatePro(page) {
  // 先确保 mock state 为 Free，再通过付费墙 UI 流程激活
  // 这样测试了完整的付费墙 → 激活流程
  // 先设置 isPro=false 以确保走 UI 流程
  await page.evaluate(() => { if (window.__state) window.__state.isPro = false; });
  // Free 模式：通过付费墙 UI 激活
  await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/activate-pro.pdf']));
  await expect(page.locator('#paywall')).toBeVisible({ timeout: 5000 });
  await page.locator('#licenseInput').fill('test-pro-key');
  await page.locator('#paywallActivate').click();
  await expect(page.locator('#paywall')).toBeHidden({ timeout: 5000 });
}

/**
 * 导入文档并等待列表渲染完成。
 * @param {import('@playwright/test').Page} page
 * @param {string[]} paths
 */
export async function importDocs(page, paths) {
  await page.evaluate((p) => {
    return window.__TAURI__.core.invoke('import_files', { paths: p });
  }, paths);
  // 等待元素挂载到 DOM（KB Modal 可能隐藏，不要求可见）
  await page.locator('#docList [data-doc-name]').first().waitFor({ state: 'attached', timeout: 5000 });
}

/**
 * 打开知识库弹框并等待可见。
 * @param {import('@playwright/test').Page} page
 */
export async function openKbModal(page) {
  await page.locator('#kbBtn').click();
  await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
}

/**
 * 关闭知识库弹框（Escape 键）。
 * @param {import('@playwright/test').Page} page
 */
export async function closeKbModal(page) {
  const modal = page.locator('#kbModal');
  if (await modal.isVisible()) {
    await page.keyboard.press('Escape');
    await expect(modal).toBeHidden({ timeout: 2000 });
  }
}

/**
 * 等待所有 toast 通知消失（容器无子元素）。
 * @param {import('@playwright/test').Page} page
 * @param {number} timeout
 */
export async function waitForToastsClear(page, timeout = 5000) {
  await page.waitForFunction(
    () => document.querySelector('#toasts')?.children.length === 0,
    { timeout }
  ).catch(() => {});
}

/**
 * 等待流式/审计输出完成（chat_done 事件触发后 sendBtn 重新可见）。
 * @param {import('@playwright/test').Page} page
 * @param {number} timeout
 */
export async function waitDone(page, timeout = 20000) {
  await page.locator('#sendBtn').waitFor({ state: 'visible', timeout });
}

/**
 * 等待指定 toast 文本出现。
 * @param {import('@playwright/test').Page} page
 * @param {string} text
 * @param {number} timeout
 */
export async function waitForToast(page, text, timeout = 5000) {
  await expect(page.locator('#toasts')).toContainText(text, { timeout });
}

/**
 * 打开工具下拉菜单（S5 P1-1：顶栏 8→5 按钮精简后，graph/dream/symbol/branchTree 按钮收纳到下拉菜单）。
 * 点击 #toolsBtn 展开 #toolsMenu，等待菜单可见后返回。
 * @param {import('@playwright/test').Page} page
 */

/**
 * 打开设置面板并切换到指定分区（V3.1 阶段二：S94 Tab 化后的统一入口）。
 * @param {import('@playwright/test').Page} page
 * @param {string} tabId - 分区 id（appearance/model/kb/retrieval/security/data/application/advanced）
 */

/**
 * 打开设置面板并显示全部分区（测试专用视图，V3.1 阶段二）。
 *
 * S94 Tab 化后非活动分区带 hidden；功能断言（toggle/文本/按钮）不依赖
 * 视觉 Tab，移除 hidden 让既有断言按原样工作。仅测试使用。
 * @param {import('@playwright/test').Page} page
 */
export async function showAllSettingsSections(page) {
  const modal = page.locator('#settingsModal');
  if (!(await modal.isVisible().catch(() => false))) {
    await page.locator('#settingsBtn').click();
    await expect(modal).toBeVisible({ timeout: 5000 });
  }
  await page.evaluate(() => {
    document
      .querySelectorAll('[data-settings-section]')
      .forEach((el) => el.classList.remove('hidden'));
  });
}

export async function openSettingsTab(page, tabId) {
  await page.locator('#settingsBtn').click();
  await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
  const tab = page.locator(`#settingsTabBar [data-tab-id="${tabId}"]`);
  if (await tab.count()) {
    await tab.click();
  }
}

export async function openToolsDropdown(page) {
  const toolsBtn = page.locator('#toolsBtn');
  await toolsBtn.waitFor({ state: 'visible', timeout: 5000 });
  await toolsBtn.click();
  await page.locator('#toolsMenu').waitFor({ state: 'visible', timeout: 3000 });
}

/**
 * 点击工具下拉菜单中的指定按钮并等待面板/覆盖层出现。
 * @param {import('@playwright/test').Page} page
 * @param {string} buttonId - 工具按钮 ID（如 'graphBtn', 'dreamBtn', 'symbolBtn', 'branchTreeBtn'）
 * @param {string} [waitForSelector] - 点击后等待出现的选择器（可选）
 * @param {number} [timeout] - 等待超时（默认 5000ms）
 */
export async function clickToolButton(page, buttonId, waitForSelector, timeout = 5000) {
  await openToolsDropdown(page);
  await page.locator(`#${buttonId}`).click();
  if (waitForSelector) {
    await page.locator(waitForSelector).waitFor({ state: 'visible', timeout });
  }
}

/**
 * 启用开发者模式（S5 P0-6：trace/budget/rag-eval 等开发者工具面板需要先启用开发者模式）。
 * 通过 ⌘Shift+D 快捷键切换 _devMode 标志。
 * @param {import('@playwright/test').Page} page
 */
export async function enableDevMode(page) {
  // S5 P0-6: ⌘Shift+D / Ctrl+Shift+D 切换 _devMode
  // 使用 Playwright keyboard API，按 Control+Shift+D
  // shiftKey 使 key 变为大写 'D'，匹配 settings.js 中的 e.key === 'D' 判断
  await page.keyboard.press('Control+Shift+KeyD');
  await page.waitForTimeout(300);
}
