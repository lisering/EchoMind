/**
 * EchoMind 工具函数单元测试 — utils.js 模块。
 *
 * 验证点：
 * 1. sanitizeError 脱敏 API Key 和用户路径
 * 2. displayDocName 剥离 MD5 哈希前缀
 * 3. docStatusOf 安全提取状态字符串
 * 4. formatBytes 格式化字节大小
 * 5. basename / extname 路径解析
 * 6. PRESETS 配置完整性
 */

import { describe, it, expect } from 'vitest';
import {
  sanitizeError,
  displayDocName,
  docStatusOf,
  formatBytes,
  basename,
  extname,
  DOC_STATUS_STYLE,
  PRESETS,
  WORKSPACE,
} from '../../../ui/src/utils.js';

describe('Utils — utils.js', () => {

  describe('sanitizeError', () => {
    it('过滤 sk- 格式 API Key', () => {
      const result = sanitizeError('API error with sk-abcd1234efgh5678 key');
      expect(result).not.toContain('abcd1234efgh5678');
      expect(result).toContain('sk-****');
    });

    it('过滤 Unix 用户路径', () => {
      const result = sanitizeError('File at /Users/john/documents/test.md not found');
      expect(result).not.toContain('john');
      expect(result).toContain('/Users/****/');
    });

    it('过滤 Windows 用户路径', () => {
      const result = sanitizeError('Error in \\Users\\admin\\data\\file.txt');
      expect(result).not.toContain('admin');
      expect(result).toContain('\\Users\\****\\');
    });

    it('处理 Error 对象', () => {
      const err = new Error('Something sk-1234567890abcdef went wrong');
      const result = sanitizeError(err);
      expect(result).toContain('sk-****');
    });

    it('无敏感信息时原样返回', () => {
      expect(sanitizeError('普通错误消息')).toBe('普通错误消息');
    });
  });

  describe('displayDocName', () => {
    it('剥离 32 位 MD5 哈希前缀', () => {
      const path = '/data/documents/abcdef0123456789abcdef0123456789-report.md';
      expect(displayDocName(path)).toBe('report.md');
    });

    it('无哈希前缀时原样返回文件名', () => {
      const path = '/data/documents/simple.txt';
      expect(displayDocName(path)).toBe('simple.txt');
    });

    it('处理短文件名（不误切）', () => {
      const path = '/data/documents/ab.txt';
      expect(displayDocName(path)).toBe('ab.txt');
    });
  });

  describe('docStatusOf', () => {
    it('返回字符串状态', () => {
      expect(docStatusOf({ status: 'Indexed' })).toBe('Indexed');
    });

    it('非字符串状态返回 Failed', () => {
      expect(docStatusOf({ status: 42 })).toBe('Failed');
    });

    it('undefined 状态返回 Failed', () => {
      expect(docStatusOf({})).toBe('Failed');
    });
  });

  describe('formatBytes', () => {
    it('小于 1KB 显示 B', () => {
      expect(formatBytes(512)).toBe('512 B');
    });

    it('1KB~1MB 显示 KB', () => {
      expect(formatBytes(2048)).toBe('2.0 KB');
    });

    it('大于 1MB 显示 MB', () => {
      expect(formatBytes(31457280)).toBe('30.0 MB');
    });

    it('0 字节', () => {
      expect(formatBytes(0)).toBe('0 B');
    });
  });

  describe('basename', () => {
    it('提取 Unix 路径文件名', () => {
      expect(basename('/path/to/file.md')).toBe('file.md');
    });

    it('提取 Windows 路径文件名（Tauri 统一用 /）', () => {
      expect(basename('C:/Users/test/doc.txt')).toBe('doc.txt');
    });

    it('仅文件名时原样返回', () => {
      expect(basename('readme.md')).toBe('readme.md');
    });
  });

  describe('extname', () => {
    it('提取 .md 扩展名', () => {
      expect(extname('/path/to/file.md')).toBe('md');
    });

    it('提取 .PDF 扩展名转小写', () => {
      expect(extname('report.PDF')).toBe('pdf');
    });

    it('无扩展名返回空字符串', () => {
      expect(extname('Makefile')).toBe('');
    });
  });

  describe('DOC_STATUS_STYLE', () => {
    it('包含 4 种状态', () => {
      expect(Object.keys(DOC_STATUS_STYLE)).toHaveLength(4);
      expect(DOC_STATUS_STYLE.Pending).toBeDefined();
      expect(DOC_STATUS_STYLE.Processing).toBeDefined();
      expect(DOC_STATUS_STYLE.Indexed).toBeDefined();
      expect(DOC_STATUS_STYLE.Failed).toBeDefined();
    });

    it('每种状态有标签和样式', () => {
      for (const [key, [label, style]] of Object.entries(DOC_STATUS_STYLE)) {
        expect(label).toBeTruthy();
        expect(style).toBeTruthy();
      }
    });
  });

  describe('PRESETS', () => {
    it('包含 10 个预设', () => {
      expect(Object.keys(PRESETS)).toHaveLength(10);
      expect(PRESETS.deepseek).toBeDefined();
      expect(PRESETS.openai).toBeDefined();
      expect(PRESETS.qwen).toBeDefined();
      expect(PRESETS.kimi).toBeDefined();
      expect(PRESETS.glm).toBeDefined();
      expect(PRESETS.minimax).toBeDefined();
      expect(PRESETS.mistral).toBeDefined();
      expect(PRESETS.grok).toBeDefined();
      expect(PRESETS.ollama).toBeDefined();
      expect(PRESETS.custom).toBeDefined();
    });

    it('每个预设有必要字段', () => {
      for (const [key, p] of Object.entries(PRESETS)) {
        expect(p.label).toBeTruthy();
        expect(typeof p.base_url).toBe('string');
        expect(typeof p.model).toBe('string');
        expect(typeof p.keyUrl).toBe('string');
        expect(typeof p.needKey).toBe('boolean');
        expect(p.descKey).toBeTruthy();
      }
    });

    it('Ollama 不需要 API Key', () => {
      expect(PRESETS.ollama.needKey).toBe(false);
    });

    it('DeepSeek 需要 API Key', () => {
      expect(PRESETS.deepseek.needKey).toBe(true);
    });
  });

  describe('WORKSPACE', () => {
    it('默认工作空间为 "default"', () => {
      expect(WORKSPACE).toBe('default');
    });
  });
});
