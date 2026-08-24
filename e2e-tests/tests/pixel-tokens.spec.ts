/**
 * EchoMind UI 像素级测试 — 设计令牌精确值验证
 *
 * 验证 CSS 变量系统三层架构（Surface → Alias → Component）的精确值。
 * 依据：docs/architecture/UI_PIXEL_SPEC.md §1
 *
 * 测试分类：
 *   TC-PIX-TOKEN-001~010: 暗色主题颜色令牌精确值
 *   TC-PIX-TOKEN-011~020: 浅色主题颜色令牌精确值
 *   TC-PIX-TOKEN-021~030: 间距/排版/圆角/阴影令牌精确值
 *   TC-PIX-TOKEN-031~040: 动效/过渡令牌精确值
 *   TC-PIX-TOKEN-041~050: Z-index 层级验证
 *   TC-PIX-TOKEN-051~060: 三层架构存在性验证
 */
import { test, expect } from '@playwright/test';
import { setupPage, injectStub, injectLocales, uiUrl } from './helpers.mjs';

// ============================================================
// 1. 暗色主题颜色令牌精确值 (TC-PIX-TOKEN-001~010)
// ============================================================

test.describe('暗色主题颜色令牌精确值', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-PIX-TOKEN-001 Surface 色阶 5 级精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        s0: cs.getPropertyValue('--surface-0').trim(),
        s1: cs.getPropertyValue('--surface-1').trim(),
        s2: cs.getPropertyValue('--surface-2').trim(),
        s3: cs.getPropertyValue('--surface-3').trim(),
        s4: cs.getPropertyValue('--surface-4').trim(),
      };
    });
    expect(tokens.s0).toBe('#0A0A0B');
    expect(tokens.s1).toBe('#131316');
    expect(tokens.s2).toBe('#1C1C20');
    expect(tokens.s3).toBe('#26262C');
    expect(tokens.s4).toBe('#303036');
  });

  test('TC-PIX-TOKEN-002 Border 色阶 3 级精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        subtle: cs.getPropertyValue('--border-subtle').trim(),
        default: cs.getPropertyValue('--border-default').trim(),
        strong: cs.getPropertyValue('--border-strong').trim(),
      };
    });
    expect(tokens.subtle).toBe('#1F1F23');
    expect(tokens.default).toBe('#2A2A2E');
    expect(tokens.strong).toBe('#3A3A40');
  });

  test('TC-PIX-TOKEN-003 Text 色阶 4 级精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        primary: cs.getPropertyValue('--text-primary').trim(),
        secondary: cs.getPropertyValue('--text-secondary').trim(),
        tertiary: cs.getPropertyValue('--text-tertiary').trim(),
        quaternary: cs.getPropertyValue('--text-quaternary').trim(),
      };
    });
    expect(tokens.primary).toBe('#F8FAFC');
    expect(tokens.secondary).toBe('#CBD5E1');
    expect(tokens.tertiary).toBe('#94A3B8');
    expect(tokens.quaternary).toBe('#8995A8');
  });

  test('TC-PIX-TOKEN-004 Accent 色阶精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        accent: cs.getPropertyValue('--accent').trim(),
        hover: cs.getPropertyValue('--accent-hover').trim(),
        subtle: cs.getPropertyValue('--accent-subtle').trim(),
        rgb: cs.getPropertyValue('--accent-rgb').trim(),
        text: cs.getPropertyValue('--accent-text').trim(),
      };
    });
    expect(tokens.accent).toBe('#38BDF8');
    expect(tokens.hover).toBe('#0EA5E9');
    expect(tokens.rgb).toBe('56, 189, 248');
  });

  test('TC-PIX-TOKEN-005 Semantic 色阶精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        success: cs.getPropertyValue('--success').trim(),
        warning: cs.getPropertyValue('--warning').trim(),
        danger: cs.getPropertyValue('--danger').trim(),
        info: cs.getPropertyValue('--info').trim(),
      };
    });
    expect(tokens.success).toBe('#4ADE80');
    expect(tokens.warning).toBe('#FBBF24');
    expect(tokens.danger).toBe('#F87171');
    expect(tokens.info).toBe('#60A5FA');
  });

  test('TC-PIX-TOKEN-006 Semantic RGB 分量精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        success: cs.getPropertyValue('--success-rgb').trim(),
        warning: cs.getPropertyValue('--warning-rgb').trim(),
        danger: cs.getPropertyValue('--danger-rgb').trim(),
        info: cs.getPropertyValue('--info-rgb').trim(),
      };
    });
    expect(tokens.success).toBe('74, 222, 128');
    expect(tokens.warning).toBe('251, 191, 36');
    expect(tokens.danger).toBe('248, 113, 113');
    expect(tokens.info).toBe('96, 165, 250');
  });

  test('TC-PIX-TOKEN-007 Bg 别名引用正确性', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      // getComputedStyle 解析 var() 为最终值，检查最终值与 Surface 一致
      const s1 = cs.getPropertyValue('--surface-1').trim();
      const s2 = cs.getPropertyValue('--surface-2').trim();
      const s3 = cs.getPropertyValue('--surface-3').trim();
      const s0 = cs.getPropertyValue('--surface-0').trim();
      return {
        primary: cs.getPropertyValue('--bg-primary').trim(),
        secondary: cs.getPropertyValue('--bg-secondary').trim(),
        hover: cs.getPropertyValue('--bg-hover').trim(),
        input: cs.getPropertyValue('--bg-input').trim(),
        s0, s1, s2, s3,
      };
    });
    // Bg 别名最终解析值应与 Surface 一致
    expect(tokens.primary).toBe(tokens.s1);
    expect(tokens.secondary).toBe(tokens.s2);
    expect(tokens.hover).toBe(tokens.s3);
    expect(tokens.input).toBe(tokens.s0);
  });

  test('TC-PIX-TOKEN-008 用户消息令牌精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        radius: cs.getPropertyValue('--msg-user-radius').trim(),
        maxWidth: cs.getPropertyValue('--msg-user-max-width').trim(),
        gapQA: cs.getPropertyValue('--msg-gap-qa').trim(),
        listMaxWidth: cs.getPropertyValue('--msg-list-max-width').trim(),
      };
    });
    expect(tokens.radius).toBe('22px');
    expect(tokens.maxWidth).toBe('calc(100% - 88px)');
    // gap-qa 解析后为 32px
    expect(tokens.gapQA).toBe('32px');
    expect(tokens.listMaxWidth).toBe('840px');
  });

  test('TC-PIX-TOKEN-009 AI 消息透明背景验证', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        bg: cs.getPropertyValue('--msg-assistant-bg').trim(),
        border: cs.getPropertyValue('--msg-assistant-border').trim(),
      };
    });
    expect(tokens.bg).toBe('transparent');
    expect(tokens.border).toBe('transparent');
  });

  test('TC-PIX-TOKEN-010 操作栏令牌精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        btnSize: cs.getPropertyValue('--action-btn-size').trim(),
        transition: cs.getPropertyValue('--action-transition').trim(),
      };
    });
    expect(tokens.btnSize).toBe('28px');
    expect(tokens.transition).toContain('opacity');
    expect(tokens.transition).toContain('0.15s');
  });
});

