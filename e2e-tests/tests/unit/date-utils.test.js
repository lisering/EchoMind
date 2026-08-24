/**
 * date-utils.js 单元测试
 *
 * 覆盖 formatDate / formatRelativeTime / formatFileSize / formatNumber / formatPercent
 * 的正常场景、边界场景和错误处理。
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock i18n.js — t() 函数返回模板字符串
vi.mock('../../../ui/src/i18n.js', () => ({
  t: vi.fn((key, params) => {
    const map = {
      'date.just_now': '刚刚',
      'date.minutes_ago': `${params?.n ?? 0} 分钟前`,
      'date.hours_ago': `${params?.n ?? 0} 小时前`,
      'date.days_ago': `${params?.n ?? 0} 天前`,
    };
    return map[key] ?? key;
  }),
}));

// Mock window globals
vi.stubGlobal('window', {
  __formatDate: undefined,
  __formatRelativeTime: undefined,
  __formatFileSize: undefined,
  __formatNumber: undefined,
});

const { formatDate, formatRelativeTime, formatFileSize, formatNumber, formatPercent } =
  await import('../../../ui/src/utils.js');

describe('date-utils', () => {
  describe('formatDate', () => {
    it('格式化毫秒级时间戳', () => {
      const ts = new Date(2026, 0, 15, 14, 30).getTime();
      const result = formatDate(ts);
      expect(result).toBe('2026-01-15 14:30');
    });

    it('格式化秒级时间戳（10位数）', () => {
      const date = new Date(2026, 5, 10, 9, 5);
      const tsSec = Math.floor(date.getTime() / 1000);
      const result = formatDate(tsSec);
      expect(result).toBe('2026-06-10 09:05');
    });

    it('格式化 Date 对象', () => {
      const date = new Date(2026, 11, 31, 23, 59);
      const result = formatDate(date);
      expect(result).toBe('2026-12-31 23:59');
    });

    it('格式化 ISO 字符串', () => {
      const result = formatDate('2026-03-15T12:00:00');
      expect(result).toBe('2026-03-15 12:00');
    });

    it('空值返回空字符串', () => {
      expect(formatDate(null)).toBe('');
      expect(formatDate(undefined)).toBe('');
      expect(formatDate(0)).toBe('');
      expect(formatDate('')).toBe('');
    });

    it('无效时间戳返回空字符串', () => {
      expect(formatDate('invalid-date')).toBe('');
      expect(formatDate(NaN)).toBe('');
    });

    it('补零：单位数月/日/时/分补零', () => {
      const date = new Date(2026, 0, 1, 1, 2);
      expect(formatDate(date)).toBe('2026-01-01 01:02');
    });
  });

  describe('formatRelativeTime', () => {
    beforeEach(() => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date(2026, 7, 17, 12, 0));
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('3分钟内返回"刚刚"', () => {
      const ts = Date.now() - 60 * 1000;
      expect(formatRelativeTime(ts)).toBe('刚刚');
    });

    it('1小时内返回"N分钟前"', () => {
      const ts = Date.now() - 10 * 60 * 1000;
      expect(formatRelativeTime(ts)).toBe('10 分钟前');
    });

    it('24小时内返回"N小时前"', () => {
      const ts = Date.now() - 3 * 60 * 60 * 1000;
      expect(formatRelativeTime(ts)).toBe('3 小时前');
    });

    it('7天内返回"N天前"', () => {
      const ts = Date.now() - 3 * 24 * 60 * 60 * 1000;
      expect(formatRelativeTime(ts)).toBe('3 天前');
    });

    it('超过7天返回绝对日期', () => {
      const ts = Date.now() - 10 * 24 * 60 * 60 * 1000;
      expect(formatRelativeTime(ts)).toBe('2026-08-07');
    });

    it('未来时间降级为绝对格式', () => {
      const ts = Date.now() + 60 * 1000;
      expect(formatRelativeTime(ts)).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
    });

    it('空值返回空字符串', () => {
      expect(formatRelativeTime(null)).toBe('');
      expect(formatRelativeTime(undefined)).toBe('');
    });
  });

  describe('formatFileSize', () => {
    it('小于 1024B 直接显示 B', () => {
      expect(formatFileSize(0)).toBe('0 B');
      expect(formatFileSize(512)).toBe('512 B');
      expect(formatFileSize(1023)).toBe('1023 B');
    });

    it('KB 级别', () => {
      expect(formatFileSize(1024)).toBe('1.0 KB');
      expect(formatFileSize(1536)).toBe('1.5 KB');
      expect(formatFileSize(1048575)).toBe('1024.0 KB');
    });

    it('MB 级别', () => {
      expect(formatFileSize(1048576)).toBe('1.0 MB');
      expect(formatFileSize(12 * 1048576)).toBe('12.0 MB');
    });

    it('GB 级别', () => {
      expect(formatFileSize(1073741824)).toBe('1.0 GB');
    });

    it('TB 级别', () => {
      expect(formatFileSize(1099511627776)).toBe('1.0 TB');
    });

    it('null / undefined / 负数返回空字符串', () => {
      expect(formatFileSize(null)).toBe('');
      expect(formatFileSize(undefined)).toBe('');
      expect(formatFileSize(-1)).toBe('');
    });
  });

  describe('formatNumber', () => {
    it('千分位分隔', () => {
      expect(formatNumber(1000)).toBe('1,000');
      expect(formatNumber(1000000)).toBe('1,000,000');
    });

    it('小数不变', () => {
      expect(formatNumber(1234.56)).toBe('1,234.56');
    });

    it('null / undefined 返回空字符串', () => {
      expect(formatNumber(null)).toBe('');
      expect(formatNumber(undefined)).toBe('');
    });

    it('非有限数字返回字符串', () => {
      expect(formatNumber(Infinity)).toBe('Infinity');
      expect(formatNumber(NaN)).toBe('NaN');
    });

    it('小数字不需要千分位', () => {
      expect(formatNumber(42)).toBe('42');
    });
  });

  describe('formatPercent', () => {
    it('0.87 → 87%', () => {
      expect(formatPercent(0.87)).toBe('87%');
    });

    it('0.5 → 50%', () => {
      expect(formatPercent(0.5)).toBe('50%');
    });

    it('1.0 → 100%', () => {
      expect(formatPercent(1.0)).toBe('100%');
    });

    it('0.0 → 0%', () => {
      expect(formatPercent(0.0)).toBe('0%');
    });

    it('四舍五入', () => {
      expect(formatPercent(0.875)).toBe('88%');
      expect(formatPercent(0.334)).toBe('33%');
    });

    it('null / undefined 返回空字符串', () => {
      expect(formatPercent(null)).toBe('');
      expect(formatPercent(undefined)).toBe('');
    });

    it('非数字返回空字符串', () => {
      expect(formatPercent('abc')).toBe('');
    });
  });
});
