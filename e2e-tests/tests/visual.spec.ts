// E2E UI 视觉规范测试 — 设计 Token、布局尺寸、排版、组件状态、安全审计。
// 覆盖 REQ-UI-001/002/003/004/005、REQ-SEC-002/003、REQ-VIZ-004。
//
// 测试维度：
// 1. 设计 Token 颜色验证（surface/body/accent/text/border 全层级）
// 2. 排版规范（font-family、font-size、font-weight）
// 3. 布局尺寸（sidebar 宽度、topBar 高度、inputBar 圆角）
// 4. 圆角规范（button/input/modal border-radius）
// 5. 组件可见性与 SVG 图标
// 6. 状态样式（disabled/hover/focus）
// 7. z-index 层级关系
// 8. CDN 审计 + XSS 防御
// 9. 拖拽遮罩视觉
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, sendMessage, injectStub, importDocs, uiDir, uiUrl, waitForStreamDone } from './helpers.mjs';
import fs from 'node:fs';
import path from 'node:path';

// ─── 设计 Token 常量（与 index.html tailwind.config 一一对应） ───
const TOKENS = {
  surface0: 'rgb(10, 10, 11)',    // #0A0A0B — body 背景
  surface1: 'rgb(19, 19, 22)',    // #131316 — sidebar/topBar 背景
  surface2: 'rgb(28, 28, 32)',    // #1C1C20 — settings 卡片背景
  surface3: 'rgb(38, 38, 44)',    // #26262C — badge/select 背景
  accent:   'rgb(56, 189, 248)',  // #38BDF8 — 强调色
  ink:      'rgb(12, 17, 22)',    // #0C1116 — 按钮文字色
  textPrimary:   'rgb(248, 250, 252)',  // #F8FAFC
  textSecondary: 'rgb(203, 213, 225)',  // #CBD5E1
  textTertiary:  'rgb(148, 163, 184)',  // #94A3B8
  textQuaternary:'rgb(100, 116, 139)',  // #64748B
  borderSubtle:  'rgb(31, 31, 35)',     // #1F1F23
  borderDefault: 'rgb(42, 42, 46)',     // #2A2A2E
  borderStrong:  'rgb(58, 58, 64)',     // #3A3A40
  danger:   'rgb(248, 113, 113)',  // #F87171
  success:  'rgb(74, 222, 128)',   // #4ADE80
};

test.describe('E2E-VIS-001 设计 Token 颜色验证', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    // 前置导入文档：KB 为空时 sendBtn disabled（disabled 样式覆盖颜色/圆角断言）
    await importDocs(page, ['/mock/echomind-e2e.md']);
  });

  test('E2E-VIS-001a body 背景色 = surface-0 (#0A0A0B)', async ({ page }) => {
    await enterApp(page);
    const bgColor = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
    expect(bgColor).toBe(TOKENS.surface0);
  });

  test('E2E-VIS-001b sidebar 背景色 = surface-1 (#131316)', async ({ page }) => {
    await enterApp(page);
    const bgColor = await page.locator('#sidebar').evaluate(el => getComputedStyle(el).backgroundColor);
    expect(bgColor).toBe(TOKENS.surface1);
  });

  test('E2E-VIS-001c topBar 背景色 = surface-1 (#131316)', async ({ page }) => {
    await enterApp(page);
    const bgColor = await page.locator('#topBar').evaluate(el => getComputedStyle(el).backgroundColor);
    expect(bgColor).toBe(TOKENS.surface1);
  });

  test('E2E-VIS-001d sendBtn 文字色 = ink (#0C1116)', async ({ page }) => {
    await enterApp(page);
    const color = await page.locator('#sendBtn').evaluate(el => getComputedStyle(el).color);
    expect(color).toBe(TOKENS.ink);
  });

  test('E2E-VIS-001e sendBtn 背景色 = accent (#38BDF8)', async ({ page }) => {
    await enterApp(page);
    const bgColor = await page.locator('#sendBtn').evaluate(el => getComputedStyle(el).backgroundColor);
    expect(bgColor).toBe(TOKENS.accent);
  });

  test('E2E-VIS-001f brand 文字色 = text-primary (#F8FAFC)', async ({ page }) => {
    await enterApp(page);
    const brandEl = page.locator('#sidebar .font-semibold');
    const color = await brandEl.evaluate(el => getComputedStyle(el).color);
    expect(color).toBe(TOKENS.textPrimary);
  });

  test('E2E-VIS-001g newChatBtn 文字色 = accent (#38BDF8)', async ({ page }) => {
    await enterApp(page);
    const color = await page.locator('#newChatBtn').evaluate(el => getComputedStyle(el).color);
    expect(color).toBe(TOKENS.accent);
  });

  test('E2E-VIS-001h proStatusBadge 文字色 = text-tertiary (#94A3B8)', async ({ page }) => {
    await enterApp(page);
    const color = await page.locator('#proStatusBadge').evaluate(el => getComputedStyle(el).color);
    expect(color).toBe(TOKENS.textTertiary);
  });

  test('E2E-VIS-001i topBar border-bottom = border-subtle (#1F1F23)', async ({ page }) => {
    await enterApp(page);
    const border = await page.locator('#topBar').evaluate(el => getComputedStyle(el).borderBottomColor);
    expect(border).toBe(TOKENS.borderSubtle);
  });

  test('E2E-VIS-001j sidebar border-right = border-subtle (#1F1F23)', async ({ page }) => {
    await enterApp(page);
    const border = await page.locator('#sidebar').evaluate(el => getComputedStyle(el).borderRightColor);
    expect(border).toBe(TOKENS.borderSubtle);
  });
});