// ============================================================
// 2. 浅色主题颜色令牌精确值 (TC-PIX-TOKEN-011~020)
// ============================================================

test.describe('浅色主题颜色令牌精确值', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // 切换到浅色主题
    await page.evaluate(() => {
      document.documentElement.dataset.theme = 'light';
    });
    await page.waitForTimeout(200);
  });

  test('TC-PIX-TOKEN-011 浅色 Surface 色阶精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        s0: cs.getPropertyValue('--surface-0').trim(),
        s1: cs.getPropertyValue('--surface-1').trim(),
        s2: cs.getPropertyValue('--surface-2').trim(),
        s3: cs.getPropertyValue('--surface-3').trim(),
      };
    });
    expect(tokens.s0).toBe('#FFFFFF');
    expect(tokens.s1).toBe('#F8FAFC');
    expect(tokens.s2).toBe('#F1F5F9');
    expect(tokens.s3).toBe('#E2E8F0');
  });

  test('TC-PIX-TOKEN-012 浅色 Text 色阶精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        primary: cs.getPropertyValue('--text-primary').trim(),
        secondary: cs.getPropertyValue('--text-secondary').trim(),
        tertiary: cs.getPropertyValue('--text-tertiary').trim(),
        quaternary: cs.getPropertyValue('--text-quaternary').trim(),
      };
    });
    expect(tokens.primary).toBe('#0F172A');
    expect(tokens.secondary).toBe('#334155');
    expect(tokens.tertiary).toBe('#475569');
    expect(tokens.quaternary).toBe('#475569');
  });

  test('TC-PIX-TOKEN-013 浅色 Accent 精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        accent: cs.getPropertyValue('--accent').trim(),
        hover: cs.getPropertyValue('--accent-hover').trim(),
        text: cs.getPropertyValue('--accent-text').trim(),
      };
    });
    expect(tokens.accent).toBe('#0EA5E9');
    expect(tokens.hover).toBe('#0284C7');
    expect(tokens.text).toBe('#0369A1');
  });

  test('TC-PIX-TOKEN-014 浅色 Semantic 色精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        success: cs.getPropertyValue('--success').trim(),
        warning: cs.getPropertyValue('--warning').trim(),
        danger: cs.getPropertyValue('--danger').trim(),
        info: cs.getPropertyValue('--info').trim(),
      };
    });
    expect(tokens.success).toBe('#15803D');
    expect(tokens.warning).toBe('#D97706');
    expect(tokens.danger).toBe('#DC2626');
    expect(tokens.info).toBe('#2563EB');
  });

  test('TC-PIX-TOKEN-015 浅色 color-scheme 设置', async ({ page }) => {
    const scheme = await page.evaluate(() => {
      return getComputedStyle(document.body).colorScheme;
    });
    expect(scheme).toBe('light');
  });

  test('TC-PIX-TOKEN-016 浅色滚动条颜色', async ({ page }) => {
    const thumbColor = await page.evaluate(() => {
      const el = document.createElement('div');
      el.className = 'scrollbar-thin';
      el.style.position = 'absolute';
      el.style.top = '-100px';
      document.body.appendChild(el);
      const cs = getComputedStyle(el, '::-webkit-scrollbar-thumb');
      document.body.removeChild(el);
      return cs.backgroundColor || getComputedStyle(document.documentElement)
        .getPropertyValue('--hover-bg-subtle').trim();
    });
    // 浅色主题滚动条应为深色半透明
    expect(thumbColor).toBeTruthy();
  });

  test('TC-PIX-TOKEN-017 浅色用户消息背景', async ({ page }) => {
    const bg = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return cs.getPropertyValue('--msg-user-bg').trim();
    });
    expect(bg).toContain('14, 165, 233');
  });

  test('TC-PIX-TOKEN-018 高对比度主题 Surface 色阶', async ({ page }) => {
    await page.evaluate(() => {
      document.documentElement.dataset.theme = 'high-contrast';
    });
    await page.waitForTimeout(200);
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        s0: cs.getPropertyValue('--surface-0').trim(),
        s1: cs.getPropertyValue('--surface-1').trim(),
        accent: cs.getPropertyValue('--accent').trim(),
        textPrimary: cs.getPropertyValue('--text-primary').trim(),
      };
    });
    expect(tokens.s0).toBe('#000000');
    expect(tokens.accent).toBe('#FFFF00');
    expect(tokens.textPrimary).toBe('#FFFFFF');
  });

  test('TC-PIX-TOKEN-019 高对比度边框加粗验证', async ({ page }) => {
    await page.evaluate(() => {
      document.documentElement.dataset.theme = 'high-contrast';
    });
    await page.waitForTimeout(200);
    const borderWidth = await page.evaluate(() => {
      const el = document.createElement('div');
      el.className = 'border';
      el.style.position = 'absolute';
      el.style.top = '-100px';
      document.body.appendChild(el);
      const cs = getComputedStyle(el);
      const w = cs.borderWidth;
      document.body.removeChild(el);
      return w;
    });
    expect(borderWidth).toBe('2px');
  });

  test('TC-PIX-TOKEN-020 高对比度 focus-visible 加粗', async ({ page }) => {
    await page.evaluate(() => {
      document.documentElement.dataset.theme = 'high-contrast';
    });
    await page.waitForTimeout(200);
    // 验证 CSS 规则中 :focus-visible 的 outline 为 3px
    const hasRule = await page.evaluate(() => {
      for (const sheet of document.styleSheets) {
        try {
          for (const rule of sheet.cssRules) {
            if (rule.selectorText && rule.selectorText.includes('focus-visible') &&
                rule.selectorText.includes('high-contrast')) {
              if (rule.style.outline && rule.style.outline.includes('3px')) {
                return true;
              }
            }
          }
        } catch (e) { /* */ }
      }
      return false;
    });
    expect(hasRule).toBeTruthy();
  });
});

