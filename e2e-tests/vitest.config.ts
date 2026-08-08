import { defineConfig } from 'vitest/config';

/**
 * Vitest 配置 — 仅运行 tests/unit/ 目录下的前端单元测试。
 * Playwright E2E 测试由 playwright.config.ts 单独管理。
 *
 * 使用 jsdom 环境以提供 document / navigator / fetch / localStorage 等 Web API，
 * 让 i18n.js / toast.js 等含 DOM 依赖的模块也能直接测试。
 */
export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['tests/unit/**/*.test.js'],
    exclude: ['tests/**/*.spec.ts', 'specs/**/*.mjs', 'node_modules/**'],
  },
});