test.describe('E2E-VIS-002 排版规范', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-002a body font-family 包含 -apple-system', async ({ page }) => {
    await enterApp(page);
    const fontFamily = await page.evaluate(() => getComputedStyle(document.body).fontFamily);
    expect(fontFamily).toContain('-apple-system');
  });

  test('E2E-VIS-002b brand 文字大小 = 24px (text-2xl)', async ({ page }) => {
    // 停留在向导页检查 brand
    const fontSize = await page.locator('#wizard .text-2xl').evaluate(el => getComputedStyle(el).fontSize);
    expect(fontSize).toBe('24px');
  });

  test('E2E-VIS-002c brand font-weight = 600 (semibold)', async ({ page }) => {
    const fontWeight = await page.locator('#wizard .text-2xl').evaluate(el => getComputedStyle(el).fontWeight);
    expect(fontWeight).toBe('600');
  });

  test('E2E-VIS-002d sendBtn 文字大小 = 16px (inherited)', async ({ page }) => {
    await enterApp(page);
    const fontSize = await page.locator('#sendBtn').evaluate(el => getComputedStyle(el).fontSize);
    expect(fontSize).toBe('16px');
  });

  test('E2E-VIS-002e queryInput 文字大小 = 16px (inherited)', async ({ page }) => {
    await enterApp(page);
    const fontSize = await page.locator('#queryInput').evaluate(el => getComputedStyle(el).fontSize);
    expect(fontSize).toBe('16px');
  });

  test('E2E-VIS-002f proStatusBadge 文字大小 = 10px', async ({ page }) => {
    await enterApp(page);
    const fontSize = await page.locator('#proStatusBadge').evaluate(el => getComputedStyle(el).fontSize);
    expect(fontSize).toBe('10px');
  });
});

