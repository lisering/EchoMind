/**
 * EchoMind 快捷指令面板单元测试 — slash-commands.js 模块（TC-QA-027~033）。
 *
 * 验证点（对应 AC-QA-010 快捷指令）：
 * 1. SLASH_COMMANDS 包含 6 个指令定义
 * 2. filterSlashCommands 按前缀过滤指令
 * 3. filterSlashCommands 输入 '/' 时返回全部
 * 4. renderSlashCommandPanel 渲染面板含指令项
 * 5. 点击指令项触发 onSelect 回调
 * 6. navigateSlashCommand 更新选中索引
 * 7. applySlashCommand 将指令文本插入输入框
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  SLASH_COMMANDS,
  filterSlashCommands,
  renderSlashCommandPanel,
  navigateSlashCommand,
  getSelectedSlashCommand,
  applySlashCommand,
  resetSlashSelection,
} from '../../../ui/src/slash-commands.js';

describe('Slash Commands — slash-commands.js', () => {
  let container;
  let inputEl;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);

    inputEl = document.createElement('textarea');
    inputEl.value = '/';
    document.body.appendChild(inputEl);

    resetSlashSelection();
  });

  describe('SLASH_COMMANDS', () => {
    it('TC-QA-027: 包含 8 个指令定义', () => {
      expect(SLASH_COMMANDS).toHaveLength(8);
    });

    it('TC-QA-027b: 每个指令含 name/label/icon 属性', () => {
      for (const cmd of SLASH_COMMANDS) {
        expect(cmd.name).toBeTruthy();
        expect(cmd.label).toBeTruthy();
        expect(cmd.icon).toBeTruthy();
      }
    });

    it('TC-QA-027c: 包含 summary/compare/extract/translate/timeline/mindmap/web', () => {
      const names = SLASH_COMMANDS.map((c) => c.name);
      expect(names).toContain('summary');
      expect(names).toContain('compare');
      expect(names).toContain('extract');
      expect(names).toContain('translate');
      expect(names).toContain('timeline');
      expect(names).toContain('mindmap');
      expect(names).toContain('web');
      expect(names).toContain('agent');
    });
  });

  describe('filterSlashCommands', () => {
    it('TC-QA-028: 按前缀过滤指令', () => {
      const filtered = filterSlashCommands('/sum');
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe('summary');
    });

    it('TC-QA-028b: 大小写不敏感过滤', () => {
      const filtered = filterSlashCommands('/COM');
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe('compare');
    });

    it('TC-QA-029: 输入仅 / 时返回全部指令', () => {
      const filtered = filterSlashCommands('/');
      expect(filtered).toHaveLength(8);
    });

    it('TC-QA-029b: 无匹配时返回空数组', () => {
      const filtered = filterSlashCommands('/xyz');
      expect(filtered).toHaveLength(0);
    });

    it('TC-QA-029c: 输入不以 / 开头时返回空数组', () => {
      const filtered = filterSlashCommands('hello');
      expect(filtered).toHaveLength(0);
    });
  });

  describe('renderSlashCommandPanel', () => {
    it('TC-QA-030: 渲染面板含指令项', () => {
      const filtered = filterSlashCommands('/');
      renderSlashCommandPanel(container, filtered);
      const items = container.querySelectorAll('.slash-command-item');
      expect(items).toHaveLength(8);
    });

    it('TC-QA-030b: 首项默认选中', () => {
      const filtered = filterSlashCommands('/');
      renderSlashCommandPanel(container, filtered);
      const first = container.querySelector('.slash-command-item');
      expect(first.classList.contains('slash-command-selected')).toBe(true);
    });

    it('TC-QA-031: 点击指令项触发 onSelect 回调', () => {
      const spy = vi.fn();
      const filtered = filterSlashCommands('/');
      renderSlashCommandPanel(container, filtered, spy);
      const first = container.querySelector('.slash-command-item');
      first.click();
      expect(spy).toHaveBeenCalledTimes(1);
      expect(spy).toHaveBeenCalledWith(expect.objectContaining({ name: 'summary' }));
    });
  });

  describe('navigateSlashCommand', () => {
    it('TC-QA-032: 向下导航更新选中索引', () => {
      const filtered = filterSlashCommands('/');
      renderSlashCommandPanel(container, filtered);
      navigateSlashCommand(filtered, 'down');
      const selected = container.querySelector('.slash-command-selected');
      const items = container.querySelectorAll('.slash-command-item');
      expect(items[1]).toBe(selected);
    });

    it('TC-QA-032b: 向上导航在首项时回绕到末项', () => {
      const filtered = filterSlashCommands('/');
      renderSlashCommandPanel(container, filtered);
      navigateSlashCommand(filtered, 'up');
      const selected = container.querySelector('.slash-command-selected');
      const items = container.querySelectorAll('.slash-command-item');
      expect(items[items.length - 1]).toBe(selected);
    });

    it('TC-QA-032c: 向下导航在末项时回绕到首项', () => {
      const filtered = filterSlashCommands('/');
      renderSlashCommandPanel(container, filtered);
      // Navigate to last item
      for (let i = 0; i < filtered.length - 1; i++) {
        navigateSlashCommand(filtered, 'down');
      }
      // One more down should wrap to first
      navigateSlashCommand(filtered, 'down');
      const selected = container.querySelector('.slash-command-selected');
      const items = container.querySelectorAll('.slash-command-item');
      expect(items[0]).toBe(selected);
    });

    it('TC-QA-032d: Home 跳转到首项（P2-4）', () => {
      const filtered = filterSlashCommands('/');
      renderSlashCommandPanel(container, filtered);
      // 先导航到中间项
      navigateSlashCommand(filtered, 'down');
      navigateSlashCommand(filtered, 'down');
      // Home 跳回首项
      navigateSlashCommand(filtered, 'home');
      const selected = container.querySelector('.slash-command-selected');
      const items = container.querySelectorAll('.slash-command-item');
      expect(items[0]).toBe(selected);
    });

    it('TC-QA-032e: End 跳转到末项（P2-4）', () => {
      const filtered = filterSlashCommands('/');
      renderSlashCommandPanel(container, filtered);
      // End 跳到末项
      navigateSlashCommand(filtered, 'end');
      const selected = container.querySelector('.slash-command-selected');
      const items = container.querySelectorAll('.slash-command-item');
      expect(items[items.length - 1]).toBe(selected);
    });

    it('TC-QA-032f: 单项列表 Home/End 仍然生效（P2-4）', () => {
      const filtered = filterSlashCommands('/sum');
      renderSlashCommandPanel(container, filtered);
      // 单项列表，Home/End 应该仍然工作（不 return）
      navigateSlashCommand(filtered, 'home');
      const selected = container.querySelector('.slash-command-selected');
      expect(selected).not.toBeNull();
      navigateSlashCommand(filtered, 'end');
      const selected2 = container.querySelector('.slash-command-selected');
      expect(selected2).not.toBeNull();
    });
  });

  describe('getSelectedSlashCommand', () => {
    it('TC-QA-032d: 初始选中首项', () => {
      const filtered = filterSlashCommands('/');
      renderSlashCommandPanel(container, filtered);
      const selected = getSelectedSlashCommand(filtered);
      expect(selected).not.toBeNull();
      expect(selected.name).toBe('summary');
    });
  });

  describe('applySlashCommand', () => {
    it('TC-QA-033: 将指令文本插入输入框', () => {
      const cmd = SLASH_COMMANDS[0]; // /summary
      applySlashCommand(cmd, inputEl);
      expect(inputEl.value).toBe('/summary '); // 原始 / 被替换为 /summary + 空格
    });

    it('TC-QA-033b: 指令后自动添加空格便于用户继续输入', () => {
      const cmd = SLASH_COMMANDS[0]; // /summary
      applySlashCommand(cmd, inputEl);
      expect(inputEl.value).toMatch(/\/summary\s/);
    });
  });
});
