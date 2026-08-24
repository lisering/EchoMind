/**
 * EchoMind 后续问题建议单元测试 — followup.js 模块（TC-QA-068~075）。
 *
 * 测试覆盖：
 * - TC-QA-068: extractEntities 从中文文本提取书名号内容
 * - TC-QA-069: extractEntities 提取 Markdown 标题内容
 * - TC-QA-070: generateFollowups 基于实体生成 2-3 个后续问题
 * - TC-QA-071: generateFollowups 无实体时返回通用追问
 * - TC-QA-072: renderFollowups 渲染建议卡片 + 关闭按钮
 * - TC-QA-073: 点击建议卡片触发 onPick 回调
 * - TC-QA-074: 关闭按钮移除建议容器
 * - TC-QA-075: 最多 3 条建议
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { JSDOM } from 'jsdom';
import { extractEntities, generateFollowups, renderFollowups, removeFollowups, renderFollowupSuggestions } from '../../../ui/src/chat-render.js';

// ============================================================
// DOM 环境设置
// ============================================================

beforeEach(() => {
  const dom = new JSDOM('<!DOCTYPE html><html><body></body></html>', {
    url: 'http://localhost',
    pretendToBeVisual: true,
  });
  global.window = dom.window;
  global.document = dom.window.document;
  global.HTMLElement = dom.window.HTMLElement;
  global.localStorage = {
    store: {},
    getItem(key) { return this.store[key] || null; },
    setItem(key, val) { this.store[key] = val; },
    removeItem(key) { delete this.store[key]; },
  };
});

// ============================================================
// 测试用例
// ============================================================

describe('followup.js — 后续问题建议', () => {
  // ----------------------------------------------------------
  // TC-QA-068: extractEntities 从中文文本提取书名号内容
  // ----------------------------------------------------------
  describe('extractEntities', () => {
    it('TC-QA-068: 从中文文本提取书名号《》内容', () => {
      const text = '根据《劳动合同法》第44条规定，以及《工资支付条例》的相关条款...';
      const entities = extractEntities(text);
      expect(entities).toContain('劳动合同法');
      expect(entities).toContain('工资支付条例');
    });

    it('TC-QA-068b: 提取引号内容', () => {
      const text = '本法所称"加班费"是指劳动者在法定工作时间外工作的报酬。';
      const entities = extractEntities(text);
      expect(entities).toContain('加班费');
    });

    it('TC-QA-068c: 空文本或 null 输入返回空数组', () => {
      expect(extractEntities('')).toEqual([]);
      expect(extractEntities(null)).toEqual([]);
      expect(extractEntities(undefined)).toEqual([]);
    });

    it('TC-QA-068d: 尊重 maxEntities 限制', () => {
      const text = '《法一》《法二》《法三》《法四》《法五》《法六》';
      const entities = extractEntities(text, 3);
      expect(entities.length).toBeLessThanOrEqual(3);
    });
  });

  // ----------------------------------------------------------
  // TC-QA-069: extractEntities 提取 Markdown 标题内容
  // ----------------------------------------------------------
  it('TC-QA-069: 提取 Markdown 标题内容作为实体', () => {
    const text = '## 加班费计算标准\n### 工作日加班\n### 休息日加班';
    const entities = extractEntities(text);
    expect(entities).toContain('加班费计算标准');
    expect(entities).toContain('工作日加班');
    expect(entities).toContain('休息日加班');
  });

  // ----------------------------------------------------------
  // TC-QA-070: generateFollowups 基于实体生成后续问题
  // ----------------------------------------------------------
  describe('generateFollowups', () => {
    it('TC-QA-070: 基于实体生成 2-3 个后续问题', () => {
      const entities = ['劳动合同法', '加班费'];
      const suggestions = generateFollowups(entities);
      expect(suggestions.length).toBeGreaterThanOrEqual(2);
      expect(suggestions.length).toBeLessThanOrEqual(3);
      // 每个建议应包含某个实体名
      const hasEntity = suggestions.some(s => s.includes('劳动合同法') || s.includes('加班费'));
      expect(hasEntity).toBe(true);
    });

    it('TC-QA-070b: 建议不重复', () => {
      const entities = ['劳动合同法', '加班费', '工资支付'];
      const suggestions = generateFollowups(entities);
      const unique = new Set(suggestions);
      expect(unique.size).toBe(suggestions.length);
    });
  });

  // ----------------------------------------------------------
  // TC-QA-071: 无实体时返回通用追问
  // ----------------------------------------------------------
  it('TC-QA-071: 无实体时返回通用追问', () => {
    const suggestions = generateFollowups([]);
    expect(suggestions.length).toBeGreaterThanOrEqual(2);
    expect(suggestions.length).toBeLessThanOrEqual(3);
    // 通用追问应是非空字符串
    suggestions.forEach(s => {
      expect(typeof s).toBe('string');
      expect(s.length).toBeGreaterThan(0);
    });
  });

  // ----------------------------------------------------------
  // TC-QA-072: renderFollowups 渲染建议卡片 + 关闭按钮
  // ----------------------------------------------------------
  describe('renderFollowups', () => {
    it('TC-QA-072: 渲染建议卡片和关闭按钮', () => {
      const blockEl = document.createElement('div');
      blockEl.className = 'msg-block msg-assistant';
      document.body.appendChild(blockEl);

      const suggestions = ['问题一？', '问题二？', '问题三？'];
      renderFollowups(blockEl, suggestions);

      const container = blockEl.querySelector('.followup-suggestions');
      expect(container).not.toBeNull();

      // 应有标题
      const header = container.querySelector('.followup-header');
      expect(header).not.toBeNull();

      // 应有关闭按钮
      const closeBtn = container.querySelector('.followup-close-btn');
      expect(closeBtn).not.toBeNull();

      // 应有 3 个建议卡片
      const cards = container.querySelectorAll('.followup-card');
      expect(cards.length).toBe(3);

      // 卡片文本应匹配
      expect(cards[0].textContent).toBe('问题一？');
    });

    it('TC-QA-072b: blockEl 为 null 时返回 null', () => {
      const result = renderFollowups(null, ['test']);
      expect(result).toBeNull();
    });

    it('TC-QA-072c: 空建议列表返回 null', () => {
      const blockEl = document.createElement('div');
      const result = renderFollowups(blockEl, []);
      expect(result).toBeNull();
    });
  });

  // ----------------------------------------------------------
  // TC-QA-073: 点击建议卡片触发 onPick 回调
  // ----------------------------------------------------------
  it('TC-QA-073: 点击建议卡片触发 onPick 回调', () => {
    const blockEl = document.createElement('div');
    blockEl.className = 'msg-block msg-assistant';
    document.body.appendChild(blockEl);

    let pickedText = null;
    const onPick = (text) => { pickedText = text; };

    const suggestions = ['加班费争议如何维权？', '试用期工资有何规定？'];
    renderFollowups(blockEl, suggestions, onPick);

    const card = blockEl.querySelector('.followup-card');
    expect(card).not.toBeNull();

    // 模拟点击
    card.click();

    expect(pickedText).toBe('加班费争议如何维权？');
  });

  // ----------------------------------------------------------
  // TC-QA-074: 关闭按钮移除建议容器
  // ----------------------------------------------------------
  it('TC-QA-074: 关闭按钮移除建议容器', () => {
    const blockEl = document.createElement('div');
    blockEl.className = 'msg-block msg-assistant';
    document.body.appendChild(blockEl);

    renderFollowups(blockEl, ['问题一', '问题二']);

    // 确认容器存在
    expect(blockEl.querySelector('.followup-suggestions')).not.toBeNull();

    // 点击关闭按钮
    const closeBtn = blockEl.querySelector('.followup-close-btn');
    closeBtn.click();

    // 容器应被移除
    expect(blockEl.querySelector('.followup-suggestions')).toBeNull();
  });

  // ----------------------------------------------------------
  // TC-QA-075: 最多 3 条建议
  // ----------------------------------------------------------
  it('TC-QA-075: 生成建议不超过 3 条', () => {
    const entities = ['实体A', '实体B', '实体C', '实体D', '实体E'];
    const suggestions = generateFollowups(entities);
    expect(suggestions.length).toBeLessThanOrEqual(3);
  });

  // ----------------------------------------------------------
  // TC-QA-075b: removeFollowups 移除已有容器
  // ----------------------------------------------------------
  it('TC-QA-075b: removeFollowups 清除已有建议容器', () => {
    const blockEl = document.createElement('div');
    blockEl.className = 'msg-block msg-assistant';
    document.body.appendChild(blockEl);

    renderFollowups(blockEl, ['问题一']);
    expect(blockEl.querySelector('.followup-suggestions')).not.toBeNull();

    removeFollowups(blockEl);
    expect(blockEl.querySelector('.followup-suggestions')).toBeNull();
  });

  // ----------------------------------------------------------
  // TC-QA-075c: renderFollowupSuggestions 一站式生成+渲染
  // ----------------------------------------------------------
  it('TC-QA-075c: renderFollowupSuggestions 从文本生成并渲染建议', () => {
    const blockEl = document.createElement('div');
    blockEl.className = 'msg-block msg-assistant';
    document.body.appendChild(blockEl);

    const answerText = '根据《劳动合同法》第44条规定，加班费按以下标准计算...';
    let picked = null;

    const container = renderFollowupSuggestions(
      blockEl,
      answerText,
      '请总结加班费规定',
      (text) => { picked = text; },
    );

    expect(container).not.toBeNull();

    // 应有至少 1 个建议卡片
    const cards = blockEl.querySelectorAll('.followup-card');
    expect(cards.length).toBeGreaterThanOrEqual(1);

    // 点击第一个卡片应触发回调
    cards[0].click();
    expect(picked).not.toBeNull();
    expect(typeof picked).toBe('string');
  });
});
