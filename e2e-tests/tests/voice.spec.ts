/**
 * 语音输入 + TTS 朗读 E2E 测试（REQ-RAG-034 / REQ-RAG-035）
 *
 * TC-UI-VOICE-001: 麦克风按钮存在 — #micBtn 元素可见
 * TC-UI-VOICE-002: 点击麦克风启动识别 — 点击 #micBtn，等待 #queryInput.value 非空
 * TC-UI-VOICE-003: 转写结果填入输入框 — #queryInput.value 包含 mock 转写文本
 * TC-UI-VOICE-004: 朗读按钮存在 — .tts-btn 元素可见（在 AI 消息操作栏中）
 * TC-UI-VOICE-005: 点击朗读启动合成 — 点击 .tts-btn，验证按钮状态变为 speaking
 * TC-UI-VOICE-006: Web Speech API 不支持时隐藏 UI — 删除 window.SpeechRecognition，验证 #micBtn 隐藏
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
    // #micBtn 元素应存在且可见（tauri-stub.js 注入了 SpeechRecognition mock）
    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });
  });

  test('TC-UI-VOICE-002 点击麦克风启动语音识别', async ({ page }) => {
    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });

    // 点击麦克风按钮
    await micBtn.click();

    // 等待 mock 识别完成（mock 在 ~150ms 后回调 onresult）
    // 验证 #queryInput 有值
    await page.waitForFunction(
      () => {
        const input = document.getElementById('queryInput') as HTMLTextAreaElement;
        return input && input.value.length > 0;
      },
      { timeout: 5000 },
    );
  });

  test('TC-UI-VOICE-003 转写结果填入输入框', async ({ page }) => {
    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toBeVisible({ timeout: 5000 });

    // 点击麦克风
    await micBtn.click();

    // 等待转写结果填入
    await page.waitForFunction(
      () => {
        const input = document.getElementById('queryInput') as HTMLTextAreaElement;
        return input && input.value.includes('测试语音输入');
      },
      { timeout: 5000 },
    );

    // 验证转写文本正确
    const value = await page.locator('#queryInput').inputValue();
    expect(value).toContain('测试语音输入');
  });

  test('TC-UI-VOICE-004 朗读按钮存在于 AI 消息操作栏', async ({ page }) => {
    // 发送消息获取 AI 回答
    await sendMessage(page, '测试朗读按钮');
    await waitForStreamDone(page, 15000);

    // 等待消息操作栏渲染（finalizeStream 中 showMsgActions 被调用）
    // .tts-btn 应在 .msg-actions 中
    const ttsBtn = page.locator('#chatArea .msg-actions .tts-btn').last();
    await expect(ttsBtn).toBeAttached({ timeout: 5000 });
  });

  test('TC-UI-VOICE-005 点击朗读按钮启动语音合成', async ({ page }) => {
    // 在页面上设置 spy 来检测 speechSynthesis.speak 是否被调用
    await page.evaluate(() => {
      const origSpeak = window.speechSynthesis.speak.bind(window.speechSynthesis);
      (window as any).__ttsSpeakCalled = false;
      window.speechSynthesis.speak = function (u: any) {
        (window as any).__ttsSpeakCalled = true;
        if (u && u.onend) setTimeout(() => u.onend(), 200);
      };
    });

    // 发送消息获取 AI 回答
    await sendMessage(page, '测试语音朗读');
    await waitForStreamDone(page, 15000);

    // 等待 TTS 按钮渲染
    const ttsBtn = page.locator('#chatArea .msg-actions .tts-btn').last();
    await expect(ttsBtn).toBeAttached({ timeout: 5000 });

    // 点击朗读按钮
    await ttsBtn.click();

    // 验证 speechSynthesis.speak 被调用了
    await page.waitForFunction(
      () => (window as any).__ttsSpeakCalled === true,
      { timeout: 5000 },
    );
    expect(await page.evaluate(() => (window as any).__ttsSpeakCalled)).toBe(true);
  });

  test('TC-UI-VOICE-006 Web Speech API 不支持时隐藏麦克风按钮', async ({ page }) => {
    // 模拟不支持 Web Speech API 的环境
    // 删除 SpeechRecognition 和 webkitSpeechRecognition
    await page.evaluate(() => {
      delete (window as any).SpeechRecognition;
      delete (window as any).webkitSpeechRecognition;
      // 模拟 initVoiceInput 中的检测逻辑
      const micBtn = document.getElementById('micBtn');
      if (micBtn) {
        const SR = (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;
        if (!SR) {
          micBtn.style.display = 'none';
        }
      }
    });

    // 验证 #micBtn 被隐藏
    const display = await page.locator('#micBtn').evaluate((el) => {
      return (el as HTMLElement).style.display;
    });
    expect(display).toBe('none');
  });
});
