#!/usr/bin/env node

/**
 * EchoMind 前端构建脚本 — 双模式：开发（外部引用）vs 生产（全内联）。
 *
 * 生产模式（默认）：node scripts/build-ui.mjs
 *   - CSS `<link href="styles/*.css">` → 内联 `<style>` 块
 *   - JS 使用 esbuild 从 src/main.js 打包为 IIFE → 内联 `<script>` 块
 *   - 输出：单文件 ui/index.html（无外部 CSS/JS 依赖，兼容 file:// 协议）
 *
 * 开发模式：node scripts/build-ui.mjs --dev
 *   - 内联 `<style>` → 外部 `<link rel="stylesheet" href="styles/*.css">`
 *   - 内联 `<script>` → `<script type="module" src="src/main.js">`
 *   - 输出：多文件引用（支持浏览器原生 ES module + CSS 热加载，刷新即生效）
 *
 * 设计原则：
 * 1. esbuild 打包 — 正确解析 ES module import/export，无需手写拓扑序
 * 2. 模块化源码 — src/*.js 是开发真相源，支持 Vitest 单元测试
 * 3. 生产兼容 — 打包后 IIFE 格式，无 CORS 限制，兼容 file:// 和 Tauri WebView
 * 4. 幂等可逆 — 两种模式可反复切换，不丢失 HTML 结构
 */

