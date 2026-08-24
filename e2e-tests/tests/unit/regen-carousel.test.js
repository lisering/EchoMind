/**
 * EchoMind 重新生成轮播单元测试 — regen-carousel.js 模块（TC-QA-040~046）。
 *
 * 验证点（对应 AC-QA-014 重新生成轮播）：
 * 1. createCarousel 创建轮播容器
 * 2. addCarouselVersion 增加版本计数
 * 3. navigateCarousel 改变当前索引
 * 4. navigateCarousel 在边界时回绕
 * 5. getCarouselState 返回正确的索引和总数
 * 6. updateCarouselDisplay 显示当前版本内容
 * 7. renderCarouselControls 显示 1/N 格式
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  createCarousel,
  addCarouselVersion,
  navigateCarousel,
  getCarouselState,
  updateCarouselDisplay,
  renderCarouselControls,
} from '../../../ui/src/chat-render.js';

describe('Regen Carousel — regen-carousel.js', () => {
  let blockEl;
  let mdContainer;

  beforeEach(() => {
    blockEl = document.createElement('div');
    blockEl.className = 'msg-block msg-assistant';

    // 模拟 assistant 消息块的结构
    const content = document.createElement('div');
    content.className = 'msg-content';
    mdContainer = document.createElement('div');
    mdContainer.className = 'md';
    content.appendChild(mdContainer);
    blockEl.appendChild(content);
    blockEl.appendChild(document.createElement('div', { className: 'msg-actions' }));

    document.body.appendChild(blockEl);
  });

  describe('createCarousel', () => {
    it('TC-QA-040: 创建 .regen-carousel 容器', () => {
      const carousel = createCarousel(blockEl);
      expect(carousel).not.toBeNull();
      expect(carousel.classList.contains('regen-carousel')).toBe(true);
    });

    it('TC-QA-040b: 初始版本计数为 0', () => {
      const carousel = createCarousel(blockEl);
      const state = getCarouselState(carousel);
      expect(state.total).toBe(0);
      expect(state.current).toBe(0);
    });
  });

  describe('addCarouselVersion', () => {
    it('TC-QA-041: 增加版本计数', () => {
      const carousel = createCarousel(blockEl);
      addCarouselVersion(carousel, '回答1', null);
      const state = getCarouselState(carousel);
      expect(state.total).toBe(1);
      expect(state.current).toBe(0);
    });

    it('TC-QA-041b: 多次添加后当前索引指向最新版本', () => {
      const carousel = createCarousel(blockEl);
      addCarouselVersion(carousel, '回答1', null);
      addCarouselVersion(carousel, '回答2', null);
      addCarouselVersion(carousel, '回答3', null);
      const state = getCarouselState(carousel);
      expect(state.total).toBe(3);
      expect(state.current).toBe(2); // 0-based, 指向最新
    });

    it('TC-QA-041c: 存储版本内容和来源', () => {
      const carousel = createCarousel(blockEl);
      const sources = [{ doc_name: 'test.md', score: 0.9 }];
      addCarouselVersion(carousel, '回答内容', sources);
      const state = getCarouselState(carousel);
      expect(state.versions).toHaveLength(1);
      expect(state.versions[0].content).toBe('回答内容');
      expect(state.versions[0].sources).toBe(sources);
    });
  });

  describe('navigateCarousel', () => {
    it('TC-QA-042: 向右导航改变当前索引', () => {
      const carousel = createCarousel(blockEl);
      addCarouselVersion(carousel, '回答1', null);
      addCarouselVersion(carousel, '回答2', null);
      addCarouselVersion(carousel, '回答3', null);
      // 当前索引 = 2（最新），导航 left → 索引 1
      navigateCarousel(carousel, 'left');
      const state = getCarouselState(carousel);
      expect(state.current).toBe(1);
    });

    it('TC-QA-042b: 向左导航改变当前索引', () => {
      const carousel = createCarousel(blockEl);
      addCarouselVersion(carousel, '回答1', null);
      addCarouselVersion(carousel, '回答2', null);
      addCarouselVersion(carousel, '回答3', null);
      navigateCarousel(carousel, 'left');
      navigateCarousel(carousel, 'left');
      navigateCarousel(carousel, 'right');
      const state = getCarouselState(carousel);
      expect(state.current).toBe(1);
    });

    it('TC-QA-043: 在首项时向左导航回绕到末项', () => {
      const carousel = createCarousel(blockEl);
      addCarouselVersion(carousel, '回答1', null);
      addCarouselVersion(carousel, '回答2', null);
      addCarouselVersion(carousel, '回答3', null);
      // 当前 = 2, left → 1, left → 0, left → wrap to 2
      navigateCarousel(carousel, 'left');
      navigateCarousel(carousel, 'left');
      navigateCarousel(carousel, 'left');
      const state = getCarouselState(carousel);
      expect(state.current).toBe(2);
    });

    it('TC-QA-043b: 在末项时向右导航回绕到首项', () => {
      const carousel = createCarousel(blockEl);
      addCarouselVersion(carousel, '回答1', null);
      addCarouselVersion(carousel, '回答2', null);
      addCarouselVersion(carousel, '回答3', null);
      // 当前 = 2 (latest), right → wrap to 0
      navigateCarousel(carousel, 'right');
      const state = getCarouselState(carousel);
      expect(state.current).toBe(0);
    });

    it('TC-QA-043c: 仅 1 个版本时导航不改变索引', () => {
      const carousel = createCarousel(blockEl);
      addCarouselVersion(carousel, '回答1', null);
      navigateCarousel(carousel, 'left');
      const state = getCarouselState(carousel);
      expect(state.current).toBe(0);
    });
  });

  describe('renderCarouselControls', () => {
    it('TC-QA-046: 显示 1/N 格式', () => {
      const carousel = createCarousel(blockEl);
      addCarouselVersion(carousel, '回答1', null);
      addCarouselVersion(carousel, '回答2', null);
      renderCarouselControls(carousel);
      const counter = carousel.querySelector('.regen-carousel-counter');
      expect(counter).not.toBeNull();
      // 当前 = 2 (latest, 0-based 1), total = 2 → "2/2"
      expect(counter.textContent).toContain('2');
      expect(counter.textContent).toContain('/');
    });

    it('TC-QA-046b: 仅 1 个版本时仍显示控制', () => {
      const carousel = createCarousel(blockEl);
      addCarouselVersion(carousel, '回答1', null);
      renderCarouselControls(carousel);
      const counter = carousel.querySelector('.regen-carousel-counter');
      expect(counter).not.toBeNull();
      expect(counter.textContent).toContain('1');
    });

    it('TC-QA-046c: 0 个版本时不渲染控制', () => {
      const carousel = createCarousel(blockEl);
      renderCarouselControls(carousel);
      const controls = carousel.querySelector('.regen-carousel-controls');
      expect(controls).toBeNull();
    });
  });

  describe('updateCarouselDisplay', () => {
    it('TC-QA-045: 显示当前版本内容', () => {
      const carousel = createCarousel(blockEl);
      addCarouselVersion(carousel, '第一个回答', null);
      addCarouselVersion(carousel, '第二个回答', null);
      updateCarouselDisplay(carousel, mdContainer);
      expect(mdContainer.textContent).toContain('第二个回答');
    });

    it('TC-QA-045b: 导航后显示对应版本', () => {
      const carousel = createCarousel(blockEl);
      addCarouselVersion(carousel, '第一个回答', null);
      addCarouselVersion(carousel, '第二个回答', null);
      navigateCarousel(carousel, 'left');
      updateCarouselDisplay(carousel, mdContainer);
      expect(mdContainer.textContent).toContain('第一个回答');
    });
  });
});
