// E2E 配置向导全场景（REQ-UI-007/008）。
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-UI-015~018 配置向导', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-UI-015 未配置时显示向导，已配置直接进入主界面', async ({ page }) => {
    // 首次启动：get_settings 返回未配置 → 显示向导
    await expect(page.locator('#wizard')).toBeVisible();
    await expect(page.locator('#app')).toBeHidden();

    // 配置后进入主界面
    await enterApp(page);
    await expect(page.locator('#app')).toBeVisible();
  });

  test('E2E-UI-016 验证失败时内联展示错误，不进入主界面', async ({ page }) => {
    await expect(page.locator('#wizard')).toBeVisible();
    // 等待向导 Step 2 可见（boot 根据embedder状态决定起始步骤）
    await page.locator('#wizardStep2').waitFor({ state: 'visible', timeout: 15000 });

    // 设置下次连接测试失败
    await page.evaluate(() => window.__mock.setConnectionFail());
    await page.locator('#wizKey').fill('sk-invalid');
    await page.locator('#wizStart').click();

    // 错误框出现，不进入主界面
    await expect(page.locator('#wizError')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#wizError')).toContainText('401');
    await expect(page.locator('#app')).toBeHidden();
  });

  test('E2E-UI-017 Ollama 预设允许空 API Key', async ({ page }) => {
    await expect(page.locator('#wizard')).toBeVisible();
    // 等待向导 Step 2 可见
    await page.locator('#wizardStep2').waitFor({ state: 'visible', timeout: 15000 });

    // 选择 Ollama 预设
    await page.locator('#presetCards button:has-text("Ollama")').click();
    // API Key 输入框旁应提示「本地端点可留空」
    await expect(page.locator('#keyOptional')).toBeVisible();

    // 不填 Key，直接验证并继续（Ollama 不需要 Key）
    await page.locator('#wizKey').fill('');
    await page.locator('#wizStart').click();
    // Step 2 验证成功后进入 Step 3
    await page.locator('#wizardStep3').waitFor({ state: 'visible', timeout: 15000 });
    await page.locator('#wizFinish').click();
    await expect(page.locator('#app')).toBeVisible({ timeout: 5000 });
  });

  test('E2E-UI-018 配置后运行态立即生效（无需重启）', async ({ page }) => {
    await enterApp(page);
    // 验证已进入主界面，可直接操作
    await expect(page.locator('#queryInput')).toBeVisible();
    // 验证配置已保存（get_settings 返回已配置）
    const settings = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_settings'),
    );
    expect(settings.has_llm_config).toBe(true);
    expect(settings.model).toBe('mock-llm');
  });

  test('E2E-UI-017b 预设卡片切换时自动填充 base_url 与 model', async ({ page }) => {
    // RC6 修复：前序测试 E2E-UI-018 调用 enterApp 后 mock 状态为 configured，
    // 需重置 mock 状态才能显示向导
    await page.evaluate(() => window.__mock.reset());
    await page.goto(uiUrl);
    await page.waitForTimeout(1000); // 等待 boot() 检测 + renderPresetCards()
    await expect(page.locator('#wizard')).toBeVisible();
    // 等待向导 Step 2 可见
    await page.locator('#wizardStep2').waitFor({ state: 'visible', timeout: 15000 });
    // 等待预设卡片渲染（PRESETS 有 10 个：deepseek/openai/qwen/kimi/glm/minimax/mistral/grok/ollama/custom）
    await expect(page.locator('#presetCards button')).toHaveCount(10, { timeout: 5000 });

    // 默认 DeepSeek 预设
    await expect(page.locator('#wizUrl')).toHaveValue('https://api.deepseek.com');
    await expect(page.locator('#wizModel')).toHaveValue('deepseek-chat');

    // 切换到 OpenAI
    // RC7 修复：filter hasText 正则不匹配多行文本，改用 evaluate 查找按钮
    await page.evaluate(() => {
      const buttons = document.querySelectorAll('#presetCards button');
      for (const btn of buttons) {
        const label = btn.querySelector('div, span');
        if (label && label.textContent?.trim() === 'OpenAI') {
          btn.click();
          return;
        }
      }
    });
    await page.waitForTimeout(200);
    await expect(page.locator('#wizUrl')).toHaveValue('https://api.openai.com');
    await expect(page.locator('#wizModel')).toHaveValue('gpt-4o-mini');

    // 切换到 Ollama
    await page.evaluate(() => {
      const buttons = document.querySelectorAll('#presetCards button');
      for (const btn of buttons) {
        const label = btn.querySelector('div, span');
        if (label && label.textContent?.trim() === 'Ollama') {
          btn.click();
          return;
        }
      }
    });
    await page.waitForTimeout(200);
    await expect(page.locator('#wizUrl')).toHaveValue('http://localhost:11434');
    await expect(page.locator('#wizModel')).toHaveValue('llama3.1');
  });
});