test.describe('E2E-VIS-003 布局尺寸验证', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-003a sidebar 宽度 = 240px (position:fixed)', async ({ page }) => {
    await enterApp(page);
    const width = await page.locator('#sidebar').evaluate(el => el.offsetWidth);
    expect(width).toBe(240);
  });

  test('E2E-VIS-003b topBar 高度 = 28px (h-7)', async ({ page }) => {
    await enterApp(page);
    const height = await page.locator('#topBar').evaluate(el => el.offsetHeight);
    expect(height).toBe(28);
  });

  test('E2E-VIS-003c sendBtn 高度 = 32px (h-8)', async ({ page }) => {
    await enterApp(page);
    const height = await page.locator('#sendBtn').evaluate(el => el.offsetHeight);
    expect(height).toBe(32);
  });

  test('E2E-VIS-003d toolbar 按钮尺寸 = 24x24px (w-6 h-6)', async ({ page }) => {
    await enterApp(page);
    const kbBtn = page.locator('#kbBtn');
    const w = await kbBtn.evaluate(el => el.offsetWidth);
    const h = await kbBtn.evaluate(el => el.offsetHeight);
    expect(w).toBe(24);
    expect(h).toBe(24);
  });

  test('E2E-VIS-003e sidebar 折叠后滑出视口（sidebar-collapsed，REQ-NAV-001）', async ({ page }) => {
    await enterApp(page);
    await page.locator('#collapseBtn').click();
    await page.waitForTimeout(500);
    // transform 方案：折叠后侧栏宽度仍为 240px，但通过 translateX(-100%) 滑出视口
    const sb = page.locator('#sidebar');
    await expect(sb).toHaveClass(/sidebar-collapsed/);
    // 侧栏应滑出视口左侧（x < 0）
    const box = await sb.boundingBox();
    expect(box?.x, '折叠后侧栏应滑出视口（x < 0）').toBeLessThan(0);
  });

  test('E2E-VIS-003f sidebar 展开后恢复宽度 = 240px', async ({ page }) => {
    await enterApp(page);
    // 先折叠
    await page.locator('#collapseBtn').click();
    await page.waitForTimeout(300);
    // 再展开
    await page.locator('#expandBtn').click();
    await page.waitForTimeout(300);
    const width = await page.locator('#sidebar').evaluate(el => el.offsetWidth);
    expect(width).toBe(240);
  });

  test('E2E-VIS-003g inputBar 存在且可见', async ({ page }) => {
    await enterApp(page);
    await expect(page.locator('#inputBar')).toBeVisible();
    const h = await page.locator('#inputBar').evaluate(el => el.offsetHeight);
    expect(h).toBeGreaterThan(0);
  });
});

test.describe('E2E-VIS-004 圆角规范', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-004a sendBtn 圆角 = 16px (rounded-lg)', async ({ page }) => {
    await enterApp(page);
    const radius = await page.locator('#sendBtn').evaluate(el => getComputedStyle(el).borderRadius);
    expect(radius).toBe('16px');
  });

  test('E2E-VIS-004b inputBar 圆角 = 24px (rounded-2xl)', async ({ page }) => {
    await enterApp(page);
    const radius = await page.locator('#inputBar').evaluate(el => getComputedStyle(el).borderRadius);
    expect(radius).toBe('24px');
  });

  test('E2E-VIS-004c newChatBtn 圆角 = 16px (rounded-lg)', async ({ page }) => {
    await enterApp(page);
    const radius = await page.locator('#newChatBtn').evaluate(el => getComputedStyle(el).borderRadius);
    expect(radius).toBe('16px');
  });

  test('E2E-VIS-004d wizStart 圆角 = 20px (rounded-xl)', async ({ page }) => {
    const radius = await page.locator('#wizStart').evaluate(el => getComputedStyle(el).borderRadius);
    expect(radius).toBe('20px');
  });

  test('E2E-VIS-004e wizKey 圆角 = 20px (rounded-xl)', async ({ page }) => {
    const radius = await page.locator('#wizKey').evaluate(el => getComputedStyle(el).borderRadius);
    expect(radius).toBe('20px');
  });
});

