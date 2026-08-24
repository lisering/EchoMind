// E2E Agentic RAG 多步推理边界场景（REQ-RAG-022）：
// E2E-AGENT-001: Agentic 模式——默认关闭
// E2E-AGENT-002: Agentic 模式——启用后设置持久化
// E2E-AGENT-003: Agentic 模式——agent_step 事件推送
// E2E-AGENT-004: Agentic 模式——多步检索 Thought/Action/Observation
// E2E-AGENT-005: Agentic 模式——最终答案以 chat_token 流式输出
// E2E-AGENT-006: Agentic 模式——最大迭代次数限制
// E2E-AGENT-007: Agentic 模式——解析失败降级为标准 RAG
// E2E-AGENT-008: Agentic 模式——取消保留已生成内容
// E2E-AGENT-009: Agentic 模式——引用来源聚合
// E2E-AGENT-010: Agentic 模式——标准 RAG 共存
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';

test.describe('E2E-AGENT Agentic RAG 多步推理边界场景（REQ-RAG-022）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);

    // 导入文档
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/rust-guide.md'] })
    );
    await page.waitForTimeout(300);
  });

  // ─── Agentic 模式开关 ───

  test('E2E-AGENT-001 Agentic 模式——默认关闭', async ({ page }) => {
    // 检查 mock state 中的 agentEnabled 默认为 false
    const enabled = await page.evaluate(() => window.__mock.state.agentEnabled);
    expect(enabled).toBe(false);
  });

  test('E2E-AGENT-002 Agentic 模式——启用后持久化', async ({ page }) => {
    // 启用 Agentic 模式
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.agent_enabled', value: String(true) })
    );

    // 验证 mock state 已更新
    const enabled = await page.evaluate(() => window.__mock.state.agentEnabled);
    expect(enabled).toBe(true);

    // 关闭
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.agent_enabled', value: String(false) })
    );

    const enabledAfter = await page.evaluate(() => window.__mock.state.agentEnabled);
    expect(enabledAfter).toBe(false);
  });

  // ─── agent_step 事件 ───

  test('E2E-AGENT-003 Agentic 模式——agent_step 事件推送', async ({ page }) => {
    // 启用 Agentic 模式
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.agent_enabled', value: String(true) })
    );

    // 监听 agent_step 事件
    let stepReceived = false;
    await page.evaluate(() => {
      window.__state.listeners['agent_step'] = window.__state.listeners['agent_step'] || [];
      window.__state.listeners['agent_step'].push(() => {
        window.__agentStepReceived = true;
      });
    });

    // 发送消息
    await page.locator('#queryInput').fill('复杂查询：Rust 的所有权机制是什么？');
    await page.locator('#sendBtn').click();
    await page.waitForTimeout(3000);

    // 应收到 agent_step 事件
    const received = await page.evaluate(() => window.__agentStepReceived);
    expect(received).toBe(true);
  });

  // ─── 多步检索 ───

  test('E2E-AGENT-004 Agentic 模式——多步检索 Thought/Action/Observation', async ({ page }) => {
    // 启用 Agentic 模式
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.agent_enabled', value: String(true) })
    );

    // 监听 agent_step 事件，记录所有步骤
    await page.evaluate(() => {
      window.__state.listeners['agent_step'] = window.__state.listeners['agent_step'] || [];
      window.__agentSteps = [];
      window.__state.listeners['agent_step'].push((event) => {
        window.__agentSteps.push(event.payload);
      });
    });

    await page.locator('#queryInput').fill('多步查询');
    await page.locator('#sendBtn').click();
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 15000 });

    // 验证收到了 agent_step 事件
    const steps = await page.evaluate(() => window.__agentSteps || []);
    expect(steps.length, '应收到至少 1 个 agent_step 事件').toBeGreaterThan(0);

    // 验证步骤类型包含 Thought 和 Action（多步检索的核心环节）
    const stepTypes = steps.map((s: { step_type: string }) => s.step_type);
    expect(stepTypes, '应包含 Thought 步骤').toContain('thought');
    expect(stepTypes, '应包含 Action 步骤').toContain('action');

    // 验证应用不崩溃
    await expect(page.locator('#app')).toBeVisible();
  });

  // ─── 最终答案流式输出 ───

  test('E2E-AGENT-005 Agentic 模式——最终答案以 chat_token 流式输出', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.agent_enabled', value: String(true) })
    );

    // 监听 chat_token 事件
    let tokenReceived = false;
    await page.evaluate(() => {
      window.__state.listeners['chat_token'] = window.__state.listeners['chat_token'] || [];
      window.__state.listeners['chat_token'].push(() => {
        window.__tokenReceived = true;
      });
    });

    await page.locator('#queryInput').fill('流式测试');
    await page.locator('#sendBtn').click();
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 15000 });

    // 应收到 token 事件
    const received = await page.evaluate(() => window.__tokenReceived);
    expect(received).toBe(true);
  });

  // ─── 最大迭代次数 ───

  test('E2E-AGENT-006 Agentic 模式——最大迭代次数限制', async ({ page }) => {
    // 启用 Agentic 模式
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.agent_enabled', value: String(true) })
    );

    // 监听 agent_step 事件以计数迭代轮数
    await page.evaluate(() => {
      window.__state.listeners['agent_step'] = window.__state.listeners['agent_step'] || [];
      window.__agentStepCount = 0;
      window.__state.listeners['agent_step'].push(() => {
        window.__agentStepCount++;
      });
    });

    // 发送复杂查询
    await page.locator('#queryInput').fill('非常复杂的查询需要多步推理');
    await page.locator('#sendBtn').click();
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 30000 });

    // 应最终完成（不会无限循环）
    await expect(page.locator('#sendBtn')).toBeVisible({ timeout: 5000 });

    // 验证迭代步数有限（不会无限循环）
    const stepCount = await page.evaluate(() => window.__agentStepCount);
    expect(stepCount, 'Agent 步数应 > 0 且有限').toBeGreaterThan(0);
    expect(stepCount, 'Agent 步数应 ≤ 20（不会无限循环）').toBeLessThanOrEqual(20);
  });

  // ─── 解析失败降级 ───

  test('E2E-AGENT-007 Agentic 模式——解析失败降级为标准 RAG', async ({ page }) => {
    // 启用 Agentic 模式
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.agent_enabled', value: String(true) })
    );

    // 设置非预期格式的 LLM 响应
    await page.evaluate(() => window.__mock.setCustomTokens(['这不是 ReAct 格式的响应，而是普通文本。']));

    // 监听 chat_token 事件以验证降级输出
    await page.evaluate(() => {
      window.__state.listeners['chat_token'] = window.__state.listeners['chat_token'] || [];
      window.__degradedTokens = [];
      window.__state.listeners['chat_token'].push((event) => {
        window.__degradedTokens.push(event.payload);
      });
    });

    await page.locator('#queryInput').fill('解析测试');
    await page.locator('#sendBtn').click();
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 15000 });

    // 应优雅降级，不崩溃
    await expect(page.locator('#app')).toBeVisible();

    // 验证降级输出：应通过 chat_token 事件产生了内容
    const tokens = await page.evaluate(() => window.__degradedTokens || []);
    expect(tokens.length, '降级后应仍有 token 输出').toBeGreaterThan(0);

    // 聊天区应有内容
    const chatContent = await page.locator('#chatArea').innerText();
    expect(chatContent.length, '聊天区应有降级输出内容').toBeGreaterThan(0);
  });

  // ─── 取消 ───

  test('E2E-AGENT-008 Agentic 模式——取消保留已生成内容', async ({ page }) => {
    // 启用 Agentic 模式
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.agent_enabled', value: String(true) })
    );

    await page.locator('#queryInput').fill('取消测试');
    await page.locator('#sendBtn').click();

    // 点击停止（发送/停止合二为一：流式态 sendBtn 即停止按钮）
    // 注意：mock 流式受 E2E_SPEED 倍率加速，不能用固定 sleep；轮询等待 stop-mode 出现
    const sendBtn = page.locator('#sendBtn');
    await expect(sendBtn, '流式输出中 sendBtn 应处于 stop-mode').toHaveClass(/stop-mode/, { timeout: 5000 });
    await sendBtn.click();
    await page.waitForTimeout(500);

    // 应恢复空闲态
    await expect(page.locator('#sendBtn'), '取消后 sendBtn 应恢复可见').toBeVisible({ timeout: 5000 });

    // 聊天区应有内容（已生成的部分应保留）
    const chatContent = await page.locator('#chatArea').innerText();
    expect(chatContent.length, '取消后聊天区应保留已生成内容').toBeGreaterThan(0);
  });

  // ─── 引用来源聚合 ───

  test('E2E-AGENT-009 Agentic 模式——引用来源聚合', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.agent_enabled', value: String(true) })
    );

    await page.locator('#queryInput').fill('来源聚合测试');
    await page.locator('#sendBtn').click();
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 15000 });

    // 引用来源区应存在
    const sources = page.locator('#chatArea .sources, #chatArea [class*="rounded-full"]');
    await expect(sources.first()).toBeVisible({ timeout: 10000 });
  });

  // ─── 标准 RAG 共存 ───

  test('E2E-AGENT-010 Agentic 模式——标准 RAG 共存', async ({ page }) => {
    // 禁用 Agentic 模式
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.agent_enabled', value: String(false) })
    );

    // 标准 RAG 应正常工作
    await page.locator('#queryInput').fill('标准 RAG 查询');
    await page.locator('#sendBtn').click();
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 15000 });

    // 应正常完成
    await expect(page.locator('#sendBtn')).toBeVisible();
    await expect(page.locator('#app')).toBeVisible();
  });
});