import { readFileSync, writeFileSync, existsSync, copyFileSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import esbuild from 'esbuild';

const __dirname = dirname(fileURLToPath(import.meta.url));
const uiDir = resolve(__dirname, '../ui');
const srcDir = join(uiDir, 'src');
const stylesDir = join(uiDir, 'styles');
const htmlPath = join(uiDir, 'index.html');

const isDevMode = process.argv.includes('--dev');

// CSS 文件列表（按加载顺序）
const CSS_FILES = ['tokens.css', 'typography.css', 'components.css', 'graph-viewer.css', 'print.css'];

// ============================================================
// 标记注释常量（用于在 HTML 中定位 CSS/JS 区域）
// ============================================================
const CSS_MARKER = '<!-- ============ EchoMind 样式';
const JS_MARKER = '<!-- ============ 入口模块';

// ============================================================
// JS 模块处理（esbuild）
// ============================================================

/**
 * 使用 esbuild 从 src/main.js 打包为 IIFE 格式的单文件脚本。
 *
 * esbuild 自动解析所有 import/export，无需手写 MODULE_ORDER。
 * 输出为 IIFE 格式，所有代码包裹在闭包中，通过 window.* 暴露全局函数。
 *
 * @returns {Promise<string>} 打包后的 JavaScript 代码
 */
async function buildInlineScript() {
  const result = await esbuild.build({
    entryPoints: [join(srcDir, 'main.js')],
    bundle: true,
    format: 'iife',
    target: 'es2022',
    write: false,
    minify: false,
    sourcemap: false,
    logLevel: 'warning',
    // vendor 库（marked, DOMPurify, mermaid, hljs, katex, Chart）通过
    // <script> 标签全局加载，不参与打包
    external: [],
  });

  const code = result.outputFiles[0].text;

  // 添加头部注释
  return `// ============================================================
// EchoMind 前端打包文件 — 由 scripts/build-ui.mjs (esbuild) 自动生成
// 源码位于 ui/src/*.js（模块化开发，支持 Vitest 单元测试）
// 此文件由 esbuild 打包生成，请勿手动编辑
// ============================================================
${code}`;
}

/**
 * 生成生产模式的内联 <script> 块。
 * @param {string} inlineScript - 打包后的 JavaScript 代码
 * @returns {string} 完整的 HTML <script> 块（含标记注释）
 */
function buildProdScriptBlock(inlineScript) {
  const indented = inlineScript.split('\n').map(line => '      ' + line).join('\n');
  return `    ${JS_MARKER}（esbuild 打包内联 / FIGMA_DESIGN_SPEC 架构重构） ============ -->
    <script>
${indented}
    </script>`;
}

/**
 * 生成开发模式的 <script type="module"> 块。
 * @returns {string} 完整的 HTML <script> 块（含标记注释）
 */
function buildDevScriptBlock() {
  return `    ${JS_MARKER}（开发模式：ES Module 原生加载 / 生产模式：esbuild 打包内联） ============ -->
    <script type="module" src="src/main.js"></script>`;
}

// ============================================================
// CSS 处理
// ============================================================

/**
 * 生成生产模式的内联 <style> 块（读取所有 CSS 文件内容内联）。
 * @returns {string} 完整的 HTML CSS 块（含标记注释）
 */
function buildProdCssBlock() {
  const styles = CSS_FILES.map(filename => {
    const filePath = join(stylesDir, filename);
    if (!existsSync(filePath)) {
      console.warn(`⚠ CSS 文件不存在: ${filename}，跳过`);
      return '';
    }
    const css = readFileSync(filePath, 'utf-8').trim();
    return `    <style data-src="${filename}">
${css.split('\n').map(line => '      ' + line).join('\n')}
    </style>`;
  }).filter(Boolean).join('\n');

  return `    ${CSS_MARKER}（模块化拆分 / FIGMA_DESIGN_SPEC §2-§10） ============ -->
${styles}`;
}

/**
 * 生成开发模式的外部 <link> 块。
 * @returns {string} 完整的 HTML CSS 块（含标记注释）
 */
function buildDevCssBlock() {
  const links = CSS_FILES.map(filename =>
    `    <link rel="stylesheet" href="styles/${filename}" />`
  ).join('\n');
  return `    ${CSS_MARKER}（模块化拆分 / FIGMA_DESIGN_SPEC §2-§10） ============ -->
${links}`;
}

// ============================================================
// HTML 注入/恢复
// ============================================================

/**
 * 替换 HTML 中指定标记区域的块内容。
 * @param {string} html - 原始 HTML
 * @param {string} marker - 标记注释的前缀
 * @param {string} newBlock - 替换后的完整块
 * @param {string} blockType - 块类型 'css' 或 'js'
 * @returns {string} 替换后的 HTML
 */
function replaceBlock(html, marker, newBlock, blockType) {
  const markerEscaped = marker.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

  if (blockType === 'css') {
    const regex = new RegExp(
      '[ \\t]*' + markerEscaped + '[\\s\\S]*?(?=\\n[ \\t]*</head>)'
    );
    if (regex.test(html)) {
      return html.replace(regex, newBlock);
    }
  } else {
    const regex = new RegExp(
      '[ \\t]*' + markerEscaped + '[\\s\\S]*?(?=\\n[ \\t]*</body>)'
    );
    if (regex.test(html)) {
      return html.replace(regex, newBlock);
    }
  }

  console.warn(`⚠ 未找到标记区域: ${marker}`);
  return html;
}

// ============================================================
// 主流程
// ============================================================

async function buildProduction() {
  console.log('🔨 生产模式构建：esbuild 打包 JS + 内联 CSS → 单文件 ui/index.html');

  let html = readFileSync(htmlPath, 'utf-8');

  // 1. 内联 CSS
  const prodCss = buildProdCssBlock();
  html = replaceBlock(html, CSS_MARKER, prodCss, 'css');

  // 2. esbuild 打包 JS 并内联
  const inlineScript = await buildInlineScript();
  const prodJs = buildProdScriptBlock(inlineScript);
  html = replaceBlock(html, JS_MARKER, prodJs, 'js');

  writeFileSync(htmlPath, html, 'utf-8');
  console.log(`✓ 已更新 ${htmlPath}`);
  console.log(`✅ 生产构建完成，esbuild 打包脚本 ${inlineScript.split('\n').length} 行`);
}

function buildDev() {
  console.log('🔧 开发模式构建：外部引用 CSS + JS → 多文件 ui/index.html');

  let html = readFileSync(htmlPath, 'utf-8');

  const devCss = buildDevCssBlock();
  html = replaceBlock(html, CSS_MARKER, devCss, 'css');

  const devJs = buildDevScriptBlock();
  html = replaceBlock(html, JS_MARKER, devJs, 'js');

  writeFileSync(htmlPath, html, 'utf-8');
  console.log(`✓ 已更新 ${htmlPath}`);
  console.log('✅ 开发模式构建完成，JS/CSS 通过外部引用加载');
  console.log('   提示：编辑 ui/src/*.js 后刷新浏览器即可生效，无需重新构建');
}

// ============================================================
// 同步到 GitHub 发布仓库（EchoMind/ui/index.html）
// ============================================================

/**
 * 构建完成后自动同步 ui/index.html 到 EchoMind/ui/index.html，
 * 确保发布仓库副本与开发目录始终一致（铁律十一）。
 */
function syncToReleaseRepo() {
  const releaseHtmlPath = resolve(__dirname, '../EchoMind/ui/index.html');
  if (existsSync(resolve(__dirname, '../EchoMind/.git'))) {
    copyFileSync(htmlPath, releaseHtmlPath);
    console.log(`✓ 已同步到发布仓库: ${releaseHtmlPath}`);
  }
}

// 执行
if (isDevMode) {
  buildDev();
} else {
  await buildProduction();
}
syncToReleaseRepo();
