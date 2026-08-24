/**
 * EchoMind Markdown 渲染工具单元测试 — markdown.js 模块。
 *
 * 验证点：
 * 1. normalizeBulletGlyphs 将字面圆点行（• ● ◦ ▪）转换为标准列表项
 * 2. 代码块（``` / ~~~）内的圆点行不转换
 * 3. 行内圆点（非行首）不转换
 * 4. 嵌套缩进圆点行转换为对应缩进的列表项
 * 5. 无圆点文本原样返回
 */

import { describe, it, expect } from 'vitest';
import { normalizeBulletGlyphs } from '../../../ui/src/markdown.js';

describe('Markdown — normalizeBulletGlyphs', () => {
  it('将行首字面圆点（•）转换为标准列表项', () => {
    const input = '要点如下：\n• 第一点内容\n• 第二点内容';
    const out = normalizeBulletGlyphs(input);
    expect(out).toBe('要点如下：\n- 第一点内容\n- 第二点内容');
  });

  it('支持多种圆点符号（● ◦ ▪ ‣ ⁃）', () => {
    const input = '● 实心点\n◦ 空心点\n▪ 方块点';
    const out = normalizeBulletGlyphs(input);
    expect(out).toBe('- 实心点\n- 空心点\n- 方块点');
  });

  it('圆点后无空格（如 •内容）不转换，避免误伤普通文本', () => {
    const input = '价格 •100元\n正常文本';
    const out = normalizeBulletGlyphs(input);
    expect(out).toBe('价格 •100元\n正常文本');
  });

  it('代码块内的圆点行不转换', () => {
    const input = '示例：\n```text\n• 这是代码\n- 这是原有列表\n```\n• 这是正文列表';
    const out = normalizeBulletGlyphs(input);
    expect(out).toBe('示例：\n```text\n• 这是代码\n- 这是原有列表\n```\n- 这是正文列表');
  });

  it('带缩进的圆点行保留缩进（嵌套列表），前导空白保留', () => {
    const input = '• 一级\n  • 二级';
    const out = normalizeBulletGlyphs(input);
    expect(out).toBe('- 一级\n  - 二级');
  });

  it('无圆点的文本原样返回', () => {
    const input = '普通段落\n- 已有列表\n1. 有序列表';
    expect(normalizeBulletGlyphs(input)).toBe(input);
  });

  it('流式增量输入（不完整行）不崩溃', () => {
    const input = '• 部分内容';
    const out = normalizeBulletGlyphs(input);
    expect(out).toBe('- 部分内容');
  });
});