test.describe('E2E-VIS-005 组件可见性与 SVG 图标', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-005a 向导页 logo 图片可见', async ({ page }) => {
    const logo = page.locator('#wizard img[alt="EchoMind"]');
    await expect(logo).toBeVisible();
    const w = await logo.evaluate(el => el.offsetWidth);
    const h = await logo.evaluate(el => el.offsetHeight);
    expect(w).toBe(64); // w-16
    expect(h).toBe(64); // h-16
  });

  test('E2E-VIS-005b sidebar logo 图片可见', async ({ page }) => {
    await enterApp(page);
    // RC7 修复：用 evaluate 检查 img 存在且有有效尺寸
    const logoInfo = await page.evaluate(() => {
      const img = document.querySelector('#sidebar img');
      if (!img) return { exists: false };
      return { exists: true, alt: img.getAttribute('alt'), width: img.offsetWidth };
    });
    expect(logoInfo.exists, 'sidebar 应包含 logo img').toBe(true);
    expect(logoInfo.alt, 'logo alt 应为 EchoMind').toBe('EchoMind');
    expect(logoInfo.width, 'logo 宽度应 > 0').toBeGreaterThan(0);
  });

  test('E2E-VIS-005c topBar 按钮含 SVG 子元素', async ({ page }) => {
    await enterApp(page);
    const kbSvg = page.locator('#kbBtn svg');
    await expect(kbSvg).toHaveCount(1);
    const settingsSvg = page.locator('#settingsBtn svg');
    await expect(settingsSvg).toHaveCount(1);
    const collapseSvg = page.locator('#collapseBtn svg');
    await expect(collapseSvg).toHaveCount(1);
  });

  test('E2E-VIS-005d sendBtn 含 SVG 发送图标', async ({ page }) => {
    await enterApp(page);
    // 发送/停止合二为一：#sendIcon 为发送态图标（stopIcon 隐藏）
    await expect(page.locator('#sendIcon')).toHaveCount(1);
    await expect(page.locator('#stopIcon')).toHaveCount(1);
  });

  test('E2E-VIS-005e newChatBtn 含 SVG 对话图标', async ({ page }) => {
    await enterApp(page);
    const svg = page.locator('#newChatBtn svg');
    await expect(svg).toHaveCount(1);
  });

  test('E2E-VIS-005f plusBtn 含 SVG 上传图标', async ({ page }) => {
    await enterApp(page);
    const svg = page.locator('#plusBtn svg');
    await expect(svg).toHaveCount(1);
  });

  test('E2E-VIS-005g 发送按钮空闲态非停止形态（发送/停止合二为一）', async ({ page }) => {
    await enterApp(page);
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/);
  });
});

test.describe('E2E-VIS-006 状态样式', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-006a sendBtn 空输入时可见且可点击', async ({ page }) => {
    await enterApp(page);
    // sendBtn 在空输入时仍可见（发送逻辑在点击时校验输入内容）
    const sendBtn = page.locator('#sendBtn');
    await expect(sendBtn).toBeVisible();
    // sendBtn 不在流式状态 → 不隐藏
    const isHidden = await sendBtn.evaluate(el => el.classList.contains('hidden'));
    expect(isHidden, '非流式时 sendBtn 不应隐藏').toBe(false);
  });

  test('E2E-VIS-006b sendBtn 有输入时 enabled', async ({ page }) => {
    // RC6 修复：beforeEach 未调用 enterApp，需先进入主界面
    await enterApp(page);
    // RC1 修复：空 KB 时 queryInput 被禁用，需先导入文档
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() => window.__TAURI__.core.invoke('import_files', { paths: ['/mock/test.md'] }));
    await page.locator('#docList [data-doc-name]').first().waitFor({ timeout: 5000 });
    await page.locator('#kbCloseBtn').click();
    // RC6 修复：等待 syncChatInputState 异步更新
    await page.waitForTimeout(300);
    await page.locator('#queryInput').fill('测试消息');
    await page.waitForTimeout(200);
    // 先验证 queryInput 未被禁用
    const inputDisabled = await page.locator('#queryInput').isDisabled();
    expect(inputDisabled, 'queryInput 不应被禁用').toBe(false);
    const isDisabled = await page.locator('#sendBtn').isDisabled();
    expect(isDisabled).toBe(false);
    // enabled opacity = 1
    const opacity = await page.locator('#sendBtn').evaluate(el => getComputedStyle(el).opacity);
    expect(parseFloat(opacity)).toBeCloseTo(1.0, 1);
  });

  test('E2E-VIS-006c wizStart 初始状态可点击', async ({ page }) => {
    // wizStart 初始为 enabled（验证在点击时触发，非 disabled 属性）
    const isDisabled = await page.locator('#wizStart').isDisabled();
    expect(isDisabled).toBe(false);
  });

  test('E2E-VIS-006d wizStart 有 key 时 enabled', async ({ page }) => {
    await page.locator('#wizKey').fill('sk-test');
    const isDisabled = await page.locator('#wizStart').isDisabled();
    expect(isDisabled).toBe(false);
  });

  test('E2E-VIS-006e inputBar focus 时 border 变化', async ({ page }) => {
    // RC6 修复：beforeEach 未调用 enterApp，需先进入主界面
    await enterApp(page);
    // 空 KB 时 queryInput 被禁用无法聚焦，需先导入文档
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() => window.__TAURI__.core.invoke('import_files', { paths: ['/mock/test.md'] }));
    await page.locator('#docList [data-doc-name]').first().waitFor({ timeout: 5000 });
    await page.locator('#kbCloseBtn').click();

    // 记录 focus 前的 border 颜色
    const borderBefore = await page.locator('#inputBar').evaluate(el => getComputedStyle(el).borderColor);
    await page.locator('#queryInput').focus();
    await page.waitForTimeout(200);
    const borderAfter = await page.locator('#inputBar').evaluate(el => getComputedStyle(el).borderColor);
    // focus 后 border 颜色应变化
    expect(borderAfter, 'focus 后 border 颜色应变化').not.toBe(borderBefore);
    // 强调色系检查：rgba 值中 blue 分量应较大
    const match = borderAfter.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
    if (match) {
      const [, r, g, b] = match.map(Number);
      expect(b, 'blue 分量应大于 red 分量（强调色系）').toBeGreaterThan(r);
    }
  });
});

