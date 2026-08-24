/**
 * EchoMind 空状态重设计单元测试 — empty-state.js 模块（TC-QA-005~011）。
 *
 * 验证点（对应 AC-QA-003）：
 * 1. 渲染 EchoMind logo + tagline
 * 2. 渲染知识库摘要卡片（文档数 / chunk 数）
 * 3. 渲染隐私状态卡片（加密状态 / PII 状态）
 * 4. 渲染 3 个推荐问题卡片
 * 5. 点击推荐问题卡片触发 onPickQuestion 回调
 * 6. 知识库为空（0 篇文档）时仍正常渲染引导
 * 7. opts 参数缺失时安全降级（不抛异常）
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderEmptyState, generateRecommendations } from '../../../ui/src/empty-state.js';

describe('Empty State — empty-state.js', () => {
  let container;

  beforeEach(() => {
    container = document.createElement('div');
    container.id = 'chatArea';
    document.body.appendChild(container);
  });

  describe('renderEmptyState', () => {
    it('TC-QA-005: 渲染 EchoMind logo + tagline', () => {
      renderEmptyState(container, { docCount: 10, chunkCount: 500 });
      const logo = container.querySelector('.empty-state-logo');
      expect(logo).not.toBeNull();
      // v1.21: logo 从 Unicode ◈ 改为 SVG inline 图标
      expect(logo.querySelector('svg')).not.toBeNull();
      const tagline = container.querySelector('.empty-state-tagline');
      expect(tagline).not.toBeNull();
    });

    it('TC-QA-006: 渲染知识库摘要卡片含文档数和 chunk 数', () => {
      renderEmptyState(container, { docCount: 23, chunkCount: 1247 });
      const kbCard = container.querySelector('.empty-state-kb-card');
      expect(kbCard).not.toBeNull();
      // toLocaleString() 可能插入千位分隔符（如 1,247），移除后断言
      const plainText = kbCard.textContent.replace(/,/g, '');
      expect(plainText).toContain('23');
      expect(plainText).toContain('1247');
    });

    it('TC-QA-007: 渲染隐私状态卡片含加密状态', () => {
      renderEmptyState(container, { docCount: 5, chunkCount: 100, encrypted: true });
      const privacyCard = container.querySelector('.empty-state-privacy-card');
      expect(privacyCard).not.toBeNull();
      // v1.21: 从 emoji 🔒 改为 SVG inline lock 图标
      const iconSvg = privacyCard.querySelector('.empty-state-card-icon svg');
      expect(iconSvg).not.toBeNull();
    });

    it('TC-QA-007b: 未加密时隐私卡片显示未加密状态', () => {
      renderEmptyState(container, { docCount: 5, chunkCount: 100, encrypted: false });
      const privacyCard = container.querySelector('.empty-state-privacy-card');
      expect(privacyCard).not.toBeNull();
      // v1.21: 从 emoji 🔓 改为 SVG inline unlock 图标
      const iconSvg = privacyCard.querySelector('.empty-state-card-icon svg');
      expect(iconSvg).not.toBeNull();
    });

    it('TC-QA-008: 渲染 3 个推荐问题卡片', () => {
      renderEmptyState(container, { docCount: 10, chunkCount: 500 });
      const cards = container.querySelectorAll('.empty-state-suggestion-card');
      expect(cards).toHaveLength(3);
    });

    it('TC-QA-009: 点击推荐问题卡片触发 onPickQuestion 回调', () => {
      const spy = vi.fn();
      renderEmptyState(container, {
        docCount: 10,
        chunkCount: 500,
        onPickQuestion: spy,
      });
      const firstCard = container.querySelector('.empty-state-suggestion-card');
      firstCard.click();
      expect(spy).toHaveBeenCalledTimes(1);
      expect(spy).toHaveBeenCalledWith(expect.any(String));
    });

    it('TC-QA-010: 知识库为空（0 篇文档）时仍正常渲染引导', () => {
      expect(() => {
        renderEmptyState(container, { docCount: 0, chunkCount: 0 });
      }).not.toThrow();
      const logo = container.querySelector('.empty-state-logo');
      expect(logo).not.toBeNull();
    });

    it('TC-QA-011: opts 参数缺失时安全降级不抛异常', () => {
      expect(() => {
        renderEmptyState(container);
      }).not.toThrow();
      const logo = container.querySelector('.empty-state-logo');
      expect(logo).not.toBeNull();
    });

    it('TC-QA-010c: 无文档时渲染导入引导按钮', () => {
      renderEmptyState(container, { docCount: 0, chunkCount: 0 });
      const importBtn = container.querySelector('.empty-state-import-btn');
      expect(importBtn).not.toBeNull();
      // 不应有推荐问题卡片
      const suggestionCards = container.querySelectorAll('.empty-state-suggestion-card');
      expect(suggestionCards).toHaveLength(0);
    });

    it('TC-QA-010d: 点击导入按钮触发 onImport 回调', () => {
      const spy = vi.fn();
      renderEmptyState(container, {
        docCount: 0,
        chunkCount: 0,
        onImport: spy,
      });
      const importBtn = container.querySelector('.empty-state-import-btn');
      expect(importBtn).not.toBeNull();
      importBtn.click();
      expect(spy).toHaveBeenCalledTimes(1);
    });
  });

  describe('generateRecommendations', () => {
    it('TC-QA-008b: 有文档时生成 3 个推荐问题', () => {
      const recs = generateRecommendations(10);
      expect(recs).toHaveLength(3);
      recs.forEach((r) => {
        expect(typeof r).toBe('string');
        expect(r.length).toBeGreaterThan(0);
      });
    });

    it('TC-QA-010b: 无文档时不生成推荐问题（改为导入引导按钮）', () => {
      const recs = generateRecommendations(0);
      expect(recs).toHaveLength(0);
    });
  });
});
