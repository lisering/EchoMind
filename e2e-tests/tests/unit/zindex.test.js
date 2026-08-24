/**
 * zindex.js 单元测试
 *
 * 覆盖 Z_INDEX 常量正确性 + zClass() 辅助函数。
 */
import { describe, it, expect } from 'vitest';
import { Z_INDEX, zClass } from '../../../ui/src/panel-stack.js';

describe('zindex', () => {
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

    it('PANEL_3 = 65', () => {
      expect(Z_INDEX.PANEL_3).toBe(65);
    });

    it('PANEL_4 = 70', () => {
      expect(Z_INDEX.PANEL_4).toBe(70);
    });

    it('PANEL_5 = 75', () => {
      expect(Z_INDEX.PANEL_5).toBe(75);
    });

    it('COMMAND_PALETTE = 80', () => {
      expect(Z_INDEX.COMMAND_PALETTE).toBe(80);
    });

    it('GRAPH_VIEWER = 90', () => {
      expect(Z_INDEX.GRAPH_VIEWER).toBe(90);
    });

    it('AUDIT_LOG = 95', () => {
      expect(Z_INDEX.AUDIT_LOG).toBe(95);
    });

    it('LOCK_OVERLAY = 99999', () => {
      expect(Z_INDEX.LOCK_OVERLAY).toBe(99999);
    });
  });

  describe('层级递增顺序', () => {
    it('BASE < WIZARD < PANEL_1 < PANEL_2 < TOAST', () => {
      expect(Z_INDEX.BASE).toBeLessThan(Z_INDEX.WIZARD);
      expect(Z_INDEX.WIZARD).toBeLessThan(Z_INDEX.PANEL_1);
      expect(Z_INDEX.PANEL_1).toBeLessThan(Z_INDEX.PANEL_2);
      expect(Z_INDEX.PANEL_2).toBeLessThan(Z_INDEX.TOAST);
    });

    it('TOAST < PANEL_3 < PANEL_4 < PANEL_5', () => {
      expect(Z_INDEX.TOAST).toBeLessThan(Z_INDEX.PANEL_3);
      expect(Z_INDEX.PANEL_3).toBeLessThan(Z_INDEX.PANEL_4);
      expect(Z_INDEX.PANEL_4).toBeLessThan(Z_INDEX.PANEL_5);
    });

    it('PANEL_5 < COMMAND_PALETTE < GRAPH_VIEWER < AUDIT_LOG', () => {
      expect(Z_INDEX.PANEL_5).toBeLessThan(Z_INDEX.COMMAND_PALETTE);
      expect(Z_INDEX.COMMAND_PALETTE).toBeLessThan(Z_INDEX.GRAPH_VIEWER);
      expect(Z_INDEX.GRAPH_VIEWER).toBeLessThan(Z_INDEX.AUDIT_LOG);
    });

    it('LOCK_OVERLAY 是最大值', () => {
      expect(Z_INDEX.LOCK_OVERLAY).toBeGreaterThan(Z_INDEX.AUDIT_LOG);
      expect(Z_INDEX.LOCK_OVERLAY).toBeGreaterThan(1000);
    });
  });

  describe('zClass()', () => {
    it('Tailwind 内置值生成标准类名', () => {
      expect(zClass(0)).toBe('z-0');
      expect(zClass(10)).toBe('z-10');
      expect(zClass(20)).toBe('z-20');
      expect(zClass(30)).toBe('z-30');
      expect(zClass(40)).toBe('z-40');
      expect(zClass(50)).toBe('z-50');
    });

    it('非标准值生成 Tailwind 任意值类名', () => {
      expect(zClass(55)).toBe('z-[55]');
      expect(zClass(60)).toBe('z-[60]');
      expect(zClass(65)).toBe('z-[65]');
      expect(zClass(70)).toBe('z-[70]');
      expect(zClass(75)).toBe('z-[75]');
      expect(zClass(80)).toBe('z-[80]');
      expect(zClass(90)).toBe('z-[90]');
      expect(zClass(95)).toBe('z-[95]');
    });

    it('LOCK_OVERLAY 生成任意值类名', () => {
      expect(zClass(99999)).toBe('z-[99999]');
    });

    it('传入 Z_INDEX 常量值生成正确类名', () => {
      expect(zClass(Z_INDEX.BASE)).toBe('z-0');
      expect(zClass(Z_INDEX.WIZARD)).toBe('z-40');
      expect(zClass(Z_INDEX.PANEL_1)).toBe('z-50');
      expect(zClass(Z_INDEX.PANEL_2)).toBe('z-[55]');
      expect(zClass(Z_INDEX.TOAST)).toBe('z-[60]');
      expect(zClass(Z_INDEX.LOCK_OVERLAY)).toBe('z-[99999]');
    });
  });
});