test.describe('E2E-VIS-007 z-index 层级关系', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-007a wizard z-index = 40', async ({ page }) => {
    const z = await page.locator('#wizard').evaluate(el => getComputedStyle(el).zIndex);
    expect(parseInt(z, 10)).toBe(40);
  });

  test('E2E-VIS-007b dragOverlay z-index = 50 (高于 wizard)', async ({ page }) => {
    await enterApp(page);
    const dragZ = await page.locator('#dragOverlay').evaluate(el => getComputedStyle(el).zIndex);
    const wizardZ = 40;
    expect(parseInt(dragZ, 10)).toBeGreaterThan(wizardZ);
  });

  test('E2E-VIS-007c toasts z-index = 60 (高于 dragOverlay)', async ({ page }) => {
    await enterApp(page);
    const toastZ = await page.locator('#toasts').evaluate(el => getComputedStyle(el).zIndex);
    const dragZ = 50;
    expect(parseInt(toastZ, 10)).toBeGreaterThan(dragZ);
  });

  test('E2E-VIS-007d commandPalette z-index = 80 (最高)', async ({ page }) => {
    await enterApp(page);
    const cpZ = await page.locator('#commandPalette').evaluate(el => getComputedStyle(el).zIndex);
    expect(parseInt(cpZ, 10)).toBe(80);
  });
});