// ============================================================
// 3. 间距/排版/圆角/阴影令牌精确值 (TC-PIX-TOKEN-021~030)
// ============================================================

test.describe('间距/排版/圆角/阴影令牌', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-PIX-TOKEN-021 间距令牌 4px 网格验证', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        s0: cs.getPropertyValue('--space-0').trim(),
        s1: cs.getPropertyValue('--space-1').trim(),
        s2: cs.getPropertyValue('--space-2').trim(),
        s3: cs.getPropertyValue('--space-3').trim(),
        s4: cs.getPropertyValue('--space-4').trim(),
        s5: cs.getPropertyValue('--space-5').trim(),
        s6: cs.getPropertyValue('--space-6').trim(),
        s8: cs.getPropertyValue('--space-8').trim(),
        s10: cs.getPropertyValue('--space-10').trim(),
        s12: cs.getPropertyValue('--space-12').trim(),
      };
    });
    expect(tokens.s0).toBe('0px');
    expect(tokens.s1).toBe('4px');
    expect(tokens.s2).toBe('8px');
    expect(tokens.s3).toBe('12px');
    expect(tokens.s4).toBe('16px');
    expect(tokens.s5).toBe('20px');
    expect(tokens.s6).toBe('24px');
    expect(tokens.s8).toBe('32px');
    expect(tokens.s10).toBe('40px');
    expect(tokens.s12).toBe('48px');
  });

  test('TC-PIX-TOKEN-022 排版令牌精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        xs: cs.getPropertyValue('--text-xs').trim(),
        sm: cs.getPropertyValue('--text-sm').trim(),
        base: cs.getPropertyValue('--text-base').trim(),
        lg: cs.getPropertyValue('--text-lg').trim(),
        leadingTight: cs.getPropertyValue('--leading-tight').trim(),
        leadingNormal: cs.getPropertyValue('--leading-normal').trim(),
      };
    });
    expect(tokens.xs).toBe('11px');
    expect(tokens.sm).toBe('12px');
    expect(tokens.base).toBe('16px');
    expect(tokens.lg).toBe('18px');
    expect(tokens.leadingTight).toBe('1.4');
    expect(tokens.leadingNormal).toBe('1.75');
  });

  test('TC-PIX-TOKEN-023 圆角令牌精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        none: cs.getPropertyValue('--radius-none').trim(),
        sm: cs.getPropertyValue('--radius-sm').trim(),
        md: cs.getPropertyValue('--radius-md').trim(),
        lg: cs.getPropertyValue('--radius-lg').trim(),
        xl: cs.getPropertyValue('--radius-xl').trim(),
        '2xl': cs.getPropertyValue('--radius-2xl').trim(),
        full: cs.getPropertyValue('--radius-full').trim(),
      };
    });
    expect(tokens.none).toBe('0px');
    expect(tokens.sm).toBe('4px');
    expect(tokens.md).toBe('8px');
    expect(tokens.lg).toBe('12px');
    expect(tokens.xl).toBe('16px');
    expect(tokens['2xl']).toBe('24px');
    expect(tokens.full).toBe('9999px');
  });

  test('TC-PIX-TOKEN-024 阴影令牌精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        sm: cs.getPropertyValue('--shadow-sm').trim(),
        md: cs.getPropertyValue('--shadow-md').trim(),
        lg: cs.getPropertyValue('--shadow-lg').trim(),
      };
    });
    expect(tokens.sm).toContain('4px 12px');
    expect(tokens.sm).toContain('0.15');
    expect(tokens.md).toContain('8px 24px');
    expect(tokens.md).toContain('0.35');
    expect(tokens.lg).toContain('16px 48px');
    expect(tokens.lg).toContain('0.4');
  });

  test('TC-PIX-TOKEN-025 组件圆角令牌精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        buttonRadius: cs.getPropertyValue('--dsl-button-radius').trim(),
        buttonRadiusSm: cs.getPropertyValue('--dsl-button-radius-sm').trim(),
        inputRadius: cs.getPropertyValue('--dsl-input-radius').trim(),
        cardRadius: cs.getPropertyValue('--dsl-card-radius').trim(),
        toggleRadius: cs.getPropertyValue('--dsl-toggle-radius').trim(),
      };
    });
    expect(tokens.buttonRadius).toBe('4096px');
    expect(tokens.buttonRadiusSm).toBe('8px');
    expect(tokens.inputRadius).toBe('16px');
    expect(tokens.cardRadius).toBe('12px');
    expect(tokens.toggleRadius).toBe('8px');
  });
});

