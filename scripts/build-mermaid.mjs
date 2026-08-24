#!/usr/bin/env node

/**
 * Mermaid 自定义打包脚本 — 使用 esbuild 从 npm mermaid v11 ESM 源码打包。
 *
 * 优化原理：
 * 1. esbuild tree-shaking 移除未使用的内部工具函数
 * 2. esbuild minify 比 mermaid 官方 UMD 构建更激进
 * 3. 移除 UMD wrapper 开销
 * 4. 单文件输出（无 chunk 动态 import），兼容 file:// 和 Tauri WebView
 *
 * 对比：
 * - 原方案：vendor/mermaid.min.js（UMD 预构建，3.4MB）
 * - 新方案：vendor/mermaid-custom.min.js（esbuild 打包，预期 ~2.5-3MB）
 *
 * 用法：node scripts/build-mermaid.mjs
 * 输出：ui/vendor/mermaid-custom.min.js
 */

import esbuild from 'esbuild';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const entryPoint = resolve(__dirname, 'mermaid-entry.js');
const outFile = resolve(__dirname, '../ui/vendor/mermaid-custom.min.js');

console.log('🔨 Mermaid 自定义打包：esbuild + tree-shaking + minify...');

try {
  const result = await esbuild.build({
    entryPoints: [entryPoint],
    bundle: true,
    format: 'iife',
    target: 'es2022',
    minify: true,
    sourcemap: false,
    write: false,
    logLevel: 'warning',
    legalComments: 'none',
    // mermaid 内部使用 dynamic import() 加载图表定义
    // IIFE 格式下 esbuild 会将其转为同步加载（内联到单文件）
    splitting: false,
    // 树摇优化
    treeShaking: true,
  });

  const code = result.outputFiles[0].text;
  const { writeFileSync, statSync } = await import('node:fs');
  writeFileSync(outFile, code, 'utf-8');

  const sizeKB = (statSync(outFile).size / 1024).toFixed(0);
  console.log(`✓ Mermaid 自定义打包完成: ${outFile}`);
  console.log(`  体积: ${sizeKB}KB`);

  // 对比原 UMD 体积
  try {
    const oldSize = statSync(resolve(__dirname, '../ui/vendor/mermaid.min.js')).size;
    const oldKB = (oldSize / 1024).toFixed(0);
    const reduction = ((1 - statSync(outFile).size / oldSize) * 100).toFixed(1);
    console.log(`  原始 UMD: ${oldKB}KB → 减少 ${reduction}%`);
  } catch {
    // 原文件不存在时跳过对比
  }
} catch (err) {
  console.error('✗ Mermaid 打包失败:', err.message);
  process.exit(1);
}