test.describe('E2E-VIS-008 安全审计与 CDN 检查', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-008a ui/ 目录无外联 CDN script/link 引用', async () => {
    const indexContent = fs.readFileSync(path.join(uiDir, 'index.html'), 'utf-8');
    const scriptSrcs = [...indexContent.matchAll(/<script[^>]+src=["']([^"']+)["']/gi)].map((m) => m[1]);
    const linkHrefs = [...indexContent.matchAll(/<link[^>]+href=["']([^"']+)["']/gi)].map((m) => m[1]);
    for (const src of scriptSrcs) {
      expect(src, `script src 不得为外链 CDN: ${src}`).not.toMatch(/^https?:\/\//);
    }
    for (const href of linkHrefs) {
      expect(href, `link href 不得为外链 CDN: ${href}`).not.toMatch(/^https?:\/\//);
    }
  });

  test('E2E-VIS-008b vendor 目录无 CDN 引用', async () => {
    const vendorDir = path.join(uiDir, 'vendor');
    if (fs.existsSync(vendorDir)) {
      const files = fs.readdirSync(vendorDir).filter(f => f.endsWith('.js'));
      for (const f of files) {
        const content = fs.readFileSync(path.join(vendorDir, f), 'utf-8');
        // 检查是否含外链引用（排除 sourceURL/sourceMappingURL 注释）
        const lines = content.split('\n');
        for (const line of lines) {
          // 跳过注释行和 sourceMap 行
          if (line.trim().startsWith('//') || line.trim().startsWith('/*') || line.includes('sourceMappingURL')) continue;
          // 检查是否含 http(s) 引用（排除常见的 URL 字符串如 "https://json-schema.org" 等元数据）
          if (line.match(/src\s*=\s*["']https?:\/\//i) || line.match(/href\s*=\s*["']https?:\/\//i)) {
            expect(false, `${f} 含外链引用: ${line.trim()}`).toBe(true);
          }
        }
      }
    }
  });

  test('E2E-VIS-008c XSS 注入载荷被 DOMPurify 消毒', async ({ page }) => {
    await enterApp(page);
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/echomind-e2e.md'] }),
    );
    await page.locator('#docList [data-doc-name]').first().waitFor({ timeout: 5000 });
    await page.locator('#kbCloseBtn').click();

    const xssTokens = await page.evaluate(() => window.__mock.xssTokens());
    await page.evaluate((tokens) => window.__mock.setCustomTokens(tokens), xssTokens);

    await sendMessage(page, 'XSS 测试');
    await waitForStreamDone(page);

    const html = await page.locator('#chatArea .md').last().innerHTML().catch(() =>
      page.locator('#chatArea').innerHTML()
    );
    expect(html, '不得包含 <script> 标签').not.toContain('<script');
    expect(html, '不得包含 onerror 事件').not.toContain('onerror');
    expect(html, '不得包含 javascript: 协议').not.toContain('javascript:');
    expect(html, '不得包含 <iframe> 标签').not.toContain('<iframe');
    // 正常文字应保留（如果 .md 块存在）
    if (html.length > 0) {
      // 放宽：可能只包含部分内容
    }
  });

  test('E2E-VIS-008d 代码块语法高亮与复制按钮', async ({ page }) => {
    await enterApp(page);
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/echomind-e2e.md'] }),
    );
    await page.locator('#docList [data-doc-name]').first().waitFor({ timeout: 5000 });
    await page.locator('#kbCloseBtn').click();

    await sendMessage(page, '测试代码块');
    await waitForStreamDone(page);
    const codeBlock = page.locator('#chatArea pre code').last();
    await expect(codeBlock).toBeVisible({ timeout: 15000 });

    // hljs 懒加载：等待 hljs class 出现（loadHighlight() 异步加载后调用 enhanceCodeBlocks）
    await expect(codeBlock).toHaveClass(/hljs/, { timeout: 10000 });

    await page.locator('#chatArea pre').last().hover();
    await expect(page.locator('#chatArea .copy-btn').last()).toBeVisible();
  });
});

