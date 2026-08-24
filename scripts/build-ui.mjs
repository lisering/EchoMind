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
import { execSync } from 'node:child_process';
import esbuild from 'esbuild';

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(__dirname, '..');
const uiDir = resolve(__dirname, '../ui');
const srcDir = join(uiDir, 'src');
const stylesDir = join(uiDir, 'styles');
const htmlPath = join(uiDir, 'index.html');

const isDevMode = process.argv.includes('--dev');
const isReleaseMode = process.argv.includes('--release');

// 开发者工具模块 — Release 模式不打包（与后端 #[cfg(debug_assertions)] 对齐）
// 通过 onResolve + onLoad 将开发者工具模块替换为空模块，
// 确保 Release 构建中不包含 trace-panel / rag-eval / embed-eval 代码。
const DEV_TOOL_PLUGINS = [
  {
    name: 'exclude-trace-panel',
    setup(build) {
      build.onResolve({ filter: /\/trace-panel\.js$/ }, () => ({ path: 'trace-panel-excluded', namespace: 'dev-tool-excluded' }));
      build.onLoad({ filter: /.*/, namespace: 'dev-tool-excluded' }, () => ({ contents: '// trace-panel excluded in release build', loader: 'js' }));
    },
  },
  {
    name: 'exclude-rag-eval',
    setup(build) {
      build.onResolve({ filter: /\/rag-eval\.js$/ }, () => ({ path: 'rag-eval-excluded', namespace: 'dev-tool-excluded' }));
      build.onLoad({ filter: /.*/, namespace: 'dev-tool-excluded' }, () => ({ contents: '// rag-eval excluded in release build', loader: 'js' }));
    },
  },
  {
    name: 'exclude-embed-eval',
    setup(build) {
      build.onResolve({ filter: /\/embed-eval\.js$/ }, () => ({ path: 'embed-eval-excluded', namespace: 'dev-tool-excluded' }));
      build.onLoad({ filter: /.*/, namespace: 'dev-tool-excluded' }, () => ({ contents: '// embed-eval excluded in release build', loader: 'js' }));
    },
  },
];

