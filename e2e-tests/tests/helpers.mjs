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
 * 激活 Pro 版（通过付费墙 UI 流程，确保前端 isPro 变量正确更新）。
 * @param {import('@playwright/test').Page} page
 */
export async function activatePro(page) {
  // 1. 先模拟 PDF 拖拽触发付费墙（此时前端 isPro 仍为 false）
  await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/activate-pro.pdf']));
  await expect(page.locator('#paywall')).toBeVisible({ timeout: 5000 });
  // 2. 填写 License Key 并激活
  await page.locator('#licenseInput').fill('test-pro-key');
  await page.locator('#paywallActivate').click();
  // 3. 等待付费墙关闭（前端 activatePro 设置 isPro=true 并刷新 UI）
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
