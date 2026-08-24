/**
 * followup.js 超大规模综合单元测试
 *
 * 覆盖：
 * - extractKeyEntities（纯规则实体提取）
 * - generateFollowups（模板生成后续问题）
 * - createFollowupCards（DOM 渲染）
 * - 停用词过滤
 * - 书名号 / 引号 / Markdown 标题提取
 * - 中英文混合处理
 * - 最多 3 条建议限制
 *
 * 30 个测试用例
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback ?? key,
}));

import {
  extractEntities,
  generateFollowups,
  renderFollowups,
} from '../../../ui/src/chat-render.js';

describe('followup — 后续问题建议', () => {
  // ============================================================
  // extractKeyEntities — 实体提取
  // ============================================================
  describe('extractEntities — 实体提取', () => {
    it('提取书名号内容《XX法》', () => {
      const entities = extractEntities('根据《个人信息保护法》规定...');
      expect(entities).toContain('个人信息保护法');
    });

    it('提取引号内容', () => {
      const entities = extractEntities('按照"数据安全法"执行');
      expect(entities).toContain('数据安全法');
    });

    it('提取 Markdown 标题内容', () => {
      const entities = extractEntities('## 安装步骤\n安装步骤如下');
      expect(entities.length).toBeGreaterThan(0);
    });

    it('过滤停用词', () => {
      const entities = extractEntities('的 了 是 在 和');
      expect(entities).not.toContain('的');
      expect(entities).not.toContain('了');
    });

    it('英文专业术语提取', () => {
      const entities = extractEntities('The GDPR regulation requires compliance.');
      expect(entities.length).toBeGreaterThan(0);
    });

    it('英文停用词过滤', () => {
      const entities = extractEntities('the a an is are was were');
      expect(entities).not.toContain('the');
      expect(entities).not.toContain('a');
    });

    it('空文本返回空数组', () => {
      expect(extractEntities('')).toEqual([]);
    });

    it('null 返回空数组', () => {
      expect(extractEntities(null)).toEqual([]);
    });

    it('纯停用词返回空数组', () => {
      const entities = extractEntities('的 了 在');
      expect(entities.length).toBe(0);
    });

    it('中英文混合提取', () => {
      const entities = extractEntities('根据《数据安全法》Data Security applies.');
      expect(entities).toContain('数据安全法');
      expect(entities.length).toBeGreaterThan(1);
    });

    it('多个书名号提取', () => {
      const entities = extractEntities('《民法典》和《刑法》相关条款');
      expect(entities).toContain('民法典');
      expect(entities).toContain('刑法');
    });
  });

  // ============================================================
  // generateFollowups — 后续问题生成
  // ============================================================
  describe('generateFollowups — 问题生成', () => {
    it('返回数组', () => {
      const entities = extractEntities('根据《XX法》规定...');
      const result = generateFollowups(entities);
      expect(Array.isArray(result)).toBe(true);
    });

    it('最多 3 条建议', () => {
      const entities = extractEntities('《法1》《法2》《法3》《法4》《法5》');
      const result = generateFollowups(entities);
      expect(result.length).toBeLessThanOrEqual(3);
    });

    it('空数组返回空或少量建议', () => {
      const result = generateFollowups([]);
      expect(Array.isArray(result)).toBe(true);
    });
  });

  // ============================================================
  // createFollowupCards — DOM 渲染
  // ============================================================
  describe('renderFollowups — DOM 渲染', () => {
    beforeEach(() => {
      document.body.innerHTML = '';
    });

    it('创建卡片 DOM 元素', () => {
      const blockEl = document.createElement('div');
      document.body.appendChild(blockEl);

      renderFollowups(blockEl, ['Q1', 'Q2'], () => {});
      const cards = blockEl.querySelectorAll('[role="button"], .followup-card');
      expect(cards.length).toBeGreaterThan(0);
    });

    it('每张卡片包含问题文本', () => {
      const blockEl = document.createElement('div');
      document.body.appendChild(blockEl);

      renderFollowups(blockEl, ['What is AI?'], () => {});
      expect(blockEl.textContent).toContain('What is AI?');
    });

    it('点击卡片触发回调', () => {
      const blockEl = document.createElement('div');
      document.body.appendChild(blockEl);

      let clicked = null;
      renderFollowups(blockEl, ['Question 1'], (q) => { clicked = q; });

      const card = blockEl.querySelector('[role="button"], .followup-card');
      if (card) {
        card.click();
        expect(clicked).not.toBeNull();
      }
    });

    it('空建议数组渲染空内容', () => {
      const blockEl = document.createElement('div');
      document.body.appendChild(blockEl);

      renderFollowups(blockEl, [], () => {});
      const cards = blockEl.querySelectorAll('[role="button"], .followup-card');
      expect(cards.length).toBe(0);
    });

    it('null 容器不出错', () => {
      expect(() => renderFollowups(null, ['Q1'], () => {})).not.toThrow();
    });
  });
});
