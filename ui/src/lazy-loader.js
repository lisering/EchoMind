/**
 * EchoMind 延迟加载器 — 按需加载 vendor 库（构建体积优化）。
 *
 * 优化原理（REQ-NFR-019）：
 * mermaid.min.js (3.4MB) / d3.min.js (273KB) / chart.umd.min.js (204KB) / katex (~280KB)
 * 仅在特定场景使用，无需启动时全量加载。延迟到首次使用时加载，显著减少首屏白屏时间。
 *
 * 设计要点：
 * 1. Promise 缓存 — 同一库只加载一次，重复调用返回缓存 Promise
 * 2. CSS + JS 分离 — KaTeX 需同时加载 JS + CSS + mhchem 插件
 * 3. 优雅降级 — 加载失败时静默跳过对应功能（如 mermaid 图表显示源码）
 * 4. 初始化回调 — mermaid 加载后需 initialize() 配置主题
 */

/** 已加载脚本 Promise 缓存 */
const _loaded = new Map();

/** Mermaid 初始化配置回调 */
let _mermaidInitFn = null;

/**
 * 设置 mermaid 初始化回调（在 main.js boot() 中调用）。
 * mermaid 加载后自动调用此回调执行 initialize()。
 * @param {() => void} fn - 初始化函数
 */
export function setMermaidInitFn(fn) {
  _mermaidInitFn = fn;
}

/**
 * 动态加载 script 标签并缓存 Promise。
 * @param {string} name - 缓存键名
 * @param {string} src - script src 路径
 * @returns {Promise<void>}
 */
function loadScript(name, src) {
  if (_loaded.has(name)) return _loaded.get(name);
  const promise = new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = src;
    script.async = true;
    script.onload = () => resolve();
    script.onerror = () => {
      _loaded.delete(name); // 允许重试
      reject(new Error(`Failed to load ${src}`));
    };
    document.head.appendChild(script);
  });
  _loaded.set(name, promise);
  return promise;
}

/**
 * 动态加载 CSS link 标签。
 * @param {string} name - 缓存键名
 * @param {string} href - CSS href 路径
 * @returns {Promise<void>}
 */
function loadStyle(name, href) {
  if (_loaded.has(name)) return _loaded.get(name);
  const promise = new Promise((resolve, reject) => {
    const link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = href;
    link.onload = () => resolve();
    link.onerror = () => {
      _loaded.delete(name);
      reject(new Error(`Failed to load ${href}`));
    };
    document.head.appendChild(link);
  });
  _loaded.set(name, promise);
  return promise;
}

/**
 * 延迟加载 mermaid（esbuild 自定义打包，~3.4MB），加载后执行初始化回调。
 *
 * 使用 vendor/mermaid-custom.min.js（esbuild 从 npm mermaid v11 ESM 打包，
 * tree-shaking + 压缩优化，比官方 UMD 预构建小 ~3%）。
 * 构建命令：node scripts/build-mermaid.mjs
 *
 * @returns {Promise<typeof mermaid | null>} mermaid 全局对象，加载失败返回 null
 */
export async function loadMermaid() {
  try {
    await loadScript('mermaid', 'vendor/mermaid-custom.min.js');
    if (_mermaidInitFn) _mermaidInitFn();
    return typeof mermaid !== 'undefined' ? mermaid : null;
  } catch {
    return null;
  }
}

/**
 * 延迟加载 d3.min.js（273KB），仅知识图谱面板使用。
 * @returns {Promise<typeof d3 | null>} d3 全局对象，加载失败返回 null
 */
export async function loadD3() {
  try {
  await loadScript('d3', 'vendor/d3.min.js');
  return typeof d3 !== 'undefined' ? d3 : null;
  } catch {
    return null;
  }
}

/**
 * 延迟加载 chart.umd.min.js（204KB），仅表格图表切换时使用。
 * @returns {Promise<typeof Chart | null>} Chart 全局对象，加载失败返回 null
 */
export async function loadChart() {
  try {
  await loadScript('chart', 'vendor/chart.umd.min.js');
  return typeof Chart !== 'undefined' ? Chart : null;
  } catch {
    return null;
  }
}

/**
 * 延迟加载 highlight.min.js（124KB），仅代码块语法高亮时使用。
 *
 * 替代 index.html 中的 eager <script> 标签，将 124KB 从启动加载移至按需加载。
 * 首次调用时加载（非流式渲染或 renderRichContent），后续从缓存返回。
 *
 * @returns {Promise<typeof hljs | null>} hljs 全局对象，加载失败返回 null
 */
export async function loadHighlight() {
  try {
    await loadScript('hljs', 'vendor/highlight.min.js');
    return typeof hljs !== 'undefined' ? hljs : null;
  } catch {
    return null;
  }
}

/**
 * 延迟加载 KaTeX（JS + CSS + mhchem 插件，~280KB），仅数学公式渲染时使用。
 * @returns {Promise<typeof katex | null>} katex 全局对象，加载失败返回 null
 */
export async function loadKatex() {
  try {
    await Promise.all([
      loadStyle('katex-css', 'vendor/katex/katex.min.css'),
      loadScript('katex', 'vendor/katex/katex.min.js'),
    ]);
  await loadScript('katex-mhchem', 'vendor/katex/katex-mhchem.min.js');
  return typeof katex !== 'undefined' ? katex : null;
  } catch {
    return null;
  }
}