// CSS 文件列表（按加载顺序）
const CSS_FILES = ['tokens.css', 'typography.css', 'components.css', 'icons.css', 'graph-viewer.css', 'print.css'];

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
  // 版本号单一来源：tauri.conf.json（构建期内联为 __APP_VERSION__ 常量）
  const tauriConf = JSON.parse(readFileSync(join(rootDir, 'crates', 'tauri-app', 'tauri.conf.json'), 'utf8'));
  const appVersion = tauriConf?.version ?? '0.0.0';

  const result = await esbuild.build({
    entryPoints: [join(srcDir, 'main.js')],
    bundle: true,
    format: 'iife',
    target: 'es2022',
    write: false,
    minify: true,
    sourcemap: false,
    logLevel: 'warning',
    define: {
      __APP_VERSION__: JSON.stringify(appVersion),
    },
    // vendor 库（marked, DOMPurify, mermaid, hljs, katex, Chart）通过
    // <script> 标签全局加载，不参与打包
    external: [],
    // Release 模式排除开发者工具模块（trace-panel / rag-eval）
    plugins: isReleaseMode ? DEV_TOOL_PLUGINS : [],
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
 *
 * 标记恢复逻辑：如果标记注释在 HTML 中不存在（被之前的错误构建消耗），
 * 则自动在正确位置（CSS→</head> 前，JS→</body> 前）注入新的标记块。
 *
 * @param {string} html - 原始 HTML
 * @param {string} marker - 标记注释的前缀
 * @param {string} newBlock - 替换后的完整块（含标记注释）
 * @param {string} blockType - 块类型 'css' 或 'js'
 * @returns {string} 替换后的 HTML
 */
function replaceBlock(html, marker, newBlock, blockType) {
  // 使用字符串索引替代正则匹配，避免 minified JS 中 </head>/<body> 字符串字面量
  // 导致正则误匹配（esbuild minify 会将 <\/body> 反转义为 </body>）。
  const markerIdx = html.indexOf(marker);
  const closeTag = blockType === 'css' ? '</head>' : '</body>';
  // CSS 用 indexOf（第一个 </head> = 真正的 HTML 标签）；
  // JS 用 lastIndexOf（最后一个 </body> = 真正的 HTML 标签，跳过 minified JS 字符串字面量中的 </body>）
  const closeIdx = blockType === 'css' ? html.indexOf(closeTag) : html.lastIndexOf(closeTag);

  if (markerIdx !== -1 && closeIdx !== -1 && markerIdx < closeIdx) {
    // 找到标记行起始位置（包含前导空白和换行符）
    let startIdx = markerIdx;
    while (startIdx > 0 && (html[startIdx - 1] === ' ' || html[startIdx - 1] === '\t')) {
      startIdx--;
    }
    if (startIdx > 0 && html[startIdx - 1] === '\n') {
      startIdx--;
    }
    return html.substring(0, startIdx) + '\n' + newBlock + '\n' + html.substring(closeIdx);
  }

  // 标记缺失：在对应闭合标签前注入
  if (closeIdx !== -1) {
    console.warn(`⚠ 未找到 ${blockType.toUpperCase()} 标记区域，将在 ${closeTag} 前自动注入`);
    return html.substring(0, closeIdx) + newBlock + '\n' + html.substring(closeIdx);
  }

  console.warn(`⚠ 未找到 ${closeTag} 标签，跳过 ${blockType.toUpperCase()} 块替换`);
  return html;
}

// ============================================================
// 主流程
// ============================================================

/**
 * 运行 Mermaid 自定义 esbuild 打包（从 npm mermaid v11 ESM 源码 tree-shake + 压缩）。
 * 替代 vendor/mermaid.min.js (3.4MB UMD 预构建)，输出 vendor/mermaid-custom.min.js。
 */
function buildMermaid() {
  console.log('📦 Mermaid 自定义打包：esbuild tree-shaking + minify...');
  try {
    execSync('node scripts/build-mermaid.mjs', { cwd: resolve(__dirname, '..'), stdio: 'pipe' });
  } catch {
    console.warn('⚠ Mermaid 打包失败（使用已有 vendor/mermaid-custom.min.js）');
  }
}

/**
 * 运行 Tailwind CSS CLI 预构建（扫描 ui/src/*.js + ui/index.html 生成静态 CSS）。
 *
 * 替代 vendor/tailwindcss.js (441KB JIT 运行时)，预构建 CSS 仅包含实际使用的工具类。
 * 配置见 tailwind.config.js，输入 ui/styles/tailwind-input.css，输出 ui/vendor/tailwind-prebuilt.css。
 */
function buildTailwind() {
  console.log('🎨 Tailwind CSS 预构建：扫描源码 → 生成静态 CSS');
  const cmd = 'npx tailwindcss -i ui/styles/tailwind-input.css -o ui/vendor/tailwind-prebuilt.css --minify';
  try {
    execSync(cmd, { cwd: resolve(__dirname, '..'), stdio: 'pipe' });
    const size = readFileSync(join(uiDir, 'vendor/tailwind-prebuilt.css'), 'utf-8').length;
    console.log(`✓ Tailwind 预构建完成: ${(size / 1024).toFixed(0)}KB`);
  } catch {
    console.warn('⚠ Tailwind 预构建失败（使用已有 vendor/tailwind-prebuilt.css）');
  }
}

async function buildProduction() {
  console.log('🔨 生产模式构建：Tailwind 预构建 + esbuild 打包 JS + 内联 CSS → 单文件 ui/index.html');

  // 0. Mermaid 自定义打包 + Tailwind CSS 预构建
  buildMermaid();
  buildTailwind();

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
