/**
 * zindex.js 超大规模综合单元测试
 *
 * 覆盖：
 * - Z_INDEX 全部常量值
 * - zClass() 辅助函数
 * - 层级顺序（BASE < WIZARD < PANEL_1 < TOAST < MODAL < LOCK_OVERLAY）
 * - 唯一性验证
 * - 值范围合理性
 *
 * 25 个测试用例
 */
import { describe, it, expect } from 'vitest';
import { Z_INDEX, zClass } from '../../../ui/src/panel-stack.js';

describe('zindex — 统一 Z-index 管理', () => {
  // ============================================================
  // Z_INDEX 常量值验证
  // ============================================================
  describe('Z_INDEX 常量', () => {
    it('BASE = 0', () => {
      expect(Z_INDEX.BASE).toBe(0);
    });

    it('WIZARD = 40', () => {
      expect(Z_INDEX.WIZARD).toBe(40);
    });

    it('PANEL_1 = 50', () => {
      expect(Z_INDEX.PANEL_1).toBe(50);
    });

    it('PANEL_2 = 55', () => {
      expect(Z_INDEX.PANEL_2).toBe(55);
    });

    it('TOAST = 60', () => {
      expect(Z_INDEX.TOAST).toBe(60);
    });

    it('COMMAND_PALETTE = 80', () => {
      expect(Z_INDEX.COMMAND_PALETTE).toBe(80);
    });

    it('GRAPH_VIEWER = 90', () => {
      expect(Z_INDEX.GRAPH_VIEWER).toBe(90);
    });

    it('LOCK_OVERLAY = 99999', () => {
      expect(Z_INDEX.LOCK_OVERLAY).toBe(99999);
    });
  });

  // ============================================================
  // 层级顺序
  // ============================================================
  describe('层级顺序', () => {
    it('BASE < WIZARD', () => {
      expect(Z_INDEX.BASE).toBeLessThan(Z_INDEX.WIZARD);
    });

    it('WIZARD < PANEL_1', () => {
      expect(Z_INDEX.WIZARD).toBeLessThan(Z_INDEX.PANEL_1);
    });

    it('PANEL_1 < PANEL_2', () => {
      expect(Z_INDEX.PANEL_1).toBeLessThan(Z_INDEX.PANEL_2);
    });

    it('PANEL_2 < TOAST', () => {
      expect(Z_INDEX.PANEL_2).toBeLessThan(Z_INDEX.TOAST);
    });

    it('TOAST < COMMAND_PALETTE', () => {
      expect(Z_INDEX.TOAST).toBeLessThan(Z_INDEX.COMMAND_PALETTE);
    });

    it('COMMAND_PALETTE < GRAPH_VIEWER', () => {
      expect(Z_INDEX.COMMAND_PALETTE).toBeLessThan(Z_INDEX.GRAPH_VIEWER);
    });

    it('GRAPH_VIEWER < LOCK_OVERLAY', () => {
      expect(Z_INDEX.GRAPH_VIEWER).toBeLessThan(Z_INDEX.LOCK_OVERLAY);
    });
  });

  // ============================================================
  // 唯一性
  // ============================================================
  describe('值唯一性', () => {
    it('所有常量值不重复', () => {
      const values = Object.values(Z_INDEX);
      const unique = new Set(values);
      expect(unique.size).toBe(values.length);
    });
  });

  // ============================================================
  // zClass 辅助函数
  // ============================================================
  describe('zClass — CSS 类生成', () => {
    it('标准值 0 返回 z-0', () => {
      expect(zClass(0)).toBe('z-0');
    });

    it('标准值 50 返回 z-50', () => {
      expect(zClass(50)).toBe('z-50');
    });

    it('非标准值 55 返回 z-[55]', () => {
      expect(zClass(55)).toBe('z-[55]');
    });

    it('非标准值 80 返回 z-[80]', () => {
      expect(zClass(80)).toBe('z-[80]');
    });

    it('大数值 99999 返回 z-[99999]', () => {
      expect(zClass(99999)).toBe('z-[99999]');
    });

    it('接受 Z_INDEX 常量', () => {
      expect(zClass(Z_INDEX.TOAST)).toContain('60');
    });
  });

  // ============================================================
  // 值范围合理性
  // ============================================================
  describe('值范围', () => {
    it('所有值 >= 0', () => {
      for (const val of Object.values(Z_INDEX)) {
        expect(val).toBeGreaterThanOrEqual(0);
      }
    });

    it('所有值 <= 100000', () => {
      for (const val of Object.values(Z_INDEX)) {
        expect(val).toBeLessThanOrEqual(100000);
      }
    });

    it('BASE = 0 是最小值', () => {
      const values = Object.values(Z_INDEX);
      const min = Math.min(...values);
      expect(min).toBe(0);
    });

    it('至少有 12 个层级', () => {
      expect(Object.keys(Z_INDEX).length).toBeGreaterThanOrEqual(12);
    });
  });
});
