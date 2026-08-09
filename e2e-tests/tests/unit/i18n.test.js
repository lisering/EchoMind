/**
 * EchoMind 国际化模块单元测试 — i18n.js。
 *
 * 验证点：
 * 1. 常量正确性（SUPPORTED_LOCALES / DEFAULT_LOCALE / FALLBACK_LOCALE）
 * 2. detectLocale 系统语言检测
 * 3. getLocale 初始值
 * 4. setLocale + t 翻译 + 回退
 * 5. t() 占位符插值
 * 6. refreshI18nElements DOM 属性刷新
 * 7. setLocale 不支持的语言回退到默认
 */

import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import {
  SUPPORTED_LOCALES,
  DEFAULT_LOCALE,
  FALLBACK_LOCALE,
  t,
  getLocale,
  detectLocale,
  setLocale,
  refreshI18nElements,
} from '../../../ui/src/i18n.js';

// ============================================================
// 测试用语言包（模拟 fetch 返回的 JSON）
// ============================================================

const MOCK_EN = {
  app: { title: 'EchoMind', subtitle: 'Your Knowledge Base' },
  sidebar: { new_chat: 'New Chat', import: 'Import' },
  import: { deleted: 'Deleted {name}', complete: 'Imported {count} files' },
  greeting: 'Hello, {user}!',
  nested: { deep: { key: 'deep value' } },
};

const MOCK_ZH = {
  app: { title: '灵犀', subtitle: '你的知识库' },
  sidebar: { new_chat: '新建会话' },
  import: { deleted: '已删除 {name}' },
  greeting: '你好，{user}！',
};

// ============================================================
// 辅助：mock fetch 返回语言包 JSON
// ============================================================

function mockFetchForLocale(locale) {
  const data = locale === 'en' ? MOCK_EN : locale === 'zh-CN' ? MOCK_ZH : {};
  globalThis.fetch = vi.fn(async (url) => {
    if (url.includes(`locales/${locale}.json`)) {
      return { ok: true, json: async () => data };
    }
    return { ok: false, json: async () => ({}) };
  });
}

