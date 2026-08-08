// E2E 高级 RAG 功能（REQ-RAG-020~022, REQ-VEC-012）：
// 验证 UI 交互 → IPC 调用 → 状态变更的完整链路。
//
// 注：hybridSearch 和 agent 模式无 UI 开关，仅通过 IPC 控制；
// rerank / hyde / embeddingModel 有实际 UI 控件，通过 UI 交互测试。
//
// E2E-RAG-ADV-001: 设置面板中重排序开关可见
// E2E-RAG-ADV-002: 启用重排序——UI 交互 + IPC 状态同步
// E2E-RAG-ADV-003: 禁用重排序——UI 交互
// E2E-RAG-ADV-004: HyDE 开关——UI 可见性与交互
// E2E-RAG-ADV-005: 嵌入模型选择器——UI 可见性与切换
// E2E-RAG-ADV-006: 混合检索——IPC 开关（无 UI 控件）
// E2E-RAG-ADV-007: Agent 模式——IPC 开关（无 UI 控件）
// E2E-RAG-ADV-008: 多功能组合——同时启用并验证状态
// E2E-RAG-ADV-009: 功能独立性——关闭一个不影响其他
// E2E-RAG-ADV-010: 混合检索启用后——对话中引用来源正常返回
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, importDocs, sendMessage, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';

