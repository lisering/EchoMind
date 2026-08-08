/**
 * EchoMind 命令面板单元测试 — command-palette.js 模块。
 *
 * 验证点：
 * 1. filterCommands 模糊搜索过滤正确
 * 2. highlightMatch 高亮匹配字符
 * 3. 空查询返回全部命令
 * 4. 无匹配返回空数组
 * 5. 分组名搜索也能匹配
 */

import { describe, it, expect } from 'vitest';
import { filterCommands, highlightMatch } from '../../../ui/src/command-palette.js';

// 模拟命令清单
const MOCK_COMMANDS = [
  { group: '导航', icon: '💬', label: '新建会话', shortcut: '⌘N', action: () => {} },
  { group: '导航', icon: '＋', label: '导入文件', shortcut: '⌘O', action: () => {} },
  { group: '导航', icon: '⚙', label: '打开设置', shortcut: '⌘,', action: () => {} },
  { group: '对话', icon: '■', label: '停止生成', shortcut: 'Esc', action: () => {} },
  { group: '配置', icon: '👁', label: '切换 VLM 增强', action: () => {} },
  { group: '授权', icon: '◈', label: '激活 Pro', action: () => {} },
];

describe('Command Palette — command-palette.js', () => {

  describe('filterCommands', () => {
    it('空查询返回全部命令', () => {
      const result = filterCommands('', MOCK_COMMANDS);
      expect(result).toHaveLength(6);
    });

    it('空格查询返回全部命令', () => {
      const result = filterCommands('   ', MOCK_COMMANDS);
      expect(result).toHaveLength(6);
    });

    it('按标签搜索 — 中文匹配', () => {
      const result = filterCommands('新建', MOCK_COMMANDS);
      expect(result).toHaveLength(1);
      expect(result[0].label).toBe('新建会话');
    });

    it('按分组名搜索 — "导航" 匹配 3 条', () => {
      const result = filterCommands('导航', MOCK_COMMANDS);
      expect(result).toHaveLength(3);
    });

    it('按分组名搜索 — "对话" 匹配 1 条', () => {
      const result = filterCommands('对话', MOCK_COMMANDS);
      expect(result).toHaveLength(1);
      expect(result[0].label).toBe('停止生成');
    });

    it('无匹配返回空数组', () => {
      const result = filterCommands('不存在的命令', MOCK_COMMANDS);
      expect(result).toHaveLength(0);
    });

    it('大小写不敏感', () => {
      const result = filterCommands('PRO', MOCK_COMMANDS);
      expect(result).toHaveLength(1);
      expect(result[0].label).toBe('激活 Pro');
    });

    it('部分匹配 — "设置" 匹配 "打开设置"', () => {
      const result = filterCommands('设置', MOCK_COMMANDS);
      expect(result).toHaveLength(1);
      expect(result[0].label).toBe('打开设置');
    });
  });

  describe('highlightMatch', () => {
    it('无查询时原样返回', () => {
      expect(highlightMatch('新建会话', '')).toBe('新建会话');
    });

    it('匹配字符被高亮 span 包裹', () => {
      const result = highlightMatch('新建会话', '新建');
      expect(result).toContain('<span class="text-accent font-medium">新建</span>');
      expect(result).toContain('会话');
    });

    it('无匹配时原样返回', () => {
      expect(highlightMatch('导入文件', '不匹配')).toBe('导入文件');
    });

    it('大小写不敏感高亮', () => {
      const result = highlightMatch('激活 Pro', 'pro');
      expect(result).toContain('<span class="text-accent font-medium">Pro</span>');
    });
  });
});
