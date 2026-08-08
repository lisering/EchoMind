// 真实 LLM 端到端测试（REQ-OBS-001 + RAG 全链路验证）。
//
// 与桥接测试不同，此 spec 不注入 tauri-stub，而是通过真实 Tauri IPC
// 连接运行中的 EchoMind 后端（需 `cargo tauri dev` 已启动）。
//
// ## 环境变量
//
// | 变量 | 说明 |
// |---|---|
// | `ECHOMIND_E2E_REAL_LLM` | 设为 `1` 启用真实 LLM E2E 测试 |
// | `ECHOMIND_LLM_API_KEY` | LLM API Key |
// | `ECHOMIND_LLM_BASE_URL` | OpenAI 兼容端点 |
// | `ECHOMIND_LLM_MODEL` | 模型名 |
// | `ECHOMIND_E2E_URL` | Tauri dev 服务器 URL（默认 `http://localhost:1420`） |
//
// ## 运行方式
//
// ```bash
// # 1. 启动 EchoMind dev 服务器
// cargo tauri dev &
//
// # 2. 运行真实 LLM E2E 测试
// ECHOMIND_E2E_REAL_LLM=1 \
// ECHOMIND_LLM_API_KEY=sk-xxx \
// ECHOMIND_LLM_BASE_URL=https://api.deepseek.com \
// ECHOMIND_LLM_MODEL=deepseek-chat \
// npx playwright test tests/real-llm.spec.ts
// ```

import { test, expect, type Page } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import fs from 'node:fs';

const specDir = path.dirname(fileURLToPath(import.meta.url));
const fixturePath = path.resolve(specDir, '../../tests/fixtures/sample.md');

const isRealLlmEnabled = process.env.ECHOMIND_E2E_REAL_LLM === '1';
const e2eUrl = process.env.ECHOMIND_E2E_URL || 'http://localhost:1420';

const apiKey = process.env.ECHOMIND_LLM_API_KEY || '';
const baseUrl = process.env.ECHOMIND_LLM_BASE_URL || '';
const model = process.env.ECHOMIND_LLM_MODEL || '';

