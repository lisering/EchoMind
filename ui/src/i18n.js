/**
 * EchoMind 国际化模块 — 轻量级 i18n 引擎。
 *
 * 设计原则：
 * 1. 零依赖 — 纯 vanilla JS，不引入外部 i18n 库
 * 2. JSON 键值对 — 语言包存储在 ui/locales/{locale}.json
 * 3. 嵌套 key — 用点号分隔（如 t('sidebar.new_chat')）
 * 4. 占位符插值 — {name} 语法（如 t('import.deleted', { name: 'test.md' })）
 * 5. 回退 — 缺失 key 时回退到 en，再回退到 key 本身（不崩溃）
 * 6. 持久化 — 用户语言偏好存储在后端 settings 表（通过 IPC）
 * 7. 自动检测 — 首次启动检测系统 locale（中文→zh-CN，其他→en）
 *
 * 扩展指南：
 * - 添加新语言：在 ui/locales/ 下新增 {locale}.json 文件
 * - 添加新文案：在所有 locale JSON 中添加对应 key
 * - 切换语言：调用 setLocale(newLocale)，会自动刷新页面所有 data-i18n 元素
 */

// ============================================================
// 常量
// ============================================================

/** 支持的语言列表 */
export const SUPPORTED_LOCALES = ['en', 'zh-CN', 'ja'];

/** 默认语言（当无法检测系统语言时） */
export const DEFAULT_LOCALE = 'en';

/** 回退语言（当当前语言包缺失 key 时） */
export const FALLBACK_LOCALE = 'en';

/** localStorage 键名（用于在 IPC 可用前的临时存储） */
const STORAGE_KEY = 'echomind_locale';

// ============================================================
// 模块级状态
// ============================================================

/** 当前语言 */
let _locale = DEFAULT_LOCALE;

/** 已加载的语言包缓存（locale → flattened key map） */
const _cache = new Map();

// ============================================================
// 内部函数
// ============================================================

/**
 * 将嵌套 JSON 对象展平为点号分隔的键值对。
 * @param {Object} obj - 嵌套对象
 * @param {string} [prefix=''] - 键前缀
 * @returns {Object} 展平后的 { 'a.b.c': 'value' } 对象
 */
function flatten(obj, prefix = '') {
  const result = {};
  for (const key of Object.keys(obj)) {
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (typeof obj[key] === 'object' && obj[key] !== null && !Array.isArray(obj[key])) {
      Object.assign(result, flatten(obj[key], fullKey));
    } else {
      result[fullKey] = String(obj[key]);
    }
  }
  return result;
}

/**
 * 异步加载语言包 JSON 文件。
 * 在 Tauri 环境中通过 fetch 加载 ui/locales/{locale}.json。
 * 在测试环境中可注入 mock。
 * @param {string} locale - 语言代码
 * @returns {Promise<Object>} 展平后的键值对
 */
async function loadLocaleData(locale) {
  if (_cache.has(locale)) return _cache.get(locale);

  // file:// 协议下无 Tauri 运行时（如浏览器直接打开、无 stub 的测试环境）时
  // fetch 必然失败并在控制台产生错误，直接跳过，回退默认文案
  if (location.protocol === 'file:' && !window.__TAURI__) {
    _cache.set(locale, {});
    return {};
  }

  try {
    const resp = await fetch(`locales/${locale}.json`);
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const data = await resp.json();
    const flattened = flatten(data);
    _cache.set(locale, flattened);
    return flattened;
  } catch (err) {
    console.warn(`[i18n] Failed to load locale "${locale}": ${err.message}`);
    // 回退到空对象（后续 t() 会回退到 FALLBACK_LOCALE 或 key 本身）
    _cache.set(locale, {});
    return {};
  }
}

/**
 * 替换字符串中的 {placeholder} 占位符。
 * @param {string} str - 含占位符的字符串
 * @param {Object} [params] - 占位符键值对
 * @returns {string} 替换后的字符串
 */
function interpolate(str, params) {
  if (!params) return str;
  return str.replace(/\{(\w+)\}/g, (match, key) => {
    return params[key] !== undefined ? String(params[key]) : match;
  });
}

// ============================================================
// 公共 API
// ============================================================

/**
 * 翻译函数：根据 key 获取当前语言的对应文案。
 * @param {string} key - 点号分隔的键（如 'sidebar.new_chat'）
 * @param {Object} [params] - 占位符参数（如 { name: 'test.md' }）
 * @returns {string} 翻译后的字符串（缺失时回退到 en，再回退到 key 本身）
 */
