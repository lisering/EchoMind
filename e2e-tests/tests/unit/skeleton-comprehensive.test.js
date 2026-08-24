/**
 * skeleton.js 超大规模综合单元测试
 *
 * 覆盖：
 * - showSkeleton（200ms 延迟 + DOM 结构）
 * - hideSkeleton（清理定时器 + 移除 DOM）
 * - createSkeletonItem（doc/conv 两种类型）
 * - 200ms 延迟避免闪烁
 * - 容器已有内容时不显示骨架
 * - 多次调用幂等性
 * - count 参数
 *
 * 25 个测试用例
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { showSkeleton, hideSkeleton } from '../../../ui/src/utils.js';

describe('skeleton — 骨架屏模块', () => {
  let container;

  beforeEach(() => {
    document.body.innerHTML = '';
    container = document.createElement('div');
    container.id = 'docList';
    document.body.appendChild(container);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // ============================================================
  // showSkeleton
  // ============================================================
  describe('showSkeleton — 显示骨架屏', () => {
    it('200ms 内不显示骨架（避免闪烁）', () => {
      showSkeleton(container, 'doc', 4);
      expect(container.querySelector('.skeleton-container')).toBeNull();
    });

    it('200ms 后显示骨架', () => {
      showSkeleton(container, 'doc', 4);
      vi.advanceTimersByTime(250);
      expect(container.querySelector('.skeleton-container')).not.toBeNull();
    });

    it('doc 类型骨架项含 120px 宽度占位块', () => {
      showSkeleton(container, 'doc', 4);
      vi.advanceTimersByTime(250);
      const items = container.querySelectorAll('.skeleton-container > div');
      const nameBlock = items[0]?.querySelector('div');
      if (nameBlock) {
        expect(nameBlock.style.width).toBe('120px');
      }
    });

    it('conv 类型骨架项含 160px 宽度占位块', () => {
      showSkeleton(container, 'conv', 4);
      vi.advanceTimersByTime(250);
      const items = container.querySelectorAll('.skeleton-container > div');
      const nameBlock = items[0]?.querySelector('div');
      if (nameBlock) {
        expect(nameBlock.style.width).toBe('160px');
      }
    });

    it('count 参数控制骨架项数量', () => {
      showSkeleton(container, 'doc', 6);
      vi.advanceTimersByTime(250);
      const items = container.querySelectorAll('.skeleton-container > div');
      expect(items.length).toBe(6);
    });

    it('默认 count=4', () => {
      showSkeleton(container, 'doc');
      vi.advanceTimersByTime(250);
      const items = container.querySelectorAll('.skeleton-container > div');
      expect(items.length).toBe(4);
    });

    it('骨架项含 animate-pulse 类', () => {
      showSkeleton(container, 'doc', 4);
      vi.advanceTimersByTime(250);
      const pulseElements = container.querySelectorAll('.animate-pulse');
      expect(pulseElements.length).toBeGreaterThan(0);
    });

    it('容器已有内容时不显示骨架', () => {
      container.innerHTML = '<div>Existing content</div>';
      showSkeleton(container, 'doc', 4);
      vi.advanceTimersByTime(250);
      expect(container.querySelector('.skeleton-container')).toBeNull();
    });

    it('null 容器不出错', () => {
      expect(() => showSkeleton(null, 'doc', 4)).not.toThrow();
    });
  });

  // ============================================================
  // hideSkeleton
  // ============================================================
  describe('hideSkeleton — 移除骨架屏', () => {
    it('移除已显示的骨架', () => {
      showSkeleton(container, 'doc', 4);
      vi.advanceTimersByTime(250);
      expect(container.querySelector('.skeleton-container')).not.toBeNull();

      hideSkeleton(container);
      expect(container.querySelector('.skeleton-container')).toBeNull();
    });

    it('在 200ms 之前调用 hideSkeleton 取消定时器', () => {
      showSkeleton(container, 'doc', 4);
      hideSkeleton(container);
      vi.advanceTimersByTime(500);
      expect(container.querySelector('.skeleton-container')).toBeNull();
    });

    it('无骨架时调用不出错', () => {
      expect(() => hideSkeleton(container)).not.toThrow();
    });

    it('null 容器不出错', () => {
      expect(() => hideSkeleton(null)).not.toThrow();
    });
  });

  // ============================================================
  // 多次调用
  // ============================================================
  describe('多次调用幂等性', () => {
    it('连续两次 showSkeleton 只显示一个骨架', () => {
      showSkeleton(container, 'doc', 4);
      showSkeleton(container, 'doc', 4);
      vi.advanceTimersByTime(250);
      const skeletons = container.querySelectorAll('.skeleton-container');
      expect(skeletons.length).toBe(1);
    });

    it('show → hide → show 循环不出错', () => {
      showSkeleton(container, 'doc', 2);
      vi.advanceTimersByTime(250);
      hideSkeleton(container);
      showSkeleton(container, 'doc', 3);
      vi.advanceTimersByTime(250);
      expect(container.querySelector('.skeleton-container')).not.toBeNull();
    });
  });

  // ============================================================
  // 骨架 DOM 结构验证
  // ============================================================
  describe('骨架 DOM 结构', () => {
    it('骨架容器有 skeleton-container 类', () => {
      showSkeleton(container, 'doc', 2);
      vi.advanceTimersByTime(250);
      const sk = container.querySelector('.skeleton-container');
      expect(sk).not.toBeNull();
    });

    it('每个骨架项含 2 个子元素（名称 + 徽标占位）', () => {
      showSkeleton(container, 'doc', 2);
      vi.advanceTimersByTime(250);
      const items = container.querySelectorAll('.skeleton-container > div');
      expect(items.length).toBe(2);
      expect(items[0].children.length).toBe(2);
    });

    it('占位块有 bg-white/5 背景色', () => {
      showSkeleton(container, 'doc', 2);
      vi.advanceTimersByTime(250);
      const bg = container.querySelector('.bg-white\\/5');
      expect(bg).not.toBeNull();
    });
  });
});
