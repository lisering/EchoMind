/**
 * EchoMind 文档范围筛选单元测试 — doc-mention.js 模块（TC-QA-034~039）。
 *
 * 验证点（对应 AC-QA-011 文档范围筛选 @-syntax）：
 * 1. extractDocMentions 从文本中提取 @docname 引用
 * 2. extractDocMentions 无 @ 时返回空数组
 * 3. filterDocuments 按名称部分匹配过滤文档
 * 4. renderDocMentionPopup 渲染弹框含文档项
 * 5. 点击文档项触发 onSelect 回调
 * 6. getDocFilter 返回文档名数组供 chat IPC 使用
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  extractDocMentions,
  filterDocuments,
  renderDocMentionPopup,
  insertDocMention,
  getDocFilter,
} from '../../../ui/src/doc-mention.js';

describe('Doc Mention — doc-mention.js', () => {
  let container;
  let inputEl;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);

    inputEl = document.createElement('textarea');
    inputEl.value = '';
    document.body.appendChild(inputEl);
  });

  describe('extractDocMentions', () => {
    it('TC-QA-034: 从文本中提取 @docname 引用', () => {
      const mentions = extractDocMentions('搜索 @劳动合同法 中关于 @试用期 的规定');
      expect(mentions).toHaveLength(2);
      expect(mentions).toContain('劳动合同法');
      expect(mentions).toContain('试用期');
    });

    it('TC-QA-034b: 支持中文文档名含空格的情况', () => {
      // @ 后跟空格不应被视为引用
      const mentions = extractDocMentions('搜索 @ 劳动合同法');
      expect(mentions).toHaveLength(0);
    });

    it('TC-QA-035: 无 @ 时返回空数组', () => {
      expect(extractDocMentions('搜索劳动合同法')).toHaveLength(0);
      expect(extractDocMentions('')).toHaveLength(0);
    });

    it('TC-QA-035b: 去重相同文档引用', () => {
      const mentions = extractDocMentions('@劳动合同法 和 @劳动合同法 的比较');
      expect(mentions).toHaveLength(1);
      expect(mentions).toContain('劳动合同法');
    });
  });

  describe('filterDocuments', () => {
    const docs = [
      { id: '1', name: '劳动合同法.md' },
      { id: '2', name: '加班费案例.docx' },
      { id: '3', name: '工资支付条例.md' },
    ];

    it('TC-QA-036: 按名称部分匹配过滤', () => {
      const filtered = filterDocuments(docs, '劳动');
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe('劳动合同法.md');
    });

    it('TC-QA-036b: 大小写不敏感匹配', () => {
      const filtered = filterDocuments(docs, 'DOCX');
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe('加班费案例.docx');
    });

    it('TC-QA-036c: 空查询时返回全部', () => {
      const filtered = filterDocuments(docs, '');
      expect(filtered).toHaveLength(3);
    });

    it('TC-QA-036d: 无匹配时返回空数组', () => {
      const filtered = filterDocuments(docs, '不存在的文档');
      expect(filtered).toHaveLength(0);
    });
  });

  describe('renderDocMentionPopup', () => {
    const docs = [
      { id: '1', name: '劳动合同法.md' },
      { id: '2', name: '加班费案例.docx' },
      { id: '3', name: '工资支付条例.md' },
    ];

    it('TC-QA-037: 渲染弹框含文档项', () => {
      renderDocMentionPopup(container, docs);
      const items = container.querySelectorAll('.doc-mention-item');
      expect(items).toHaveLength(3);
    });

    it('TC-QA-037b: 每项显示文档名', () => {
      renderDocMentionPopup(container, docs);
      const first = container.querySelector('.doc-mention-item');
      expect(first.textContent).toContain('劳动合同法.md');
    });

    it('TC-QA-038: 点击文档项触发 onSelect 回调', () => {
      const spy = vi.fn();
      renderDocMentionPopup(container, docs, spy);
      const first = container.querySelector('.doc-mention-item');
      first.click();
      expect(spy).toHaveBeenCalledTimes(1);
      expect(spy).toHaveBeenCalledWith(expect.objectContaining({ name: '劳动合同法.md' }));
    });
  });

  describe('insertDocMention', () => {
    it('TC-QA-039: 在输入框光标位置插入 @docname', () => {
      inputEl.value = '搜索 关于加班费';
      inputEl.setSelectionRange(3, 3); // 光标在 "搜索 " 之后
      insertDocMention(inputEl, '劳动合同法.md');
      expect(inputEl.value).toBe('搜索 @劳动合同法.md 关于加班费');
    });

    it('TC-QA-039b: 插入后光标位于 @docname 之后', () => {
      inputEl.value = '搜索 关于加班费';
      inputEl.setSelectionRange(3, 3);
      insertDocMention(inputEl, '劳动合同法.md');
      // 光标应在插入文本（含尾部空格）之后
      const expectedPrefix = '搜索 @劳动合同法.md ';
      expect(inputEl.selectionStart).toBe(expectedPrefix.length);
    });
  });

  describe('getDocFilter', () => {
    it('TC-QA-039c: 返回文档名数组', () => {
      const mentions = ['劳动合同法.md', '加班费案例.docx'];
      const filter = getDocFilter(mentions);
      expect(filter).toEqual(mentions);
    });

    it('TC-QA-039d: 空数组时返回空数组', () => {
      const filter = getDocFilter([]);
      expect(filter).toEqual([]);
    });
  });
});