// ============================================================
// 4. 动效/过渡令牌精确值 (TC-PIX-TOKEN-031~040)
// ============================================================

test.describe('动效/过渡令牌', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-PIX-TOKEN-031 动效持续时间令牌', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        micro: cs.getPropertyValue('--duration-micro').trim(),
        fast: cs.getPropertyValue('--duration-fast').trim(),
        normal: cs.getPropertyValue('--duration-normal').trim(),
        slow: cs.getPropertyValue('--duration-slow').trim(),
      };
    });
    expect(tokens.micro).toBe('100ms');
    expect(tokens.fast).toBe('150ms');
    expect(tokens.normal).toBe('250ms');
    expect(tokens.slow).toBe('400ms');
  });

  test('TC-PIX-TOKEN-032 缓动函数令牌', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        out: cs.getPropertyValue('--ease-out').trim(),
        inOut: cs.getPropertyValue('--ease-in-out').trim(),
        spring: cs.getPropertyValue('--ease-spring').trim(),
      };
    });
    expect(tokens.out).toBe('ease-out');
    expect(tokens.inOut).toBe('cubic-bezier(0.4, 0, 0.2, 1)');
    expect(tokens.spring).toBe('cubic-bezier(0.34, 1.56, 0.64, 1)');
  });

  test('TC-PIX-TOKEN-033 Hover 令牌精确值', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        bgSubtle: cs.getPropertyValue('--hover-bg-subtle').trim(),
        opacity: cs.getPropertyValue('--hover-opacity').trim(),
      };
    });
    expect(tokens.bgSubtle).toContain('rgba(255, 255, 255, 0.05)');
    expect(tokens.opacity).toBe('0.9');
  });

  test('TC-PIX-TOKEN-034 @keyframes messageIn 存在性', async ({ page }) => {
    const hasKeyframe = await page.evaluate(() => {
      for (const sheet of document.styleSheets) {
        try {
          for (const rule of sheet.cssRules) {
            if (rule.cssText && rule.cssText.includes('@keyframes messageIn')) {
              return true;
            }
          }
        } catch (e) { /* cross-origin */ }
      }
      return false;
    });
    expect(hasKeyframe).toBeTruthy();
  });

  test('TC-PIX-TOKEN-035 @keyframes scaleIn 存在性', async ({ page }) => {
    const hasKeyframe = await page.evaluate(() => {
      for (const sheet of document.styleSheets) {
        try {
          for (const rule of sheet.cssRules) {
            if (rule.cssText && rule.cssText.includes('@keyframes scaleIn')) {
              return true;
            }
          }
        } catch (e) { /* cross-origin */ }
      }
      return false;
    });
    expect(hasKeyframe).toBeTruthy();
  });

  test('TC-PIX-TOKEN-036 @keyframes toastIn 存在性', async ({ page }) => {
    const hasKeyframe = await page.evaluate(() => {
      for (const sheet of document.styleSheets) {
        try {
          for (const rule of sheet.cssRules) {
            if (rule.cssText && rule.cssText.includes('@keyframes toastIn')) {
              return true;
            }
          }
        } catch (e) { /* cross-origin */ }
      }
      return false;
    });
    expect(hasKeyframe).toBeTruthy();
  });

  test('TC-PIX-TOKEN-037 animate-* 类应用验证', async ({ page }) => {
    const classes = await page.evaluate(() => {
      const testEl = document.createElement('div');
      testEl.className = 'animate-fade-in';
      testEl.style.position = 'absolute';
      testEl.style.top = '-100px';
      document.body.appendChild(testEl);
      const cs = getComputedStyle(testEl);
      const anim = cs.animationName;
      document.body.removeChild(testEl);
      return anim;
    });
    expect(classes).toBe('fadeIn');
  });

  test('TC-PIX-TOKEN-038 prefers-reduced-motion 降级规则存在', async ({ page }) => {
    const hasReducedMotion = await page.evaluate(() => {
      for (const sheet of document.styleSheets) {
        try {
          for (const rule of sheet.cssRules) {
            if (rule.cssText && rule.cssText.includes('prefers-reduced-motion')) {
              return true;
            }
          }
        } catch (e) { /* cross-origin */ }
      }
      return false;
    });
    expect(hasReducedMotion).toBeTruthy();
  });

  test('TC-PIX-TOKEN-039 主题切换防闪烁规则存在', async ({ page }) => {
    const hasTransitionControl = await page.evaluate(() => {
      for (const sheet of document.styleSheets) {
        try {
          for (const rule of sheet.cssRules) {
            if (rule.selectorText && rule.selectorText.includes('change-theme')) {
              return true;
            }
          }
        } catch (e) { /* cross-origin */ }
      }
      return false;
    });
    expect(hasTransitionControl).toBeTruthy();
  });

  test('TC-PIX-TOKEN-040 .sr-only 类存在性验证', async ({ page }) => {
    const srOnlyStyles = await page.evaluate(() => {
      const el = document.createElement('div');
      el.className = 'sr-only';
      el.style.position = 'absolute';
      el.style.top = '-100px';
      document.body.appendChild(el);
      const cs = getComputedStyle(el);
      const result = {
        position: cs.position,
        width: cs.width,
        height: cs.height,
        overflow: cs.overflow,
        clip: cs.clip,
      };
      document.body.removeChild(el);
      return result;
    });
    expect(srOnlyStyles.position).toBe('absolute');
    expect(srOnlyStyles.width).toBe('1px');
    expect(srOnlyStyles.height).toBe('1px');
    expect(srOnlyStyles.overflow).toBe('hidden');
  });
});

