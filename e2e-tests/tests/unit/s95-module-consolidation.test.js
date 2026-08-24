/**
 * S95: 前端模块精简 — TC-ARCH-MODULE-001~004 单元测试
 *
 * AC-1: 前端模块数 ≤60
 * AC-2: esbuild 构建成功且功能不变
 * AC-3: Vitest 全量测试通过无回归
 * AC-4: Release 构建中开发者工具前端模块不打包
 */
import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const srcDir = join(__dirname, '../../../ui/src');

// ============================================================
// TC-ARCH-MODULE-001: 前端模块数 ≤64
// V3.1 P4-3 拆分 main.js 上帝模块（kb-panel / conversation-list / disk-space），
// 单一职责优先于绝对数量，上限相应放宽至 64。

describe('TC-ARCH-MODULE-001: 前端模块数 ≤64', () => {
  it('ui/src/ 目录下 JS 模块数不超过 64', () => {
    const files = readdirSync(srcDir).filter(f => f.endsWith('.js'));
    expect(files.length).toBeLessThanOrEqual(64);
  });

  it('input-utils.js 存在（input-keymap + input-history 合并后）', () => {
    expect(existsSync(join(srcDir, 'input-utils.js'))).toBe(true);
  });

  it('input-keymap.js 已删除', () => {
    expect(existsSync(join(srcDir, 'input-keymap.js'))).toBe(false);
  });

  it('input-history.js 已删除', () => {
    expect(existsSync(join(srcDir, 'input-history.js'))).toBe(false);
  });
});

// ============================================================
// TC-ARCH-MODULE-002: 合并后 esbuild 构建成功（验证打包文件存在）
// ============================================================

describe('TC-ARCH-MODULE-002: 合并后构建产物', () => {
  it('ui/index.html 存在且包含 esbuild 打包内联脚本', () => {
    const htmlPath = join(__dirname, '../../../ui/index.html');
    expect(existsSync(htmlPath)).toBe(true);
    const html = readFileSync(htmlPath, 'utf-8');
    // 验证 <script> 块存在（打包后的内联脚本）
    expect(html).toContain('<script>');
  });
});

// ============================================================
// TC-ARCH-MODULE-003: 合并后功能不变（验证导出）
// ============================================================

describe('TC-ARCH-MODULE-003: input-utils.js 导出完整性', () => {
  it('导出 createInputKeyHandler 函数', async () => {
    // 动态导入验证模块可加载
    const module = await import('../../../ui/src/input-utils.js');
    expect(typeof module.createInputKeyHandler).toBe('function');
  });

  it('导出 isComposingEvent 函数', async () => {
    const module = await import('../../../ui/src/input-utils.js');
    expect(typeof module.isComposingEvent).toBe('function');
  });

  it('导出 createImeGuard 函数', async () => {
    const module = await import('../../../ui/src/input-utils.js');
    expect(typeof module.createImeGuard).toBe('function');
  });

  it('导出 recordInput 函数', async () => {
    const module = await import('../../../ui/src/input-utils.js');
    expect(typeof module.recordInput).toBe('function');
  });

  it('导出 navigateHistoryUp 函数', async () => {
    const module = await import('../../../ui/src/input-utils.js');
    expect(typeof module.navigateHistoryUp).toBe('function');
  });

  it('导出 navigateHistoryDown 函数', async () => {
    const module = await import('../../../ui/src/input-utils.js');
    expect(typeof module.navigateHistoryDown).toBe('function');
  });

  it('导出 resetHistoryNav 函数', async () => {
    const module = await import('../../../ui/src/input-utils.js');
    expect(typeof module.resetHistoryNav).toBe('function');
  });

  it('导出 saveDraft 函数', async () => {
    const module = await import('../../../ui/src/input-utils.js');
    expect(typeof module.saveDraft).toBe('function');
  });

  it('导出 restoreDraft 函数', async () => {
    const module = await import('../../../ui/src/input-utils.js');
    expect(typeof module.restoreDraft).toBe('function');
  });

  it('导出 updateTokenEstimate 函数', async () => {
    const module = await import('../../../ui/src/input-utils.js');
    expect(typeof module.updateTokenEstimate).toBe('function');
  });

  it('导出 checkPopups 函数', async () => {
    const module = await import('../../../ui/src/input-utils.js');
    expect(typeof module.checkPopups).toBe('function');
  });
});

// ============================================================
// TC-ARCH-MODULE-004: Release 构建排除开发者工具模块
// ============================================================

describe('TC-ARCH-MODULE-004: build-ui.mjs Release 模式配置', () => {
  it('build-ui.mjs 包含 DEV_TOOL_PLUGINS 配置', () => {
    const buildScript = readFileSync(join(__dirname, '../../../scripts/build-ui.mjs'), 'utf-8');
    expect(buildScript).toContain('DEV_TOOL_PLUGINS');
    expect(buildScript).toContain('isReleaseMode');
  });

  it('排除 trace-panel 模块的 onResolve 配置', () => {
    const buildScript = readFileSync(join(__dirname, '../../../scripts/build-ui.mjs'), 'utf-8');
    expect(buildScript).toContain('trace-panel');
    expect(buildScript).toContain('dev-tool-excluded');
  });

  it('排除 rag-eval 模块的 onResolve 配置', () => {
    const buildScript = readFileSync(join(__dirname, '../../../scripts/build-ui.mjs'), 'utf-8');
    expect(buildScript).toContain('rag-eval');
  });

  it('settings.js 包含防御性 typeof 检查（Release 模式函数为 undefined）', () => {
    const settingsJs = readFileSync(join(srcDir, 'settings.js'), 'utf-8');
    expect(settingsJs).toContain("typeof renderRagEvalSettings === 'function'");
    expect(settingsJs).toContain("typeof renderTraceBudgetSettings === 'function'");
  });
});
