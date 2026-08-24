// E2E QM 借鉴功能测试（Q01-Q11 + B05）：
// 验证 IPC 命令在 mock 环境下的端到端行为正确性。
//
// Q01: Scratch-Promote 记忆整合 — trigger_memory_consolidation / get_scratch_logs
// Q02: Burst Buffer 延迟捕获 — push_burst_turn / flush_memory_burst_buffer / get_burst_buffer_status
// Q05: 安全态势分层 — set_security_posture / get_security_posture
// Q06: Shadow 安全筛查 — get_security_screen_stats / reset_security_screen_stats
// Q08: 预算追踪 — get_budget_stats / set_budget_limit
// B05: Durable Prompt Admission — admit_input / promote_input / get_pending_inputs
import { test, expect } from '@playwright/test';
import { injectStub, injectLocales, uiUrl, enterApp } from './helpers.mjs';

test.describe('QM 借鉴功能 E2E 测试', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── Q01: Scratch-Promote 记忆整合 ───

  test('TC-QM-E2E-001 trigger_memory_consolidation 清空 scratch 日志', async ({ page }) => {
    // 先通过 burst buffer 填充一些 scratch 日志
    await page.evaluate(() => {
      window.__mock.state.scratchLogs = [
        { id: 's1', date: '2026-08-08', content: 'fact 1', created_at: Date.now() },
        { id: 's2', date: '2026-08-08', content: 'fact 2', created_at: Date.now() },
      ];
    });

    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('trigger_memory_consolidation'),
    );

    expect(result).toBeTruthy();
    expect(typeof result.actions_count).toBe('number');
    expect(typeof result.remaining_scratch).toBe('number');
    expect(result.remaining_scratch).toBe(0);

    // scratch 日志应被清空
    const logs = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_scratch_logs'),
    );
    expect(logs).toHaveLength(0);
  });

  test('TC-QM-E2E-002 get_scratch_logs 返回 scratch 日志列表', async ({ page }) => {
    await page.evaluate(() => {
      window.__mock.state.scratchLogs = [
        { id: 's1', date: '2026-08-08', content: 'test fact', created_at: 1723000000 },
      ];
    });

    const logs = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_scratch_logs'),
    );

    expect(logs).toHaveLength(1);
    expect(logs[0].id).toBe('s1');
    expect(logs[0].content).toBe('test fact');
  });

  test('TC-QM-E2E-003 get_scratch_logs 支持 limit 参数', async ({ page }) => {
    await page.evaluate(() => {
      window.__mock.state.scratchLogs = Array.from({ length: 10 }, (_, i) => ({
        id: 's' + i,
        date: '2026-08-08',
        content: 'fact ' + i,
        created_at: Date.now(),
      }));
    });

    const logs = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_scratch_logs', { limit: 3 }),
    );

    expect(logs).toHaveLength(3);
  });

  // ─── Q02: Burst Buffer 延迟捕获 ───

  test('TC-QM-E2E-004 push_burst_turn 添加对话轮次到缓冲区', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('push_burst_turn', {
        user_msg: '什么是 Rust?',
        assistant_reply: 'Rust 是一种系统编程语言',
        conversation_id: 'conv-1',
        message_seq: 1,
      }),
    );

    expect(result).toBeTruthy();
    expect(result.pending).toBe(1);
    expect(result.flushed).toBe(false);
    expect(result.extracted).toBe(0);
  });

  test('TC-QM-E2E-005 push_burst_turn 达到阈值自动 flush', async ({ page }) => {
    // 推送 10 条（默认 max_turns=10）
    let lastResult;
    for (let i = 0; i < 10; i++) {
      lastResult = await page.evaluate((seq) =>
        window.__TAURI__.core.invoke('push_burst_turn', {
          user_msg: 'question ' + seq,
          assistant_reply: 'answer ' + seq,
          conversation_id: 'conv-1',
          message_seq: seq,
        }),
      i + 1);
    }

    expect(lastResult.flushed).toBe(true);
    expect(lastResult.pending).toBe(0);
    expect(lastResult.extracted).toBeGreaterThan(0);

    // flush 后 scratch 日志应有记录
    const logs = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_scratch_logs'),
    );
    expect(logs.length).toBeGreaterThan(0);
  });

  test('TC-QM-E2E-006 flush_memory_burst_buffer 手动刷新缓冲区', async ({ page }) => {
    // 先推送几条
    for (let i = 0; i < 3; i++) {
      await page.evaluate((seq) =>
        window.__TAURI__.core.invoke('push_burst_turn', {
          user_msg: 'q' + seq,
          assistant_reply: 'a' + seq,
          conversation_id: 'conv-1',
          message_seq: seq,
        }),
      i + 1);
    }

    // 手动 flush
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('flush_memory_burst_buffer'),
    );

    expect(result.extracted).toBe(3);
    expect(result.pending_before).toBe(3);

    // 缓冲区应空
    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_burst_buffer_status'),
    );
    expect(status.pending).toBe(0);
  });

  test('TC-QM-E2E-007 get_burst_buffer_status 返回缓冲区状态', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('push_burst_turn', {
        user_msg: 'test',
        assistant_reply: 'reply',
        conversation_id: 'conv-1',
        message_seq: 1,
      }),
    );

    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_burst_buffer_status'),
    );

    expect(status.pending).toBe(1);
    expect(status.should_flush).toBe(false);
  });

  test('TC-QM-E2E-008 flush 空缓冲区返回零提取', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('flush_memory_burst_buffer'),
    );

    expect(result.extracted).toBe(0);
    expect(result.pending_before).toBe(0);
  });

  // ─── Q05: 安全态势分层 ───

  test('TC-QM-E2E-009 set_security_posture 设置态势值', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_security_posture', { posture: 'strict' }),
    );

    const posture = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_posture'),
    );
    expect(posture).toBe('strict');
  });

  test('TC-QM-E2E-010 get_security_posture 默认返回 auto', async ({ page }) => {
    const posture = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_posture'),
    );
    expect(posture).toBe('auto');
  });

  test('TC-QM-E2E-011 set_security_posture 无效值抛出错误', async ({ page }) => {
    await expect(
      page.evaluate(() =>
        window.__TAURI__.core.invoke('set_security_posture', { posture: 'invalid' }),
      ),
    ).rejects.toThrow();
  });

  test('TC-QM-E2E-012 set_security_posture 切换 dangerous/auto/strict', async ({ page }) => {
    for (const posture of ['dangerous', 'auto', 'strict']) {
      await page.evaluate((p) =>
        window.__TAURI__.core.invoke('set_security_posture', { posture: p }),
      posture);
      const result = await page.evaluate(() =>
        window.__TAURI__.core.invoke('get_security_posture'),
      );
      expect(result).toBe(posture);
    }
  });

  // ─── Q06: Shadow 安全筛查统计 ───

  test('TC-QM-E2E-013 get_security_screen_stats 返回统计结构', async ({ page }) => {
    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_screen_stats'),
    );

    expect(stats).toBeTruthy();
    expect(typeof stats.total).toBe('number');
    expect(typeof stats.agree).toBe('number');
    expect(typeof stats.disagree).toBe('number');
    expect(typeof stats.unavailable).toBe('number');
  });

  test('TC-QM-E2E-014 reset_security_screen_stats 重置统计', async ({ page }) => {
    // 先设置一些数据
    await page.evaluate(() => {
      window.__mock.state.shadowScreenStats = {
        total: 10,
        agree: 5,
        disagree: 3,
        unavailable: 2,
      };
    });

    await page.evaluate(() =>
      window.__TAURI__.core.invoke('reset_security_screen_stats'),
    );

    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_screen_stats'),
    );
    expect(stats.total).toBe(0);
    expect(stats.agree).toBe(0);
    expect(stats.disagree).toBe(0);
    expect(stats.unavailable).toBe(0);
  });

  // ─── Q08: 预算追踪 ───

  test('TC-QM-E2E-015 get_budget_stats 返回预算统计结构', async ({ page }) => {
    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_budget_stats'),
    );

    expect(stats).toBeTruthy();
    // Mock returns daily_limit_usd / daily_spent_usd (matching backend field names)
    expect(typeof (stats.daily_limit_usd ?? stats.daily_limit)).toBe('number');
    expect(typeof (stats.daily_spent_usd ?? stats.spent_today)).toBe('number');
    expect('remaining' in stats).toBe(true);
  });

  test('TC-QM-E2E-016 set_budget_limit 设置每日预算上限', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_budget_limit', { daily_limit_usd: 5.0 }),
    );

    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_budget_stats'),
    );
    expect(stats.daily_limit_usd ?? stats.daily_limit).toBe(5.0);
  });

  test('TC-QM-E2E-017 预算未设置时 remaining 为 Infinity', async ({ page }) => {
    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_budget_stats'),
    );

    // 默认 daily_limit_usd=0 → remaining=Infinity
    expect(stats.daily_limit_usd ?? stats.daily_limit).toBe(0);
    expect(stats.remaining).toBe(Infinity);
  });

  test('TC-QM-E2E-018 设置预算后 remaining 正确计算', async ({ page }) => {
    await page.evaluate(() => {
      window.__mock.state.budgetDailyLimit = 10.0;
      window.__mock.state.budgetSpentToday = 3.5;
    });

    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_budget_stats'),
    );
    expect(stats.daily_limit_usd ?? stats.daily_limit).toBe(10.0);
    expect(stats.daily_spent_usd ?? stats.spent_today).toBe(3.5);
    expect(stats.remaining).toBeCloseTo(6.5, 2);
  });

  // ─── B05: Durable Prompt Admission ───

  test('TC-QM-E2E-019 admit_input 接纳待处理输入', async ({ page }) => {
    const id = await page.evaluate(() =>
      window.__TAURI__.core.invoke('admit_input', {
        conversation_id: 'conv-1',
        content: 'queued question',
        delivery: 'queue',
      }),
    );

    expect(id).toBeTruthy();
    expect(typeof id).toBe('string');
    expect(id.startsWith('pi-')).toBe(true);
  });

  test('TC-QM-E2E-020 get_pending_inputs 返回未提升的输入', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('admit_input', {
        conversation_id: 'conv-1',
        content: 'question 1',
        delivery: 'queue',
      }),
    );
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('admit_input', {
        conversation_id: 'conv-1',
        content: 'steer hint',
        delivery: 'steer',
      }),
    );

    const pending = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_pending_inputs', { conversation_id: 'conv-1' }),
    );

    expect(pending).toHaveLength(2);
    // steer 优先排序
    expect(pending[0].delivery).toBe('steer');
    expect(pending[1].delivery).toBe('queue');
  });

  test('TC-QM-E2E-021 promote_input 提升待处理输入', async ({ page }) => {
    const id = await page.evaluate(() =>
      window.__TAURI__.core.invoke('admit_input', {
        conversation_id: 'conv-1',
        content: 'to be promoted',
        delivery: 'queue',
      }),
    );

    await page.evaluate((inputId) =>
      window.__TAURI__.core.invoke('promote_input', { input_id: inputId }),
    id);

    // 提升后不应出现在 pending 列表中
    const pending = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_pending_inputs', { conversation_id: 'conv-1' }),
    );
    expect(pending).toHaveLength(0);
  });

  test('TC-QM-E2E-022 get_pending_inputs 按会话隔离', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('admit_input', {
        conversation_id: 'conv-1',
        content: 'conv1 question',
        delivery: 'queue',
      }),
    );
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('admit_input', {
        conversation_id: 'conv-2',
        content: 'conv2 question',
        delivery: 'queue',
      }),
    );

    const pending1 = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_pending_inputs', { conversation_id: 'conv-1' }),
    );
    const pending2 = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_pending_inputs', { conversation_id: 'conv-2' }),
    );

    expect(pending1).toHaveLength(1);
    expect(pending1[0].content).toBe('conv1 question');
    expect(pending2).toHaveLength(1);
    expect(pending2[0].content).toBe('conv2 question');
  });

  // ─── 集成场景：多 QM 功能协同 ───

  test('TC-QM-E2E-023 Burst Buffer flush 后 trigger_memory_consolidation 清空 scratch', async ({ page }) => {
    // 推送并 flush burst buffer
    for (let i = 0; i < 3; i++) {
      await page.evaluate((seq) =>
        window.__TAURI__.core.invoke('push_burst_turn', {
          user_msg: 'q' + seq,
          assistant_reply: 'a' + seq,
          conversation_id: 'conv-1',
          message_seq: seq,
        }),
      i + 1);
    }
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('flush_memory_burst_buffer'),
    );

    // scratch 日志应有记录
    let logs = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_scratch_logs'),
    );
    expect(logs.length).toBeGreaterThan(0);

    // 触发整合后 scratch 日志清空
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('trigger_memory_consolidation'),
    );
    logs = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_scratch_logs'),
    );
    expect(logs).toHaveLength(0);
  });

  test('TC-QM-E2E-024 安全态势切换不影响 Shadow 统计', async ({ page }) => {
    // 设置 shadow 统计
    await page.evaluate(() => {
      window.__mock.state.shadowScreenStats = {
        total: 5,
        agree: 3,
        disagree: 1,
        unavailable: 1,
      };
    });

    // 切换安全态势
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_security_posture', { posture: 'strict' }),
    );

    // Shadow 统计不应被影响
    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_screen_stats'),
    );
    expect(stats.total).toBe(5);
    expect(stats.agree).toBe(3);
  });

  // ─── Q03: 双阈值压缩 + 压缩比设置 ───

  test('TC-QM-E2E-025 set_compression_ratio 设置压缩比', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'compression.ratio', value: String(0.5) }),
    );
    const ratio = await page.evaluate(() =>
      (async () => parseFloat(await window.__TAURI__.core.invoke('get_setting', { key: 'compression.ratio' })))(),
    );
    expect(ratio).toBe(0.5);
  });

  test('TC-QM-E2E-026 get_compression_ratio 默认返回初始值', async ({ page }) => {
    const ratio = await page.evaluate(() =>
      (async () => parseFloat(await window.__TAURI__.core.invoke('get_setting', { key: 'compression.ratio' })))(),
    );
    expect(typeof ratio).toBe('number');
    expect(ratio).toBeGreaterThanOrEqual(0);
    expect(ratio).toBeLessThanOrEqual(1);
  });

  // ─── Q04: Token 级预算 + 对话费用追踪 ───

  test('TC-QM-E2E-027 get_conversation_cost 返回费用结构', async ({ page }) => {
    const cost = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_conversation_cost', { conversation_id: 'test-conv' }),
    );
    expect(cost).toBeTruthy();
    expect(cost.conversation_id).toBe('test-conv');
    expect(typeof cost.total_prompt_tokens).toBe('number');
    expect(typeof cost.total_completion_tokens).toBe('number');
    expect(typeof cost.total_tokens).toBe('number');
    expect(cost.total_tokens).toBe(cost.total_prompt_tokens + cost.total_completion_tokens);
    expect(typeof cost.exchange_count).toBe('number');
  });

  test('TC-QM-E2E-028 set_token_budget 设置预算', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_token_budget', { budget: 50000 }),
    );
    const cost = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_conversation_cost', { conversation_id: 'test-conv' }),
    );
    expect(cost.token_budget).toBe(50000);
  });

  test('TC-QM-E2E-029 token_budget 默认为 0（无限制）', async ({ page }) => {
    const cost = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_conversation_cost', { conversation_id: 'test-conv' }),
    );
    expect(cost.token_budget).toBe(0);
  });

  // ─── Q07: 检索记忆 + 反馈记录 ───

  test('TC-QM-E2E-030 set_retrieval_memory_enabled 开关切换', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.retrieval_memory_enabled', value: String(true) }),
    );
    // 验证状态已更新（通过 mock state 检查）
    const enabled = await page.evaluate(() => window.__mock.state.retrievalMemoryEnabled);
    expect(enabled).toBe(true);
  });

  test('TC-QM-E2E-031 get_retrieval_memory_stats 返回统计数组', async ({ page }) => {
    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_retrieval_memory_stats'),
    );
    expect(Array.isArray(stats)).toBe(true);
  });

  test('TC-QM-E2E-032 record_retrieval_feedback 记录反馈', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('record_retrieval_feedback', {
        query: 'test query',
        doc_id: 'doc-1',
        feedback: 'positive',
      }),
    );
    expect(result).toBeNull();
    // 验证反馈已记录
    const stats = await page.evaluate(() => window.__mock.state.retrievalMemoryStats);
    expect(stats.length).toBeGreaterThan(0);
  });

  test('TC-QM-E2E-033 reset_retrieval_memory 重置统计', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.retrieval_memory_enabled', value: String(true) }),
    );
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('reset_retrieval_memory'),
    );
    expect(result).toBeNull();
    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_retrieval_memory_stats'),
    );
    expect(stats).toHaveLength(0);
  });

  // ─── Q10: 缓存统计 + 配置 ───

  test('TC-QM-E2E-034 get_cache_stats 返回缓存统计', async ({ page }) => {
    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_cache_stats'),
    );
    expect(stats).toBeTruthy();
    expect(typeof stats.exact_hits).toBe('number');
    expect(typeof stats.semantic_hits).toBe('number');
    expect(typeof stats.retrieval_hits).toBe('number');
    expect(typeof stats.cache_size_entries).toBe('number');
    expect(typeof stats.estimated_token_saved).toBe('number');
  });

  test('TC-QM-E2E-035 clear_cache 清空缓存统计', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('clear_cache'),
    );
    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_cache_stats'),
    );
    expect(stats.exact_hits).toBe(0);
    expect(stats.semantic_hits).toBe(0);
    expect(stats.retrieval_hits).toBe(0);
    expect(stats.cache_size_entries).toBe(0);
    expect(stats.estimated_token_saved).toBe(0);
  });

  test('TC-QM-E2E-036 set_cache_settings + get_cache_settings', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_cache_settings', {
        settings: { enabled: true, max_entries: 500, ttl_seconds: 3600 },
      }),
    );
    const settings = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_cache_settings'),
    );
    expect(settings).toBeTruthy();
    expect(settings.enabled).toBe(true);
    expect(settings.max_entries).toBe(500);
    expect(settings.ttl_seconds).toBe(3600);
  });

  // ─── Q11: 索引重建 + 摘要树 ───

  test('TC-QM-E2E-037 rebuild_bm25_index 完成无错误', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('rebuild_bm25_index'),
    );
    expect(result).toBeNull();
  });

  test('TC-QM-E2E-038 rebuild_proposition_index 完成无错误', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('rebuild_proposition_index'),
    );
    expect(result).toBeNull();
  });

  test('TC-QM-E2E-039 build_summary_tree 完成无错误', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('build_summary_tree'),
    );
    expect(result).toBeNull();
  });

  // ─── 跨功能集成测试 ───

  test('TC-QM-E2E-040 预算设置后费用追踪包含预算信息', async ({ page }) => {
    // 设置预算
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_token_budget', { budget: 10000 }),
    );
    // 查询费用
    const cost = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_conversation_cost', { conversation_id: 'integration-test' }),
    );
    expect(cost.token_budget).toBe(10000);
    expect(cost.total_tokens).toBeGreaterThan(0);
    // 费用不应超过预算（模拟场景）
    expect(cost.total_tokens).toBeLessThanOrEqual(cost.token_budget);
  });

  test('TC-QM-E2E-041 压缩比 + 缓存清空互不影响', async ({ page }) => {
    // 设置压缩比
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'compression.ratio', value: String(0.3) }),
    );
    // 清空缓存
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('clear_cache'),
    );
    // 压缩比不应被缓存清空影响
    const ratio = await page.evaluate(() =>
      (async () => parseFloat(await window.__TAURI__.core.invoke('get_setting', { key: 'compression.ratio' })))(),
    );
    expect(ratio).toBe(0.3);
    // 缓存统计应已清空
    const stats = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_cache_stats'),
    );
    expect(stats.exact_hits).toBe(0);
  });
});