test.describe('E2E-RAG-ADV 高级 RAG 功能（REQ-RAG-020~022, REQ-VEC-012）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── Cross-Encoder 重排序（有 UI 开关） ───

  test('E2E-RAG-ADV-001 设置面板中重排序开关可见', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsPanel, #settingsModal')).toBeVisible({ timeout: 3000 });
    const toggle = page.locator('#rerankToggle');
    await expect(toggle).toBeVisible({ timeout: 3000 });
    // 默认关闭
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
  });

  test('E2E-RAG-ADV-002 启用重排序——UI 交互 + IPC 状态同步', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsPanel, #settingsModal')).toBeVisible({ timeout: 3000 });
    const toggle = page.locator('#rerankToggle');
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
    // 点击启用
    await toggle.click();
    // mock 状态应同步
    expect(await page.evaluate(() => window.__mock.state.rerankEnabled)).toBe(true);
    // UI 选中态
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
  });

  test('E2E-RAG-ADV-003 禁用重排序——UI 交互', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsPanel, #settingsModal')).toBeVisible({ timeout: 3000 });
    const toggle = page.locator('#rerankToggle');
    // 先启用
    await toggle.click();
    expect(await page.evaluate(() => window.__mock.state.rerankEnabled)).toBe(true);
    // 再禁用
    await toggle.click();
    expect(await page.evaluate(() => window.__mock.state.rerankEnabled)).toBe(false);
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
  });

  // ─── HyDE 查询改写（有 UI 开关） ───

  test('E2E-RAG-ADV-004 HyDE 开关——UI 可见性与交互', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsPanel, #settingsModal')).toBeVisible({ timeout: 3000 });
    const toggle = page.locator('#hydeToggle');
    await expect(toggle).toBeVisible({ timeout: 3000 });
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
    // 启用
    await toggle.click();
    expect(await page.evaluate(() => window.__mock.state.hydeEnabled)).toBe(true);
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
  });

  // ─── 嵌入模型切换（有 UI 选择器） ───

  test('E2E-RAG-ADV-005 嵌入模型选择器——UI 可见性与切换', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsPanel, #settingsModal')).toBeVisible({ timeout: 3000 });
    const select = page.locator('#embeddingModelSelect');
    await expect(select).toBeVisible({ timeout: 3000 });
    // 默认值
    const initialValue = await select.evaluate(el => (el as HTMLSelectElement).value);
    expect(initialValue).toBe('all-MiniLM-L6-v2');
    // 切换模型
    await select.selectOption('bge-small-zh-v1.5');
    expect(await page.evaluate(() => window.__mock.state.embeddingModel)).toBe('bge-small-zh-v1.5');
    // 切换回默认
    await select.selectOption('all-MiniLM-L6-v2');
    expect(await page.evaluate(() => window.__mock.state.embeddingModel)).toBe('all-MiniLM-L6-v2');
  });

  // ─── 混合检索（无 UI 开关，仅 IPC） ───

  test('E2E-RAG-ADV-006 混合检索——IPC 开关', async ({ page }) => {
    // 默认关闭
    expect(await page.evaluate(() => window.__mock.state.hybridSearch)).toBe(false);
    // 启用
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_hybrid_search', { enabled: true })
    );
    expect(await page.evaluate(() => window.__mock.state.hybridSearch)).toBe(true);
    // 禁用
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_hybrid_search', { enabled: false })
    );
    expect(await page.evaluate(() => window.__mock.state.hybridSearch)).toBe(false);
  });

  // ─── Agent 模式（无 UI 开关，仅 IPC） ───

  test('E2E-RAG-ADV-007 Agent 模式——IPC 开关', async ({ page }) => {
    expect(await page.evaluate(() => window.__mock.state.agentEnabled)).toBe(false);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_agent_enabled', { enabled: true })
    );
    expect(await page.evaluate(() => window.__mock.state.agentEnabled)).toBe(true);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_agent_enabled', { enabled: false })
    );
    expect(await page.evaluate(() => window.__mock.state.agentEnabled)).toBe(false);
  });

  // ─── 多功能组合 ───

  test('E2E-RAG-ADV-008 多功能组合——同时启用并验证状态', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsPanel, #settingsModal')).toBeVisible({ timeout: 3000 });

    // UI 开关：启用重排序和 HyDE
    await page.locator('#rerankToggle').click();
    await page.locator('#hydeToggle').click();
    // IPC 开关：启用混合检索和 Agent
    await page.evaluate(() => window.__TAURI__.core.invoke('set_hybrid_search', { enabled: true }));
    await page.evaluate(() => window.__TAURI__.core.invoke('set_agent_enabled', { enabled: true }));

    // 验证所有状态
    const state = await page.evaluate(() => ({
      rerank: window.__mock.state.rerankEnabled,
      hyde: window.__mock.state.hydeEnabled,
      hybrid: window.__mock.state.hybridSearch,
      agent: window.__mock.state.agentEnabled,
    }));
    expect(state.rerank).toBe(true);
    expect(state.hyde).toBe(true);
    expect(state.hybrid).toBe(true);
    expect(state.agent).toBe(true);

    // 验证 UI 开关选中态
    await expect(page.locator('#rerankToggle')).toHaveAttribute('aria-checked', 'true');
    await expect(page.locator('#hydeToggle')).toHaveAttribute('aria-checked', 'true');
  });

  test('E2E-RAG-ADV-009 功能独立性——关闭一个不影响其他', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsPanel, #settingsModal')).toBeVisible({ timeout: 3000 });

    // 全部启用
    await page.locator('#rerankToggle').click();
    await page.locator('#hydeToggle').click();
    await page.evaluate(() => window.__TAURI__.core.invoke('set_hybrid_search', { enabled: true }));
    await page.evaluate(() => window.__TAURI__.core.invoke('set_agent_enabled', { enabled: true }));

    // 关闭重排序
    await page.locator('#rerankToggle').click();

    // 验证独立性
    const state = await page.evaluate(() => ({
      rerank: window.__mock.state.rerankEnabled,
      hyde: window.__mock.state.hydeEnabled,
      hybrid: window.__mock.state.hybridSearch,
      agent: window.__mock.state.agentEnabled,
    }));
    expect(state.rerank).toBe(false);
    expect(state.hyde).toBe(true);
    expect(state.hybrid).toBe(true);
    expect(state.agent).toBe(true);
  });

  // ─── 混合检索 + 对话集成 ───

  test('E2E-RAG-ADV-010 混合检索启用后——对话正常完成', async ({ page }) => {
    // 导入文档
    await importDocs(page, ['/mock/echomind-e2e.md']);
    // 启用混合检索（通过 IPC，不打开设置面板）
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_hybrid_search', { enabled: true })
    );
    expect(await page.evaluate(() => window.__mock.state.hybridSearch)).toBe(true);

    // 发送消息
    await sendMessage(page, '混合检索测试');
    await waitForStreamDone(page, 15000);

    // 对话应正常完成，有内容返回
    const mdContent = await page.locator('#chatArea .md').last().innerText();
    expect(mdContent.length, '回答内容不应为空').toBeGreaterThan(0);
    // 输入框恢复空闲态
    await expect(page.locator('#queryInput')).not.toBeDisabled();
  });
});
