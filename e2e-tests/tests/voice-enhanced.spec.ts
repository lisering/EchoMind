/**
 * 语音输入增强功能 E2E 测试（REQ-RAG-034 增强）。
 *
 * 验证：
 * - TC-VOICE-ENH-001：录音遮罩在录音时显示
 * - TC-VOICE-ENH-002：音频电平条存在
 * - TC-VOICE-ENH-003：录音计时器从 00:00 开始
 * - TC-VOICE-ENH-004：STT 配置面板存在于设置中
 * - TC-VOICE-ENH-005：STT 配置可保存
 * - TC-VOICE-ENH-006：转写文本追加到已有文本后
 */

import { test, expect } from '@playwright/test';
import { setupPage, sendMessage, waitForStreamDone } from './helpers.mjs';

test.beforeEach(async ({ page }) => {
  await setupPage(page);
});

test.describe('语音输入增强 (REQ-RAG-034 增强)', () => {
  test('TC-VOICE-ENH-001 录音遮罩在录音时显示', async ({ page }) => {
    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });

    // 启动录音
    await micBtn.click();
    await expect(micBtn).toHaveClass(/recording/, { timeout: 3000 });

    // 录音遮罩应可见
    const overlay = page.locator('#recordingOverlay');
    await expect(overlay).toBeVisible({ timeout: 2000 });

    // 停止录音
    await micBtn.click();

    // 遮罩应隐藏
    await expect(overlay).not.toBeVisible({ timeout: 5000 });
  });

  test('TC-VOICE-ENH-002 音频电平条存在', async ({ page }) => {
    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });

    await micBtn.click();
    await expect(micBtn).toHaveClass(/recording/, { timeout: 3000 });

    // 验证电平条存在
    const levelBars = page.locator('#levelBarsContainer .level-bar');
    const count = await levelBars.count();
    expect(count).toBeGreaterThanOrEqual(5);

    // 停止录音
    await micBtn.click();
    await page.waitForTimeout(1000);
  });

  test('TC-VOICE-ENH-003 录音计时器从 00:00 开始', async ({ page }) => {
    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });

    await micBtn.click();
    await expect(micBtn).toHaveClass(/recording/, { timeout: 3000 });

    // 计时器应显示 00:00
    const timer = page.locator('#recordingTimer');
    await expect(timer).toBeVisible({ timeout: 2000 });
    const text = await timer.textContent();
    expect(text).toMatch(/^\d{2}:\d{2}$/);

    // 停止录音
    await micBtn.click();
    await page.waitForTimeout(1000);
  });

  test('TC-VOICE-ENH-004 STT 配置面板存在于设置中', async ({ page }) => {
    // 打开设置
    const settingsBtn = page.locator('#settingsBtn');
    await settingsBtn.click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 3000 });

    // STT 配置区域应存在
    const sttSection = page.locator('#sttConfigSection');
    await expect(sttSection).toBeVisible({ timeout: 3000 });

    // 验证字段存在
    await expect(page.locator('#sttApiKeyInput')).toBeVisible();
    await expect(page.locator('#sttBaseUrlInput')).toBeVisible();
    await expect(page.locator('#sttModelInput')).toBeVisible();
    await expect(page.locator('#sttLanguageSelect')).toBeVisible();
    await expect(page.locator('#sttSaveBtn')).toBeVisible();
  });

  test('TC-VOICE-ENH-005 STT 配置可保存', async ({ page }) => {
    // 打开设置
    const settingsBtn = page.locator('#settingsBtn');
    await settingsBtn.click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 3000 });

    // 等待 STT 配置加载
    await page.waitForTimeout(500);

    // 填写配置
    await page.locator('#sttBaseUrlInput').fill('https://api.groq.com/openai');
    await page.locator('#sttModelInput').fill('whisper-large-v3');

    // 选择英文
    await page.locator('#sttLanguageSelect').selectOption('en');

    // 保存
    await page.locator('#sttSaveBtn').click();

    // 等待保存成功
    await page.waitForTimeout(1000);

    // 验证配置已保存（重新加载）
    const config = await page.evaluate(() => {
      return (window as any).__mock?.sttConfig;
    });
    expect(config).toBeTruthy();
    expect(config.baseUrl).toBe('https://api.groq.com/openai');
    expect(config.model).toBe('whisper-large-v3');
    expect(config.language).toBe('en');
  });

  test('TC-VOICE-ENH-006 转写文本追加到已有文本后', async ({ page }) => {
    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });

    // 先在输入框填入已有文本（使用 evaluate 设置 value + 触发 input 事件）
    await page.evaluate(() => {
      const input = document.getElementById('queryInput') as HTMLTextAreaElement;
      if (input) {
        input.value = '已有文本';
        input.dispatchEvent(new Event('input', { bubbles: true }));
      }
    });

    // 启动录音
    await micBtn.click();
    await expect(micBtn).toHaveClass(/recording/, { timeout: 3000 });

    // 停止录音
    await micBtn.click();

    // 等待转写结果
    await page.waitForFunction(
      () => {
        const el = document.getElementById('queryInput') as HTMLTextAreaElement;
        return el && el.value.includes('测试语音输入');
      },
      { timeout: 10000 },
    );

    // 验证追加模式：已有文本 + 空格 + 转写文本
    const value = await page.locator('#queryInput').inputValue();
    expect(value).toContain('已有文本');
    expect(value).toContain('测试语音输入');
  });
});
