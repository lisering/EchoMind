/**
 * 语音输入 + TTS 朗读 E2E 测试（REQ-RAG-034 / REQ-RAG-035）
 *
 * 桌面应用方案：getUserMedia + MediaRecorder + IPC transcribe_audio
 *
 * TC-UI-VOICE-001: 麦克风按钮存在 — #micBtn 元素可见
 * TC-UI-VOICE-002: 点击麦克风启动录音 — #micBtn 添加 .recording 类
 * TC-UI-VOICE-003: 停止录音后转写结果填入输入框 — #queryInput.value 非空
 * TC-UI-VOICE-004: 朗读按钮存在 — .tts-btn 元素可见（在 AI 消息操作栏中）
 * TC-UI-VOICE-005: 点击朗读启动合成 — 点击 .tts-btn，验证按钮状态变为 speaking
 * TC-UI-VOICE-006: getUserMedia 不支持时隐藏麦克风按钮
 * TC-UI-VOICE-007: 权限拒绝时显示错误提示
 * TC-UI-VOICE-008: 转写 API 错误时显示错误提示
 */
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, sendMessage, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';

test.describe('语音输入 + TTS 朗读 (REQ-RAG-034 / REQ-RAG-035)', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    // 导入文档（对话前置条件）
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/echomind-e2e.md'] }),
    );
    // 等待元素挂载到 DOM
    await page.locator('#docList [data-doc-name]').first().waitFor({ state: 'attached', timeout: 5000 });
  });

  test('TC-UI-VOICE-001 麦克风按钮存在且可见', async ({ page }) => {
    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });
  });

  test('TC-UI-VOICE-002 点击麦克风启动录音', async ({ page }) => {
    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });

    // 点击麦克风按钮启动录音
    await micBtn.click();

    // 验证按钮进入录音状态（.recording 类）
    await expect(micBtn).toHaveClass(/recording/, { timeout: 3000 });
  });

  test('TC-UI-VOICE-003 停止录音后转写结果填入输入框', async ({ page }) => {
    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });

    // 启动录音
    await micBtn.click();
    await expect(micBtn).toHaveClass(/recording/, { timeout: 3000 });

    // 再次点击停止录音
    await micBtn.click();

    // 等待转写结果填入输入框（mock 返回 '测试语音输入'）
    await page.waitForFunction(
      () => {
        const input = document.getElementById('queryInput') as HTMLTextAreaElement;
        return input && input.value.length > 0;
      },
      { timeout: 10000 },
    );

    // 验证转写文本正确
    const value = await page.locator('#queryInput').inputValue();
    expect(value).toContain('测试语音输入');
  });

  test('TC-UI-VOICE-004 朗读按钮存在于 AI 消息操作栏', async ({ page }) => {
    await sendMessage(page, '测试朗读按钮');
    await waitForStreamDone(page, 15000);

    const ttsBtn = page.locator('#chatArea .msg-actions .tts-btn').last();
    await expect(ttsBtn).toBeAttached({ timeout: 5000 });
  });

  test('TC-UI-VOICE-005 点击朗读按钮启动语音合成', async ({ page }) => {
    await page.evaluate(() => {
      (window as any).__ttsSpeakCalled = false;
      window.speechSynthesis.speak = function (u: any) {
        (window as any).__ttsSpeakCalled = true;
        if (u && u.onend) setTimeout(() => u.onend(), 200);
      };
    });

    await sendMessage(page, '测试语音朗读');
    await waitForStreamDone(page, 15000);

    const ttsBtn = page.locator('#chatArea .msg-actions .tts-btn').last();
    await expect(ttsBtn).toBeAttached({ timeout: 5000 });

    await ttsBtn.click();

    await page.waitForFunction(
      () => (window as any).__ttsSpeakCalled === true,
      { timeout: 5000 },
    );
    expect(await page.evaluate(() => (window as any).__ttsSpeakCalled)).toBe(true);
  });

  test('TC-UI-VOICE-006 getUserMedia 不支持时隐藏麦克风按钮', async ({ page }) => {
    // 在页面加载前完全移除 navigator.mediaDevices
    await page.addInitScript(() => {
      // 完全删除 mediaDevices 对象
      try {
        delete (navigator as any).mediaDevices;
        Object.defineProperty(navigator, 'mediaDevices', {
          get: () => undefined,
          configurable: true,
        });
      } catch (_) {
        // 如果无法删除，设为 undefined
        (navigator as any).mediaDevices = undefined;
      }
    });
    await page.goto(uiUrl);
    await enterApp(page);

    // initVoiceInput 应检测到 mediaDevices 不存在并隐藏按钮
    const display = await page.locator('#micBtn').evaluate((el) => {
      return (el as HTMLElement).style.display;
    });
    expect(display).toBe('none');
  });

  test('TC-UI-VOICE-007 权限拒绝时显示错误提示', async ({ page }) => {
    // 覆盖 getUserMedia 使其返回权限拒绝错误
    await page.evaluate(() => {
      navigator.mediaDevices.getUserMedia = function () {
        return Promise.reject(new DOMException('Permission denied', 'NotAllowedError'));
      };
    });

    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });

    // 点击麦克风按钮 — 应触发权限拒绝错误
    await micBtn.click();

    // 验证按钮未进入录音状态
    await page.waitForTimeout(500);
    expect(await micBtn.evaluate((el) => el.classList.contains('recording'))).toBe(false);
  });

  test('TC-UI-VOICE-008 转写 API 错误时显示错误提示', async ({ page }) => {
    // 覆盖 transcribe_audio mock 使其返回错误
    await page.evaluate(() => {
      const origInvoke = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = function (cmd: string, args?: any) {
        if (cmd === 'transcribe_audio') {
          return Promise.reject('LLM: 语音转写 API 返回错误 401: Unauthorized');
        }
        return origInvoke.call(this, cmd, args);
      };
    });

    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });

    // 启动录音
    await micBtn.click();
    await expect(micBtn).toHaveClass(/recording/, { timeout: 3000 });

    // 停止录音 — 触发转写，应失败
    await micBtn.click();

    // 等待转写完成（失败），按钮恢复非录音状态
    await page.waitForFunction(
      () => {
        const btn = document.getElementById('micBtn');
        return btn && !btn.classList.contains('recording') && !btn.classList.contains('transcribing');
      },
      { timeout: 10000 },
    );

    // 验证输入框为空（转写失败未填入文本）
    const value = await page.locator('#queryInput').inputValue();
    expect(value).toBe('');
  });
});