test.describe('真实 LLM 端到端测试', () => {
  test.skip(!isRealLlmEnabled, '需要 ECHOMIND_E2E_REAL_LLM=1 环境变量');

  test.beforeEach(async ({ page }) => {
    // 不注入 tauri-stub，使用真实 Tauri IPC
    await page.goto(e2eUrl);
    // 等待应用加载
    await page.locator('#app').waitFor({ state: 'visible', timeout: 30000 });
  });

  test('E2E-REAL-LLM-001 RAG 全链路：导入→检索→流式生成', async ({ page }) => {
    test.skip(!apiKey || !baseUrl || !model, '需要 LLM API Key / Base URL / Model 环境变量');

    // 1. 配置 LLM 端点（通过设置面板）
    await configureLlm(page, apiKey, baseUrl, model);

    // 2. 导入测试文档
    await importDocument(page, fixturePath);

    // 3. 等待索引完成
    await waitForIndexing(page);

    // 4. 发送 RAG 查询
    const query = 'EchoMind 的核心特性有哪些？';
    await page.locator('#queryInput').fill(query);
    await page.locator('#sendBtn').click();

    // 5. 等待流式响应完成
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 120000 });

    // 6. 验证助手回复非空
    const assistantMessages = page.locator('.msg-assistant');
    const count = await assistantMessages.count();
    expect(count, '应至少有 1 条助手消息').toBeGreaterThan(0);

    const lastResponse = await assistantMessages.last().textContent();
    expect(lastResponse, '助手回复不应为空').not.toBeNull();
    expect(lastResponse.length, '助手回复长度应大于0').toBeGreaterThan(0);
    expect(
      lastResponse!.length,
      '助手回复应有一定长度'
    ).toBeGreaterThan(20);

    // 7. 验证引用来源显示
    const sources = page.locator('.chat-source, [data-source]');
    const sourceCount = await sources.count();
    expect(sourceCount, '应显示引用来源').toBeGreaterThan(0);
  });

  test('E2E-REAL-LLM-002 多轮对话上下文保持', async ({ page }) => {
    test.skip(!apiKey || !baseUrl || !model, '需要 LLM API Key / Base URL / Model 环境变量');

    await configureLlm(page, apiKey, baseUrl, model);
    await importDocument(page, fixturePath);
    await waitForIndexing(page);

    // 第一轮
    await page.locator('#queryInput').fill('EchoMind 的架构分几层？');
    await page.locator('#sendBtn').click();
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 120000 });

    // 第二轮（引用上下文）
    await page.locator('#queryInput').fill('每一层的职责是什么？');
    await page.locator('#sendBtn').click();
    await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 120000 });

    // 验证两条助手消息
    const assistantMessages = page.locator('.msg-assistant');
    const count = await assistantMessages.count();
    expect(count, '应至少有 2 条助手消息').toBeGreaterThanOrEqual(2);

    // 第二条回复应提及架构相关关键词
    const lastResponse = (await assistantMessages.last().textContent()) || '';
    const lower = lastResponse.toLowerCase();
    const hasArchKeywords =
      lower.includes('models') ||
      lower.includes('core') ||
      lower.includes('infra') ||
      lower.includes('tauri') ||
      lower.includes('契约') ||
      lower.includes('适配');
    expect(hasArchKeywords, '第二轮回复应提及架构各层').toBe(true);
  });

  test('E2E-REAL-LLM-003 日志系统验证（REQ-OBS-001）', async ({ page }) => {
    test.skip(!apiKey || !baseUrl || !model, '需要 LLM API Key / Base URL / Model 环境变量');

    // 验证日志级别 IPC 可用
    const logLevel = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_log_level')
    );
    expect(logLevel, '默认日志级别应为 info').toBe('info');

    // 切换到 DEBUG 级别
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_log_level', { level: 'debug' })
    );

    const newLevel = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_log_level')
    );
    expect(newLevel, '切换后日志级别应为 debug').toBe('debug');

    // 切换回 INFO
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_log_level', { level: 'info' })
    );

    // 导出日志
    const logs = await page.evaluate(() =>
      window.__TAURI__.core.invoke('export_logs', { tailLines: 100 })
    );
    expect(typeof logs, '导出日志应为字符串').toBe('string');
  });

  test('E2E-REAL-LLM-004 诊断信息导出（REQ-OBS-002）', async ({ page }) => {
    // 导出诊断信息
    const diagnostics = await page.evaluate(async () => {
      const result = await window.__TAURI__.core.invoke('export_diagnostics');
      return JSON.parse(result);
    });

    // 验证诊断信息结构
    expect(diagnostics.app_version, '应包含应用版本').not.toBeNull();
    expect(diagnostics.app_version.length, '应用版本应非空').toBeGreaterThan(0);
    expect(diagnostics.system, '应包含系统信息').not.toBeNull();
    expect(diagnostics.system.os, '应包含操作系统').not.toBeNull();
    expect(diagnostics.system.os.length, '操作系统应非空').toBeGreaterThan(0);
    expect(
      diagnostics.system.cpu_count,
      'CPU 核心数应大于 0'
    ).toBeGreaterThan(0);
    expect(diagnostics.knowledge_base, '应包含知识库信息').not.toBeNull();
    expect(
      diagnostics.knowledge_base.embedding_dimension,
      '嵌入维度应为 384'
    ).toBe(384);

    // 验证不含 API Key 明文
    const jsonStr = JSON.stringify(diagnostics);
    expect(
      jsonStr,
      '诊断信息不得包含 API Key 前缀'
    ).not.toContain('sk-');
  });
});

/**
 * 通过设置面板配置 LLM 端点。
 */
async function configureLlm(
  page: Page,
  apiKey: string,
  baseUrl: string,
  model: string
) {
  // 打开设置面板
  await page.locator('#settingsBtn').click();
  await page.locator('#settingsModal').waitFor({ state: 'visible' });

  // 填写 LLM 配置
  await page.locator('#cfgApiKey').fill(apiKey);
  await page.locator('#cfgBaseUrl').fill(baseUrl);
  await page.locator('#cfgModel').fill(model);
  await page.locator('#cfgSave').click();

  // 等待设置面板关闭
  await page.locator('#settingsModal').waitFor({ state: 'hidden', timeout: 10000 });
  await page.waitForTimeout(500);
}

/**
 * 导入文档并等待列表更新。
 */
async function importDocument(page: Page, filePath: string) {
  // 通过 IPC 导入文件
  await page.evaluate((p) => {
    return window.__TAURI__.core.invoke('import_files', { paths: [p] });
  }, filePath);

  // 等待文档列表更新
  await page.locator('#docList [data-doc-name]').first().waitFor({
    state: 'attached',
    timeout: 10000,
  });
}

/**
 * 等待文档索引完成（状态变为 Indexed）。
 */
async function waitForIndexing(page: Page) {
  // 轮询文档状态，直到显示 "已索引" 或类似状态
  await page.waitForFunction(
    () => {
      const badges = document.querySelectorAll('[data-doc-status]');
      if (badges.length === 0) return false;
      for (const badge of badges) {
        const text = badge.textContent || '';
        if (text.includes('索引中') || text.includes('Processing')) {
          return false;
        }
      }
      return true;
    },
    { timeout: 300000 } // 5 分钟超时（首次嵌入模型下载）
  );
}
