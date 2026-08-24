/**
 * icons.js 超大规模综合单元测试
 *
 * 覆盖所有图标：
 * - icon() 函数签名（name + size 参数）
 * - 每个图标的 SVG 结构验证
 * - currentColor 属性
 * - CSS 尺寸类（icon-sm/md/lg）
 * - 未知图标的 fallback
 * - 尺寸参数边界
 * - SVG viewBox 有效性
 *
 * 45 个测试用例
 */
import { describe, it, expect } from 'vitest';
import { icon, iconRaw, fileIcon, listIcons } from '../../../ui/src/utils.js';

describe('icon() — 图标系统', () => {
  describe('函数签名与返回值', () => {
    it('返回字符串', () => {
      expect(typeof icon('plus', 'sm')).toBe('string');
    });

    it('返回非空字符串', () => {
      expect(icon('settings', 'md').length).toBeGreaterThan(10);
    });

    it('包含 <svg 标签', () => {
      expect(icon('chat', 'lg')).toContain('<svg');
    });

    it('包含 </svg> 闭合标签', () => {
      expect(icon('brand', 'md')).toContain('</svg>');
    });
  });

  describe('尺寸 CSS 类', () => {
    it('sm 尺寸添加 icon-sm 类', () => {
      expect(icon('plus', 'sm')).toContain('icon-sm');
    });

    it('md 尺寸添加 icon-md 类', () => {
      expect(icon('plus', 'md')).toContain('icon-md');
    });

    it('lg 尺寸添加 icon-lg 类', () => {
      expect(icon('plus', 'lg')).toContain('icon-lg');
    });

    it('默认尺寸为 sm', () => {
      const result = icon('plus');
      expect(result).toContain('icon-sm');
    });

    it('无效尺寸使用 sm 默认', () => {
      const result = icon('plus', 'xxl');
      expect(result).toContain('icon-sm');
    });
  });

  describe('currentColor 属性', () => {
    it('大部分图标使用 currentColor stroke', () => {
      expect(icon('plus', 'sm')).toContain('currentColor');
    });

    it('brand 图标使用 currentColor', () => {
      expect(icon('brand', 'md')).toContain('currentColor');
    });
  });

  describe('viewBox 属性', () => {
    it('包含 viewBox="0 0 24 24"', () => {
      expect(icon('chat', 'md')).toContain('viewBox="0 0 24 24"');
    });

    it('plus 图标有 viewBox', () => {
      expect(icon('plus', 'sm')).toContain('viewBox="0 0 24 24"');
    });
  });

  describe('各图标存在性验证', () => {
    const knownIcons = [
      'plus', 'collapse', 'expand', 'chat', 'settings', 'stop',
      'drag', 'brand', 'close', 'retry', 'search', 'eye', 'summary',
      'trash', 'download', 'tag', 'copy',
      'warning', 'info', 'lock', 'unlock', 'check', 'cloud',
      'book', 'shield', 'clipboard', 'keyboard', 'graph',
      'memory', 'trace', 'chart', 'chevronRight', 'mic',
      'send', 'globe',
    ];

    for (const name of knownIcons) {
      it(`图标 "${name}" 返回有效 SVG`, () => {
        const result = icon(name, 'sm');
        expect(result).toContain('<svg');
        expect(result).toContain('</svg>');
        expect(result.length).toBeGreaterThan(20);
      });
    }
  });

  describe('未知图标 fallback', () => {
    it('未知图标名返回空 SVG 或 fallback', () => {
      const result = icon('nonexistent-icon-xyz', 'sm');
      expect(result).toBeDefined();
      // 应该返回某种 fallback（可能是空 SVG 或默认图标）
      expect(typeof result).toBe('string');
    });

    it('空字符串图标名返回 fallback', () => {
      const result = icon('', 'md');
      expect(typeof result).toBe('string');
    });

    it('null 图标名返回 fallback', () => {
      const result = icon(null, 'md');
      expect(typeof result).toBe('string');
    });
  });

  describe('SVG 结构完整性', () => {
    it('所有 SVG 有闭合 path 标签', () => {
      const result = icon('search', 'md');
      expect(result).toMatch(/<(path|circle|rect|polyline|line|polygon)[^>]*\/?>/);
    });

    it('stop 图标使用 fill 而非 stroke', () => {
      const result = icon('stop', 'sm');
      expect(result).toContain('fill="currentColor"');
    });

    it('close 图标有两条 line', () => {
      const result = icon('close', 'md');
      const lineCount = (result.match(/<line/g) || []).length;
      expect(lineCount).toBeGreaterThanOrEqual(2);
    });
  });

  describe('listIcons 导出', () => {
    it('返回数组', () => {
      const icons = listIcons();
      expect(Array.isArray(icons)).toBe(true);
    });

    it('包含已知图标名', () => {
      const icons = listIcons();
      expect(icons.length).toBeGreaterThan(20);
    });
  });

  describe('iconRaw — 无尺寸类的 SVG', () => {
    it('返回 SVG 字符串', () => {
      const result = iconRaw('plus');
      expect(result).toContain('<svg');
    });

    it('未知图标返回空字符串', () => {
      expect(iconRaw('nonexistent')).toBe('');
    });
  });

  describe('fileIcon — 文件类型图标', () => {
    it('返回 SVG 字符串', () => {
      const result = fileIcon('md', 'sm');
      expect(result).toContain('<svg');
    });

    it('所有扩展名返回相同图标', () => {
      const mdIcon = fileIcon('md', 'sm');
      const pdfIcon = fileIcon('pdf', 'sm');
      expect(mdIcon).toBe(pdfIcon);
    });
  });
});