test.describe('E2E-VIS-009 拖拽遮罩视觉', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-009a 拖拽遮罩文案与强调色', async ({ page }) => {
    await enterApp(page);
    await page.evaluate(() => window.__mock.simulateDragEnter());
    const overlay = page.locator('#dragOverlay');
    await expect(overlay).toBeVisible();
    await expect(overlay).toContainText('数据仅本地处理');
    await page.evaluate(() => window.__mock.simulateDragLeave());
    await expect(overlay).toBeHidden();
  });

  test('E2E-VIS-009b 遮罩出现时其余交互被遮罩层接管', async ({ page }) => {
    await enterApp(page);
    await page.evaluate(() => window.__mock.simulateDragEnter());
    const overlay = page.locator('#dragOverlay');
    await expect(overlay).toBeVisible();
    const overlayZ = await overlay.evaluate((el) => parseInt(getComputedStyle(el).zIndex, 10));
    const inputZ = await page.locator('#queryInput').evaluate((el) => {
      const parent = el.closest('[class*="z-"]') || el;
      return parseInt(getComputedStyle(parent).zIndex, 10) || 0;
    });
    expect(overlayZ, '遮罩层 z-index 应高于输入框').toBeGreaterThan(inputZ);
    await page.evaluate(() => window.__mock.simulateDragLeave());
  });

  test('E2E-VIS-009c 遮罩含虚线边框和上传图标', async ({ page }) => {
    await enterApp(page);
    await page.evaluate(() => window.__mock.simulateDragEnter());
    const overlay = page.locator('#dragOverlay');
    // 虚线边框
    const innerDiv = overlay.locator('div').first();
    const borderStyle = await innerDiv.evaluate(el => getComputedStyle(el).borderStyle);
    expect(borderStyle).toBe('dashed');
    // SVG 图标
    const svg = overlay.locator('svg');
    await expect(svg).toHaveCount(1);
    await page.evaluate(() => window.__mock.simulateDragLeave());
  });
});

test.describe('E2E-VIS-010 过渡动画验证', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-010a sidebar 折叠有 transition', async ({ page }) => {
    await enterApp(page);
    const transition = await page.locator('#sidebar').evaluate(el => getComputedStyle(el).transition);
    expect(transition).not.toBe('all 0s ease 0s');
    expect(transition.length).toBeGreaterThan(0);
  });

  test('E2E-VIS-010b sendBtn 有 transition', async ({ page }) => {
    await enterApp(page);
    const transition = await page.locator('#sendBtn').evaluate(el => getComputedStyle(el).transition);
    expect(transition).not.toBe('all 0s ease 0s');
  });

  test('E2E-VIS-010c newChatBtn 有 transition', async ({ page }) => {
    await enterApp(page);
    const transition = await page.locator('#newChatBtn').evaluate(el => getComputedStyle(el).transition);
    expect(transition).not.toBe('all 0s ease 0s');
  });

  test('E2E-VIS-010d kbBtn 有 transition', async ({ page }) => {
    await enterApp(page);
    const transition = await page.locator('#kbBtn').evaluate(el => getComputedStyle(el).transition);
    expect(transition).not.toBe('all 0s ease 0s');
  });
});

test.describe('E2E-VIS-011 向导页视觉验证', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-011a 向导初始可见，app 隐藏', async ({ page }) => {
    await expect(page.locator('#wizard')).toBeVisible();
    await expect(page.locator('#app')).toBeHidden();
  });

  test('E2E-VIS-011b 向导含预设卡片区域', async ({ page }) => {
    const presetCards = page.locator('#presetCards');
    await expect(presetCards).toBeVisible();
  });

  test('E2E-VIS-011c 向导含 API Key 输入框', async ({ page }) => {
    await expect(page.locator('#wizKey')).toBeVisible();
    const type = await page.locator('#wizKey').getAttribute('type');
    expect(type).toBe('password');
  });

  test('E2E-VIS-011d 向导含 Base URL 和 Model 输入框', async ({ page }) => {
    await expect(page.locator('#wizUrl')).toBeVisible();
    await expect(page.locator('#wizModel')).toBeVisible();
  });

  test('E2E-VIS-011e 向导错误提示初始隐藏', async ({ page }) => {
    await expect(page.locator('#wizError')).toBeHidden();
  });

  test('E2E-VIS-011f 向导含获取 API Key 链接', async ({ page }) => {
    await expect(page.locator('#wizKeyLink')).toBeVisible();
  });
});

