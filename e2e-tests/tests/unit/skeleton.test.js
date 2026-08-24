/**
 * EchoMind skeleton.js 单元测试 — 骨架屏 / 加载占位 / 过渡动画。
 *
 * 验证点：
 * 1. showSkeleton 无容器时安全返回
 * 2. showSkeleton 200ms 后插入骨架 DOM
 * 3. showSkeleton 创建指定数量的骨架项
 * 4. showSkeleton doc 类型骨架项宽度为 120px
 * 5. showSkeleton conv 类型骨架项宽度为 160px
 * 6. hideSkeleton 移除骨架 DOM
 * 7. hideSkeleton 清除定时器
 * 8. showSkeleton 重复调用先清除已有骨架
 * 9. showSkeleton 容器已有内容时不插入骨架
 * 10. hideSkeleton 无容器时安全返回
 * 11. showSkeleton 骨架项包含 animate-pulse 类
 * 12. showSkeleton 骨架容器包含 skeleton-container 类
 *
 * Mock: 无外部依赖
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

import { showSkeleton, hideSkeleton } from '../../../ui/src/utils.js';

// Helper: advance fake timers
vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });

describe('skeleton.js — 骨架屏', () => {
  beforeEach(() => {
    vi.clearAllTimers();
  });

  it('showSkeleton 无容器时安全返回不报错', () => {
    expect(() => showSkeleton(null)).not.toThrow();
  });

  it('showSkeleton 200ms 后插入骨架 DOM', () => {
    const container = document.createElement('div');
    container.id = 'test-container';
    document.body.appendChild(container);

    showSkeleton(container);
    // 200ms 之前不应有骨架
    expect(container.children.length).toBe(0);

    // 快进 200ms
    vi.advanceTimersByTime(200);

    expect(container.children.length).toBe(1);
    expect(container.children[0].classList.contains('skeleton-container')).toBe(true);

    container.remove();
  });

  it('showSkeleton 创建指定数量的骨架项', () => {
    const container = document.createElement('div');
    container.id = 'test-count';
    document.body.appendChild(container);

    showSkeleton(container, 'doc', 6);
    vi.advanceTimersByTime(200);

    const skeleton = container.querySelector('.skeleton-container');
    expect(skeleton.children.length).toBe(6);

    container.remove();
  });

  it('showSkeleton doc 类型骨架项名称占位宽度为 120px', () => {
    const container = document.createElement('div');
    container.id = 'test-doc-width';
    document.body.appendChild(container);

    showSkeleton(container, 'doc', 1);
    vi.advanceTimersByTime(200);

    const nameBlock = container.querySelector('.skeleton-container > div > div:first-child');
    expect(nameBlock.style.width).toBe('120px');

    container.remove();
  });

  it('showSkeleton conv 类型骨架项名称占位宽度为 160px', () => {
    const container = document.createElement('div');
    container.id = 'test-conv-width';
    document.body.appendChild(container);

    showSkeleton(container, 'conv', 1);
    vi.advanceTimersByTime(200);

    const nameBlock = container.querySelector('.skeleton-container > div > div:first-child');
    expect(nameBlock.style.width).toBe('160px');

    container.remove();
  });

  it('hideSkeleton 移除骨架 DOM', () => {
    const container = document.createElement('div');
    container.id = 'test-hide';
    document.body.appendChild(container);

    showSkeleton(container, 'doc', 3);
    vi.advanceTimersByTime(200);
    expect(container.querySelector('.skeleton-container')).not.toBeNull();

    hideSkeleton(container);
    expect(container.querySelector('.skeleton-container')).toBeNull();

    container.remove();
  });

  it('hideSkeleton 清除定时器（阻止延迟骨架出现）', () => {
    const container = document.createElement('div');
    container.id = 'test-timer';
    document.body.appendChild(container);

    showSkeleton(container, 'doc', 2);
    // 在 200ms 到达之前调用 hideSkeleton
    hideSkeleton(container);

    // 快进 200ms — 骨架不应出现（定时器已清除）
    vi.advanceTimersByTime(200);
    expect(container.querySelector('.skeleton-container')).toBeNull();

    container.remove();
  });

  it('showSkeleton 重复调用先清除已有骨架', () => {
    const container = document.createElement('div');
    container.id = 'test-repeat';
    document.body.appendChild(container);

    // 第一次调用
    showSkeleton(container, 'doc', 3);
    vi.advanceTimersByTime(200);
    expect(container.querySelector('.skeleton-container')).not.toBeNull();

    // 第二次调用 — 应先清除已有骨架
    showSkeleton(container, 'doc', 5);
    vi.advanceTimersByTime(200);

    const skeleton = container.querySelector('.skeleton-container');
    expect(skeleton).not.toBeNull();
    expect(skeleton.children.length).toBe(5);

    container.remove();
  });

  it('showSkeleton 容器已有内容时不插入骨架', () => {
    const container = document.createElement('div');
    container.id = 'test-has-content';
    // 预填充内容
    container.innerHTML = '<div class="real-content">Real</div>';
    document.body.appendChild(container);

    showSkeleton(container, 'doc', 3);
    vi.advanceTimersByTime(200);

    // 骨架不应插入（容器已有内容）
    expect(container.querySelector('.skeleton-container')).toBeNull();
    expect(container.querySelector('.real-content')).not.toBeNull();

    container.remove();
  });

  it('hideSkeleton 无容器时安全返回不报错', () => {
    expect(() => hideSkeleton(null)).not.toThrow();
  });

  it('showSkeleton 骨架项包含 animate-pulse 类', () => {
    const container = document.createElement('div');
    container.id = 'test-pulse';
    document.body.appendChild(container);

    showSkeleton(container, 'doc', 1);
    vi.advanceTimersByTime(200);

    const pulseBlocks = container.querySelectorAll('.animate-pulse');
    expect(pulseBlocks.length).toBeGreaterThan(0);

    container.remove();
  });

  it('showSkeleton 骨架容器包含 skeleton-container 类', () => {
    const container = document.createElement('div');
    container.id = 'test-class';
    document.body.appendChild(container);

    showSkeleton(container, 'doc', 2);
    vi.advanceTimersByTime(200);

    const skeleton = container.querySelector('.skeleton-container');
    expect(skeleton).not.toBeNull();
    expect(skeleton.className).toContain('skeleton-container');

    container.remove();
  });

  it('showSkeleton 默认 count=4', () => {
    const container = document.createElement('div');
    container.id = 'test-default-count';
    document.body.appendChild(container);

    showSkeleton(container);
    vi.advanceTimersByTime(200);

    const skeleton = container.querySelector('.skeleton-container');
    expect(skeleton.children.length).toBe(4);

    container.remove();
  });
});