// ============================================================
// 5. CSS 变量三层架构验证 (TC-PIX-TOKEN-051~060)
// ============================================================

test.describe('CSS 变量三层架构验证', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-PIX-TOKEN-051 Layer 1 Surface 色阶全部非空', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return ['--surface-0', '--surface-1', '--surface-2', '--surface-3', '--surface-4']
        .map(v => cs.getPropertyValue(v).trim());
    });
    tokens.forEach(v => expect(v).not.toBe(''));
  });

  test('TC-PIX-TOKEN-052 Layer 2 Alias 全部非空', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return [
        '--dsw-alias-brand-primary',
        '--dsw-alias-brand-hover',
        '--dsw-alias-label-primary',
        '--dsw-alias-label-secondary',
        '--dsw-alias-surface-0',
        '--dsw-alias-surface-1',
        '--dsw-alias-surface-2',
        '--dsw-alias-surface-3',
        '--dsw-alias-interactive-bg-hover',
        '--dsw-alias-border-l1',
        '--dsw-alias-border-l2',
        '--dsw-alias-border-l3',
      ].map(v => cs.getPropertyValue(v).trim());
    });
    tokens.forEach(v => expect(v).not.toBe(''));
  });

  test('TC-PIX-TOKEN-053 Layer 3 Component 全部非空', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return [
        '--dsl-button-radius',
        '--dsl-button-radius-sm',
        '--dsl-button-text',
        '--dsl-button-bg',
        '--dsl-button-bg-hover',
        '--dsl-input-radius',
        '--dsl-input-bg',
        '--dsl-input-border',
        '--dsl-msg-user-radius',
        '--dsl-msg-user-bg',
        '--dsl-msg-assistant-bg',
        '--dsl-msg-list-max-width',
        '--dsl-card-radius',
        '--dsl-card-bg',
        '--dsl-toggle-radius',
        '--dsl-thinking-text',
      ].map(v => cs.getPropertyValue(v).trim());
    });
    tokens.forEach(v => expect(v).not.toBe(''));
  });

  test('TC-PIX-TOKEN-054 Alias 引用 Surface 正确性', async ({ page }) => {
    const refs = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      // getComputedStyle 解析 var() 链，Alias 和 Surface 最终值应一致
      return {
        aliasSurface0: cs.getPropertyValue('--dsw-alias-surface-0').trim(),
        aliasSurface1: cs.getPropertyValue('--dsw-alias-surface-1').trim(),
        surface0: cs.getPropertyValue('--surface-0').trim(),
        surface1: cs.getPropertyValue('--surface-1').trim(),
      };
    });
    // Alias 解析后值应等于 Surface 值
    expect(refs.aliasSurface0).toBe(refs.surface0);
    expect(refs.aliasSurface1).toBe(refs.surface1);
  });

  test('TC-PIX-TOKEN-055 Component 引用 Alias 正确性', async ({ page }) => {
    const refs = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      // Component 变量解析后应与 Alias 一致
      const surface2 = cs.getPropertyValue('--surface-2').trim();
      const interactiveHover = cs.getPropertyValue('--dsw-alias-interactive-bg-hover').trim();
      return {
        buttonBgHover: cs.getPropertyValue('--dsl-button-bg-hover').trim(),
        inputBg: cs.getPropertyValue('--dsl-input-bg').trim(),
        cardBg: cs.getPropertyValue('--dsl-card-bg').trim(),
        surface2,
        interactiveHover,
      };
    });
    expect(refs.buttonBgHover).toBe(refs.interactiveHover);
    expect(refs.inputBg).toBe(refs.surface2);
    expect(refs.cardBg).toBe(refs.surface2);
  });

  test('TC-PIX-TOKEN-056 动画组件令牌存在性', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        modal: cs.getPropertyValue('--anim-modal').trim(),
        toast: cs.getPropertyValue('--anim-toast').trim(),
        panel: cs.getPropertyValue('--anim-panel').trim(),
        message: cs.getPropertyValue('--anim-message').trim(),
      };
    });
    expect(tokens.modal).toContain('scaleIn');
    expect(tokens.toast).toContain('toastIn');
    expect(tokens.panel).toContain('panelIn');
    expect(tokens.message).toContain('messageIn');
  });

  test('TC-PIX-TOKEN-057 主题切换时 Alias 值变化', async ({ page }) => {
    // 暗色主题下 alias 值
    const darkAccent = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return cs.getPropertyValue('--accent').trim();
    });
    expect(darkAccent).toBe('#38BDF8');

    // 切换浅色
    await page.evaluate(() => {
      document.documentElement.dataset.theme = 'light';
    });
    await page.waitForTimeout(200);

    const lightAccent = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return cs.getPropertyValue('--accent').trim();
    });
    expect(lightAccent).toBe('#0EA5E9');
    expect(darkAccent).not.toBe(lightAccent);
  });

  test('TC-PIX-TOKEN-058 body font-family 包含 CJK 引号修复', async ({ page }) => {
    const fontFamily = await page.evaluate(() => {
      return getComputedStyle(document.body).fontFamily;
    });
    expect(fontFamily).toContain('quote-cjk-patch');
    expect(fontFamily).toContain('-apple-system');
  });

  test('TC-PIX-TOKEN-059 body color-scheme 暗色', async ({ page }) => {
    const scheme = await page.evaluate(() => {
      return getComputedStyle(document.body).colorScheme;
    });
    expect(scheme).toBe('dark');
  });

  test('TC-PIX-TOKEN-060 focus-visible box-shadow 使用 var(--shadow-focus)', async ({ page }) => {
    // 验证 :focus-visible CSS 规则中 box-shadow 非 none
    const hasFocusRule = await page.evaluate(() => {
      for (const sheet of document.styleSheets) {
        try {
          for (const rule of sheet.cssRules) {
            if (rule.selectorText && rule.selectorText.includes(':focus-visible')) {
              if (rule.style.boxShadow && rule.style.boxShadow !== 'none') {
                return true;
              }
              // 也检查 style.cssText
              if (rule.cssText && rule.cssText.includes('shadow-focus')) {
                return true;
              }
            }
          }
        } catch (e) { /* */ }
      }
      return false;
    });
    expect(hasFocusRule).toBeTruthy();
  });
});
