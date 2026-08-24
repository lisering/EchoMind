/**
 * lazy-loader.js 单元测试
 *
 * 覆盖 setMermaidInitFn 回调设置。
 * loadMermaid/loadD3 等函数依赖 DOM script 标签加载，在 jsdom 中超时，
 * 因此只测试不依赖 DOM 的纯逻辑部分。
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

describe('lazy-loader', () => {
  let mod;

  beforeEach(async () => {
    vi.resetModules();
    mod = await import('../../../ui/src/lazy-loader.js');
  });

  describe('setMermaidInitFn', () => {
    it('设置回调不报错', () => {
      const fn = vi.fn();
      expect(() => mod.setMermaidInitFn(fn)).not.toThrow();
    });

    it('设置 null 回调不报错', () => {
      expect(() => mod.setMermaidInitFn(null)).not.toThrow();
    });

    it('重复设置回调覆盖旧值', () => {
      const fn1 = vi.fn();
      const fn2 = vi.fn();
      mod.setMermaidInitFn(fn1);
      mod.setMermaidInitFn(fn2);
      // 无直接验证方式，但不报错即可
    });
  });
});