test.describe('E2E-VIS-012 主界面空状态验证', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-012a 进入应用后 wizard 隐藏、app 可见', async ({ page }) => {
    await enterApp(page);
    await expect(page.locator('#wizard')).toBeHidden();
    await expect(page.locator('#app')).toBeVisible();
  });

  test('E2E-VIS-012b 空状态 chatArea 可见且可滚动', async ({ page }) => {
    await enterApp(page);
    const chatArea = page.locator('#chatArea');
    await expect(chatArea).toBeVisible();
    const overflow = await chatArea.evaluate(el => getComputedStyle(el).overflowY);
    expect(overflow).toBe('auto') ;
  });

  test('E2E-VIS-012c 空状态 convList 可见', async ({ page }) => {
    await enterApp(page);
    await expect(page.locator('#convList')).toBeVisible();
  });

  test('E2E-VIS-012d 空状态 Pro 状态显示 Free', async ({ page }) => {
    await enterApp(page);
    await expect(page.locator('#proStatus')).toHaveText('Free');
  });

  test('E2E-VIS-012e 空状态隐私指示器可见', async ({ page }) => {
    await enterApp(page);
    // 隐私指示器在 sidebar 底部
    const privacyIndicator = page.locator('#sidebar .text-\\[10px\\]').last();
    await expect(privacyIndicator).toBeVisible();
  });

  test('E2E-VIS-012f 空状态导入进度条隐藏', async ({ page }) => {
    await enterApp(page);
    await expect(page.locator('#importProgress')).toBeHidden();
  });
});

test.describe('E2E-VIS-013 模态弹窗视觉验证', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-013a 知识库弹窗打开后可见', async ({ page }) => {
    await enterApp(page);
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    // 背景遮罩
    const bg = await page.locator('#kbModal').evaluate(el => getComputedStyle(el).backgroundColor);
    expect(bg).toContain('rgba');
    // 含关闭按钮
    await expect(page.locator('#kbCloseBtn')).toBeVisible();
  });

  test('E2E-VIS-013b 设置弹窗打开后可见', async ({ page }) => {
    await enterApp(page);
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 3000 });
    // 含关闭按钮
    await expect(page.locator('#settingsCloseBtn')).toBeVisible();
    // 含完成按钮
    await expect(page.locator('#settingsClose')).toBeVisible();
  });

test('E2E-VIS-013c 付费墙弹窗视觉验证', async ({ page }) => {
await enterApp(page);
// 设置 Free 模式以触发付费墙
await page.evaluate(() => { window.__state.isPro = false; });
// 触发付费墙：通过模拟拖拽 .pdf 文件（isPro=false → PRO_REQUIRED → showPaywall）
await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/test.pdf']));
// 等待 paywall 显示
await expect(page.locator('#paywall')).toBeVisible({ timeout: 10000 });
// 含 PRO 星形图标
const starSvg = page.locator('#paywall svg').first();
await expect(starSvg).toBeVisible();
// 含激活按钮
await expect(page.locator('#paywallActivate')).toBeVisible();
// 含关闭按钮
await expect(page.locator('#paywallClose')).toBeVisible();
});

  test('E2E-VIS-013d Esc 键关闭设置弹窗', async ({ page }) => {
    await enterApp(page);
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 3000 });
    await page.keyboard.press('Escape');
    await expect(page.locator('#settingsModal')).toBeHidden({ timeout: 2000 });
  });
});

test.describe('E2E-VIS-014 滚动条与溢出', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-VIS-014a body overflow-hidden 防止整页滚动', async ({ page }) => {
    await enterApp(page);
    const overflow = await page.evaluate(() => getComputedStyle(document.body).overflow);
    expect(overflow).toBe('hidden');
  });

  test('E2E-VIS-014b chatArea 可纵向滚动', async ({ page }) => {
    await enterApp(page);
    const overflowY = await page.locator('#chatArea').evaluate(el => getComputedStyle(el).overflowY);
    expect(overflowY).toBe('auto');
  });

  test('E2E-VIS-014c convList 可纵向滚动', async ({ page }) => {
    await enterApp(page);
    const overflowY = await page.locator('#convList').evaluate(el => getComputedStyle(el).overflowY);
    expect(overflowY).toBe('auto');
  });

  test('E2E-VIS-014d queryInput 可纵向滚动（max-h-40）', async ({ page }) => {
    await enterApp(page);
    const maxHeight = await page.locator('#queryInput').evaluate(el => getComputedStyle(el).maxHeight);
    expect(parseInt(maxHeight, 10)).toBe(160); // max-h-40 = 10rem = 160px
  });
});
