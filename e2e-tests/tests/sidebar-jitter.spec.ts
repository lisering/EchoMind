// E2E 侧栏切换零抖动验证（transform 位移方案）。
//
// 方案：#sidebar 使用 position:fixed + transform:translateX 滑出视口
// （GPU 合成层加速，不触发任何布局重排），<main> 使用 margin-left 过渡
// 跟随扩展。margin-left 过渡期间聊天区宽度逐帧变化 → 文本重排 →
// scrollHeight 变化 → 需要 rAF 锚定循环补偿垂直位移。
//
// rAF 锚定循环（ui/src/sidebar.js）：动画期间每个 rAF 帧补偿重排位移：
// - 原本在底部 → 逐帧钉住 scrollHeight（底部始终贴住视口）。
// - 中部滚动 → 保持视口顶部首个可见内容元素的相对偏移不变。
// 浏览器原生 scroll anchoring 在下方兜底（零延迟修正）。
//
// 注意：动画期间 scrollTop 数值会因锚定补偿而合法变化（内容重排所致），
// 「不抖动」的正确度量是可见内容的位置（锚点元素的视口偏移），而非
// scrollTop 数值本身。
//
// E2E-SB-JITTER-001: 折叠动画期间+结束后，可见内容锚点偏移恒定（中部滚动位置）
// E2E-SB-JITTER-002: 折叠后若原本在底部，底部始终钉住（底部不跳动）
// E2E-SB-JITTER-003: 展开动画期间+结束后，可见内容锚点偏移恒定（中部滚动位置）
import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test.describe('E2E-SB-JITTER 侧栏切换零垂直抖动', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  /** 向聊天区注入大量长文本，确保可滚动且宽度变化会触发重排。 */
  async function fillChatWithLongContent(page) {
    await page.evaluate(() => {
      const chatArea = document.getElementById('chatArea');
      chatArea.innerHTML = '';
      const para = '这是一段用于触发文本重排的长内容，切换侧栏时若发生逐帧重排 scrollHeight 会变化。'.repeat(25);
      for (let i = 0; i < 40; i++) {
        const div = document.createElement('div');
        div.className = 'py-4';
        div.textContent = `第 ${i} 段：${para}`;
        chatArea.appendChild(div);
      }
    });
  }

  /** 采样时间点（ms）：覆盖 300ms 过渡期间（60-240）+ 结束后（420）。 */
  const SAMPLE_POINTS = [60, 120, 180, 240, 420];

  /**
   * 读取视口顶部可见内容锚点的相对 Y 偏移（与 startAnchorLoop 同款选取逻辑）。
   * 锚点偏移不变 = 可见内容没有垂直位移 = 零抖动。
   */
  async function readAnchorOffset(page) {
    return page.evaluate(() => {
      const ca = document.getElementById('chatArea');
      const rect = ca.getBoundingClientRect();
      let anchor = document.elementFromPoint(rect.left + 40, rect.top + 40);
      if (anchor === ca) anchor = null;
      return anchor ? anchor.getBoundingClientRect().top - rect.top : null;
    });
  }

  /** 点击按钮，在采样点逐步读取锚点偏移与聊天区容器宽度。 */
  async function sampleDuringToggle(page, btnId) {
    await page.locator(btnId).click();
    let prev = 0;
    const offsets = [];
    const widths = [];
    for (const ms of SAMPLE_POINTS) {
      await page.waitForTimeout(ms - prev);
      prev = ms;
      offsets.push(await readAnchorOffset(page));
      widths.push(await page.evaluate(() => {
        const wrapper = document.getElementById('chatArea').parentElement;
        return wrapper.getBoundingClientRect().width;
      }));
    }
    return { offsets, widths };
  }

  test('E2E-SB-JITTER-001 折叠动画期间+结束后锚点偏移恒定、宽度实时跟随（中部滚动位置）', async ({ page }) => {
    await fillChatWithLongContent(page);
    await page.evaluate(() => {
      const ca = document.getElementById('chatArea');
      ca.scrollTop = Math.floor((ca.scrollHeight - ca.clientHeight) * 0.5);
    });

    const before = await readAnchorOffset(page);
    expect(before, '应有可见内容锚点').not.toBeNull();
    const startWidth = await page.evaluate(() => document.getElementById('chatArea').parentElement.getBoundingClientRect().width);
    // 折叠后聊天区容器应变宽（侧栏 240px 让位）
    const endWidth = startWidth + 240;

    const { offsets, widths } = await sampleDuringToggle(page, '#collapseBtn');
    for (const o of offsets) {
      expect(Math.abs(o - before), `锚点视口偏移应保持 ${before}，实际 ${o}`).toBeLessThanOrEqual(3);
    }
    // 宽度零延迟：动画早期（60ms）宽度已开始向目标推进（放宽阈值适应性能差异）
    expect(widths[0], `60ms 时宽度应已开始展开（应 > ${startWidth + 5}，实际 ${widths[0]})`)
      .toBeGreaterThan(startWidth + 5);
    expect(Math.abs(widths[widths.length - 1] - endWidth), '420ms 时宽度应精确到达目标')
      .toBeLessThanOrEqual(1);
  });

  test('E2E-SB-JITTER-002 折叠后若原本在底部，底部始终钉住', async ({ page }) => {
    await fillChatWithLongContent(page);
    await page.evaluate(() => {
      const ca = document.getElementById('chatArea');
      ca.scrollTop = ca.scrollHeight;
    });

    // 动画期间（120ms/240ms）+ 结束后（450ms）都应贴住底部
    await page.locator('#collapseBtn').click();
    const dists = [];
    let prev = 0;
    for (const ms of [120, 240, 450]) {
      await page.waitForTimeout(ms - prev);
      prev = ms;
      dists.push(await page.evaluate(() => {
        const ca = document.getElementById('chatArea');
        return ca.scrollHeight - ca.scrollTop - ca.clientHeight;
      }));
    }
    for (const d of dists) {
      expect(d, `应贴住底部，实际距底部 ${d}px`).toBeLessThanOrEqual(1);
    }
  });

  test('E2E-SB-JITTER-003 展开动画期间+结束后锚点偏移恒定（中部滚动位置）', async ({ page }) => {
    await fillChatWithLongContent(page);
    await page.evaluate(() => {
      const ca = document.getElementById('chatArea');
      ca.scrollTop = Math.floor((ca.scrollHeight - ca.clientHeight) * 0.5);
    });

    // 先折叠并等待完全结束（布局稳定）
    await page.locator('#collapseBtn').click();
    await page.waitForTimeout(450);

    const before = await readAnchorOffset(page);
    expect(before, '应有可见内容锚点').not.toBeNull();
    const startWidth = await page.evaluate(() => document.getElementById('chatArea').parentElement.getBoundingClientRect().width);
    // 展开后聊天区容器应变窄（侧栏占回 240px）
    const endWidth = startWidth - 240;

    const { offsets, widths } = await sampleDuringToggle(page, '#expandBtn');
    for (const o of offsets) {
      expect(Math.abs(o - before), `锚点视口偏移应保持 ${before}，实际 ${o}`).toBeLessThanOrEqual(3);
    }
    // 宽度零延迟：动画早期（60ms）宽度已明显向目标收缩，结束时精确到达目标
    expect(widths[0], `60ms 时宽度应已开始收缩（应 < ${startWidth - 20}，实际 ${widths[0]}）`)
      .toBeLessThan(startWidth - 20);
    expect(Math.abs(widths[widths.length - 1] - endWidth), '420ms 时宽度应精确到达目标')
      .toBeLessThanOrEqual(1);
  });
});