export function t(key, params) {
  const current = _cache.get(_locale);
  const fallback = _cache.get(FALLBACK_LOCALE);

  // 优先当前语言
  let value = current?.[key];
  // 回退到 fallback 语言
  if (value === undefined) value = fallback?.[key];
  // 最终回退到 key 本身
  if (value === undefined) return key;

  return interpolate(value, params);
}

/**
 * 获取当前语言。
 * @returns {string} 当前语言代码（如 'en' / 'zh-CN'）
 */
export function getLocale() {
  return _locale;
}

/**
 * 检测系统语言。
 * @returns {string} 检测到的语言代码
 */
export function detectLocale() {
// 1. 检查 navigator.language
const navLang = navigator.language || navigator.languages?.[0] || '';
if (navLang.toLowerCase().startsWith('zh')) return 'zh-CN';
if (navLang.toLowerCase().startsWith('ja')) return 'ja';
return DEFAULT_LOCALE;
}

/**
 * 设置当前语言并加载语言包。
 * 会自动刷新页面上所有 data-i18n 元素。
 * @param {string} locale - 语言代码
 * @param {boolean} [persist=true] - 是否持久化到后端
 */
export async function setLocale(locale, persist = true) {
  if (!SUPPORTED_LOCALES.includes(locale)) {
    console.warn(`[i18n] Unsupported locale "${locale}", falling back to ${DEFAULT_LOCALE}`);
    locale = DEFAULT_LOCALE;
  }

  _locale = locale;
  await loadLocaleData(locale);
  if (locale !== FALLBACK_LOCALE) await loadLocaleData(FALLBACK_LOCALE);

  // 临时存储（IPC 可用前的 fallback）
  try { localStorage.setItem(STORAGE_KEY, locale); } catch (_) { /* 隐私模式 */ }

  // 持久化到后端
  if (persist) {
    try {
      await window.__TAURI__?.core?.invoke('set_locale', { locale });
    } catch (_) { /* 后端尚未实现或 IPC 不可用，静默忽略 */ }
  }

  // 刷新页面上所有 data-i18n 元素
  refreshI18nElements();

  // 更新 <html lang="...">
  document.documentElement.lang = locale;
}

/**
 * 刷新页面上所有带 data-i18n 属性的元素。
 * 支持：
 * - data-i18n="key" → 设置 textContent
 * - data-i18n-placeholder="key" → 设置 placeholder
 * - data-i18n-title="key" → 设置 title
 * - data-i18n-aria-label="key" → 设置 aria-label
 */
export function refreshI18nElements() {
  // data-i18n → textContent
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const key = el.getAttribute('data-i18n');
    el.textContent = t(key);
  });
  // data-i18n-html → innerHTML
  document.querySelectorAll('[data-i18n-html]').forEach((el) => {
    const key = el.getAttribute('data-i18n-html');
    el.innerHTML = t(key);
  });
  // data-i18n-placeholder → placeholder
  document.querySelectorAll('[data-i18n-placeholder]').forEach((el) => {
    const key = el.getAttribute('data-i18n-placeholder');
    el.placeholder = t(key);
  });
  // data-i18n-title → title
  document.querySelectorAll('[data-i18n-title]').forEach((el) => {
    const key = el.getAttribute('data-i18n-title');
    el.title = t(key);
  });
  // data-i18n-aria-label → aria-label
  document.querySelectorAll('[data-i18n-aria-label]').forEach((el) => {
    const key = el.getAttribute('data-i18n-aria-label');
    el.setAttribute('aria-label', t(key));
  });
}

/**
 * 初始化 i18n 模块。
 * 优先级：后端持久化偏好 → localStorage → 系统检测。
 */
export async function initI18n() {
  // 尝试从后端读取
  let savedLocale = null;
  try {
    savedLocale = await window.__TAURI__?.core?.invoke('get_locale');
  } catch (_) { /* 后端尚未实现，静默忽略 */ }

  // 回退到 localStorage
  if (!savedLocale) {
    try { savedLocale = localStorage.getItem(STORAGE_KEY); } catch (_) { /* 隐私模式 */ }
  }

  // 回退到系统检测
  if (!savedLocale || !SUPPORTED_LOCALES.includes(/** @type {string} */ (savedLocale))) {
    savedLocale = detectLocale();
  }

  _locale = /** @type {string} */ (savedLocale);
  await loadLocaleData(_locale);
  if (_locale !== FALLBACK_LOCALE) await loadLocaleData(FALLBACK_LOCALE);

  // 刷新页面上所有 data-i18n 元素
  refreshI18nElements();

  document.documentElement.lang = _locale;
}
