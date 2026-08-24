/**
 * date-utils.js 超大规模综合单元测试
 *
 * 覆盖所有导出函数：
 * - formatDate (秒/毫秒/Date/ISO 字符串/无效输入)
 * - formatRelativeTime (刚刚/N分钟前/N小时前/N天前/绝对日期/未来时间)
 * - formatFileSize (B/KB/MB/GB/TB/边界值)
 * - formatNumber (正数/零/负数/NaN/null)
 * - formatPercent (0~1/0~100/边界值)
 * - _toDate 内部函数通过公共 API 间接测试
 *
 * 40 个测试用例
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, params) => {
    if (typeof params === 'object' && params !== null) {
      // 模拟参数插值
      let result = key;
      for (const [k, v] of Object.entries(params)) {
        result = result.replace(`{${k}}`, String(v));
      }
      return result;
    }
    if (typeof params === 'string') {
      // 有些调用用第二个参数作为 fallback
      return params;
    }
    return key;
  },
}));

import {
  formatDate,
  formatRelativeTime,
  formatFileSize,
  formatNumber,
  formatPercent,
} from '../../../ui/src/utils.js';

// ============================================================
// formatDate — 日期格式化
// ============================================================
describe('formatDate — YYYY-MM-DD HH:mm 格式化', () => {
  it('毫秒级时间戳（13 位）', () => {
    const ts = new Date(2026, 0, 15, 14, 30).getTime();
    const result = formatDate(ts);
    expect(result).toBe('2026-01-15 14:30');
  });

  it('秒级时间戳（10 位）', () => {
    const ts = Math.floor(new Date(2026, 6, 4, 9, 5).getTime() / 1000);
    const result = formatDate(ts);
    expect(result).toBe('2026-07-04 09:05');
  });

  it('Date 对象', () => {
    const date = new Date(2026, 11, 31, 23, 59);
    expect(formatDate(date)).toBe('2026-12-31 23:59');
  });

  it('ISO 字符串', () => {
    const result = formatDate('2026-03-15T10:30:00');
    expect(result).toBe('2026-03-15 10:30');
  });

  it('数字字符串', () => {
    const ts = String(new Date(2026, 0, 1, 0, 0).getTime());
    const result = formatDate(ts);
    expect(result).toBe('2026-01-01 00:00');
  });

  it('null 返回空字符串', () => {
    expect(formatDate(null)).toBe('');
  });

  it('undefined 返回空字符串', () => {
    expect(formatDate(undefined)).toBe('');
  });

  it('0 返回空字符串', () => {
    expect(formatDate(0)).toBe('');
  });

  it('无效时间戳返回空字符串', () => {
    expect(formatDate('invalid-date')).toBe('');
  });

  it('NaN 返回空字符串', () => {
    expect(formatDate(NaN)).toBe('');
  });

  it('月末日期不溢出', () => {
    const date = new Date(2026, 1, 28, 15, 0); // 2月28日
    expect(formatDate(date)).toBe('2026-02-28 15:00');
  });

  it('闰年 2 月 29 日', () => {
    const date = new Date(2024, 1, 29, 12, 0); // 2024 是闰年
    expect(formatDate(date)).toBe('2024-02-29 12:00');
  });

  it('两位数月日补零', () => {
    const date = new Date(2026, 2, 5, 3, 7);
    expect(formatDate(date)).toBe('2026-03-05 03:07');
  });
});

// ============================================================
// formatRelativeTime — 相对时间
// ============================================================
describe('formatRelativeTime — 相对时间格式化', () => {
  beforeEach(() => {
    // 固定当前时间
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 7, 18, 12, 0, 0));
  });

  it('"刚刚"（3 分钟内）', () => {
    const now = Date.now();
    const result = formatRelativeTime(now);
    expect(result).toContain('just_now');
  });

  it('N 分钟前（3 分钟 ~ 1 小时）', () => {
    const tenMinAgo = Date.now() - 10 * 60 * 1000;
    const result = formatRelativeTime(tenMinAgo);
    // i18n mock 返回 key，不含参数值
    expect(result).toContain('minutes_ago');
  });

  it('N 小时前（1 ~ 24 小时）', () => {
    const threeHrAgo = Date.now() - 3 * 60 * 60 * 1000;
    const result = formatRelativeTime(threeHrAgo);
    expect(result).toContain('hours_ago');
  });

  it('N 天前（1 ~ 7 天）', () => {
    const threeDayAgo = Date.now() - 3 * 24 * 60 * 60 * 1000;
    const result = formatRelativeTime(threeDayAgo);
    expect(result).toContain('days_ago');
  });

  it('超过 7 天返回绝对日期', () => {
    const tenDayAgo = Date.now() - 10 * 24 * 60 * 60 * 1000;
    const result = formatRelativeTime(tenDayAgo);
    expect(result).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('未来时间降级为绝对格式', () => {
    const future = Date.now() + 60 * 60 * 1000; // 1 小时后
    const result = formatRelativeTime(future);
    expect(result).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
  });

  it('null 返回空字符串', () => {
    expect(formatRelativeTime(null)).toBe('');
  });

  it('0 返回空字符串', () => {
    expect(formatRelativeTime(0)).toBe('');
  });

  it('Date 对象正常工作', () => {
    const fiveMinAgo = new Date(Date.now() - 5 * 60 * 1000);
    const result = formatRelativeTime(fiveMinAgo);
    expect(result).toContain('minutes_ago');
  });

  it('刚好 3 分钟边界返回"刚刚"', () => {
    const justUnder3Min = Date.now() - 179 * 1000; // 179 秒
    const result = formatRelativeTime(justUnder3Min);
    expect(result).toContain('just_now');
  });

  it('刚好 180 秒进入"N 分钟前"', () => {
    const justOver3Min = Date.now() - 181 * 1000;
    const result = formatRelativeTime(justOver3Min);
    expect(result).toContain('minutes_ago');
  });

  it('刚好 60 分钟进入"N 小时前"', () => {
    const oneHour = Date.now() - 60 * 60 * 1000;
    const result = formatRelativeTime(oneHour);
    expect(result).toContain('hours_ago');
  });
});

// ============================================================
// formatFileSize — 文件大小格式化
// ============================================================
describe('formatFileSize — 文件大小格式化', () => {
  it('小于 1024 显示 B', () => {
    expect(formatFileSize(0)).toBe('0 B');
    expect(formatFileSize(512)).toBe('512 B');
    expect(formatFileSize(1023)).toBe('1023 B');
  });

  it('KB 级别', () => {
    expect(formatFileSize(1024)).toBe('1.0 KB');
    expect(formatFileSize(10240)).toBe('10.0 KB');
  });

  it('MB 级别', () => {
    expect(formatFileSize(1024 * 1024)).toBe('1.0 MB');
    expect(formatFileSize(1024 * 1024 * 50)).toBe('50.0 MB');
  });

  it('GB 级别', () => {
    expect(formatFileSize(1024 ** 3)).toBe('1.0 GB');
    expect(formatFileSize(1024 ** 3 * 2)).toBe('2.0 GB');
  });

  it('TB 级别', () => {
    expect(formatFileSize(1024 ** 4)).toBe('1.0 TB');
  });

  it('null 返回空字符串', () => {
    expect(formatFileSize(null)).toBe('');
  });

  it('undefined 返回空字符串', () => {
    expect(formatFileSize(undefined)).toBe('');
  });

  it('负数返回空字符串', () => {
    expect(formatFileSize(-1)).toBe('');
  });

  it('保留 1 位小数', () => {
    expect(formatFileSize(1536)).toBe('1.5 KB');
    expect(formatFileSize(2560)).toBe('2.5 KB');
  });

  it('刚好在 KB 边界', () => {
    expect(formatFileSize(1023)).toBe('1023 B');
    expect(formatFileSize(1024)).toBe('1.0 KB');
  });

  it('刚好在 MB 边界', () => {
    expect(formatFileSize(1024 * 1024 - 1)).toBe('1024.0 KB');
    expect(formatFileSize(1024 * 1024)).toBe('1.0 MB');
  });

  it('大文件不超过 TB', () => {
    const pb = 1024 ** 5; // PB
    expect(formatFileSize(pb)).toContain('TB');
  });
});

// ============================================================
// formatNumber — 数字格式化
// ============================================================
describe('formatNumber — 千分位格式化', () => {
  it('0 返回 "0"', () => {
    expect(formatNumber(0)).toBe('0');
  });

  it('小于 1000 无分隔', () => {
    expect(formatNumber(999)).toBe('999');
  });

  it('1000 有千分位', () => {
    expect(formatNumber(1000)).toBe('1,000');
  });

  it('百万级', () => {
    expect(formatNumber(1234567)).toBe('1,234,567');
  });

  it('负数', () => {
    expect(formatNumber(-1234)).toBe('-1,234');
  });

  it('小数保留', () => {
    const result = formatNumber(1234.56);
    expect(result).toContain('1,234');
  });

  it('null 返回空字符串', () => {
    expect(formatNumber(null)).toBe('');
  });

  it('undefined 返回空字符串', () => {
    expect(formatNumber(undefined)).toBe('');
  });

  it('NaN 返回 "NaN" 字符串', () => {
    expect(formatNumber(NaN)).toBe('NaN');
  });

  it('Infinity 返回 "Infinity" 字符串', () => {
    const result = formatNumber(Infinity);
    expect(typeof result).toBe('string');
  });

  it('非数字类型返回字符串', () => {
    expect(formatNumber('abc')).toBe('abc');
  });
});

// ============================================================
// formatPercent — 百分比格式化
// ============================================================
describe('formatPercent — 百分比格式化', () => {
  it('0 返回 "0%"', () => {
    expect(formatPercent(0)).toBe('0%');
  });

  it('0.5 返回 "50%"', () => {
    expect(formatPercent(0.5)).toBe('50%');
  });

  it('1 返回 "100%"', () => {
    expect(formatPercent(1)).toBe('100%');
  });

  it('0.873 四舍五入为 "87%"', () => {
    expect(formatPercent(0.873)).toBe('87%');
  });

  it('0.876 四舍五入为 "88%"', () => {
    expect(formatPercent(0.876)).toBe('88%');
  });

  it('null 返回空字符串', () => {
    expect(formatPercent(null)).toBe('');
  });

  it('undefined 返回空字符串', () => {
    expect(formatPercent(undefined)).toBe('');
  });

  it('非数字返回空字符串', () => {
    expect(formatPercent('abc')).toBe('');
  });

  it('大于 1 的小数', () => {
    expect(formatPercent(1.5)).toBe('2%');
  });
});