describe('I18n — i18n.js', () => {

  describe('常量', () => {
    it('SUPPORTED_LOCALES 包含 en 和 zh-CN', () => {
      expect(SUPPORTED_LOCALES).toContain('en');
      expect(SUPPORTED_LOCALES).toContain('zh-CN');
    });

    it('DEFAULT_LOCALE 为 en', () => {
      expect(DEFAULT_LOCALE).toBe('en');
    });

    it('FALLBACK_LOCALE 为 en', () => {
      expect(FALLBACK_LOCALE).toBe('en');
    });
  });

  describe('detectLocale', () => {
    it('中文系统返回 zh-CN', () => {
      vi.stubGlobal('navigator', { language: 'zh-CN' });
      expect(detectLocale()).toBe('zh-CN');
    });

    it('zh-TW 也返回 zh-CN', () => {
      vi.stubGlobal('navigator', { language: 'zh-TW' });
      expect(detectLocale()).toBe('zh-CN');
    });

    it('英文系统返回 en', () => {
      vi.stubGlobal('navigator', { language: 'en-US' });
      expect(detectLocale()).toBe('en');
    });

    it('其他语言返回默认 en', () => {
      // ja 已加入 SUPPORTED_LOCALES，使用真正不支持的语言测试
      vi.stubGlobal('navigator', { language: 'fr-FR' });
      expect(detectLocale()).toBe('en');
    });

    it('日语系统返回 ja', () => {
      vi.stubGlobal('navigator', { language: 'ja-JP' });
      expect(detectLocale()).toBe('ja');
    });

    it('无 navigator.language 时返回默认 en', () => {
      vi.stubGlobal('navigator', {});
      expect(detectLocale()).toBe('en');
    });
  });

  describe('getLocale 初始值', () => {
    it('初始为 en', () => {
      expect(getLocale()).toBe('en');
    });
  });

  describe('t() — 无语言包加载时', () => {
    it('缺失 key 时返回 key 本身', () => {
      expect(t('nonexistent.key')).toBe('nonexistent.key');
    });
  });

  describe('setLocale + t — 英文语言包', () => {
    beforeEach(async () => {
      mockFetchForLocale('en');
      await setLocale('en', false);
    });

    it('简单 key 翻译', () => {
      expect(t('app.title')).toBe('EchoMind');
    });

    it('嵌套 key 翻译', () => {
      expect(t('sidebar.new_chat')).toBe('New Chat');
    });

    it('深层嵌套 key', () => {
      expect(t('nested.deep.key')).toBe('deep value');
    });

    it('占位符插值 — 单参数', () => {
      expect(t('import.deleted', { name: 'test.md' })).toBe('Deleted test.md');
    });

    it('占位符插值 — 多参数', () => {
      expect(t('import.complete', { count: 3 })).toBe('Imported 3 files');
    });

    it('占位符插值 — 含标点符号', () => {
      expect(t('greeting', { user: 'John' })).toBe('Hello, John!');
    });

    it('占位符未提供参数时保留 {placeholder} 原文', () => {
      expect(t('import.deleted')).toBe('Deleted {name}');
    });

    it('无参数调用 t() 不报错', () => {
      expect(t('app.subtitle')).toBe('Your Knowledge Base');
    });
  });

  describe('setLocale + t — 中文语言包', () => {
    beforeEach(async () => {
      mockFetchForLocale('zh-CN');
      await setLocale('zh-CN', false);
    });

    it('中文翻译', () => {
      expect(t('app.title')).toBe('灵犀');
    });

    it('中文占位符插值', () => {
      expect(t('import.deleted', { name: '文档.md' })).toBe('已删除 文档.md');
    });

    it('中文 key 不在中文包中但存在于英文 fallback', () => {
      // sidebar.import 在中文包中不存在，应回退到英文
      expect(t('sidebar.import')).toBe('Import');
    });

    it('中文缺失且英文也缺失的 key 返回 key 本身', () => {
      expect(t('totally.missing.key')).toBe('totally.missing.key');
    });
  });

  describe('setLocale — 错误处理', () => {
    it('不支持的语言回退到 en', async () => {
      mockFetchForLocale('en');
      await setLocale('fr', false);
      expect(getLocale()).toBe('en');
    });

    it('持久化失败不抛异常', async () => {
      // window.__TAURI__ 不存在，invoke 应静默失败
      mockFetchForLocale('en');
      await expect(setLocale('en', true)).resolves.not.toThrow();
    });
  });

  describe('refreshI18nElements', () => {
    beforeEach(async () => {
      mockFetchForLocale('en');
      await setLocale('en', false);
    });

    it('data-i18n 元素 textContent 被刷新', () => {
      document.body.innerHTML = '<div id="el" data-i18n="app.title"></div>';
      refreshI18nElements();
      expect(document.getElementById('el').textContent).toBe('EchoMind');
    });

    it('data-i18n-placeholder 元素 placeholder 被刷新', () => {
      document.body.innerHTML = '<input id="inp" data-i18n-placeholder="sidebar.import" />';
      refreshI18nElements();
      expect(document.getElementById('inp').placeholder).toBe('Import');
    });

    it('data-i18n-title 元素 title 被刷新', () => {
      document.body.innerHTML = '<div id="tip" data-i18n-title="app.title"></div>';
      refreshI18nElements();
      expect(document.getElementById('tip').title).toBe('EchoMind');
    });

    it('data-i18n-aria-label 元素 aria-label 被刷新', () => {
      document.body.innerHTML = '<div id="btn" data-i18n-aria-label="sidebar.new_chat"></div>';
      refreshI18nElements();
      expect(document.getElementById('btn').getAttribute('aria-label')).toBe('New Chat');
    });

    it('data-i18n-html 元素 innerHTML 被刷新', () => {
      document.body.innerHTML = '<div id="html-el" data-i18n-html="app.title"></div>';
      refreshI18nElements();
      expect(document.getElementById('html-el').innerHTML).toBe('EchoMind');
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });
});
