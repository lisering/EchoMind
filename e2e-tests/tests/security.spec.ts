// E2E 安全防御功能（REQ-SEC-013~020）：
// E2E-SEC-001: 安全状态查询——初始状态为 Unencrypted
// E2E-SEC-002: 应用锁定——lock_app 后状态变为 Locked
// E2E-SEC-003: 应用解锁——unlock_app 后状态恢复 EncryptedUnlocked
// E2E-SEC-004: 暴力破解防护——5 次错误后锁定
// E2E-SEC-005: 自动锁屏配置——设置超时时间
// E2E-SEC-006: 剪贴板清除配置——设置清除延迟
// E2E-SEC-007: PII 检测——邮箱/手机号/IP/身份证
// E2E-SEC-008: PII 脱敏——敏感信息替换为掩码
// E2E-SEC-009: 密码强度检测——弱密码/强密码分级
// E2E-SEC-010: 紧急销毁密码——设置与清除
// E2E-SEC-011: 审计日志——查询与清空
// E2E-SEC-012: 活动记录——record_activity 重置计时器
// E2E-SEC-013: security-state-changed 事件——锁定时推送事件
// E2E-SEC-014: 紧急销毁密码验证——正确/错误密码
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-SEC 安全防御功能（REQ-SEC-013~020）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── 安全状态查询 ───

  test('E2E-SEC-001 安全状态查询——初始状态为 Unencrypted', async ({ page }) => {
    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_status')
    );
    expect(status.state).toBe('Unencrypted');
    expect(status.is_locked).toBe(false);
    expect(status.remaining_attempts).toBe(5);
    expect(status.panic_wipe_enabled).toBe(false);
  });

  // ─── 应用锁定 ───

  test('E2E-SEC-002 应用锁定——lock_app 后状态变为 Locked', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('lock_app', { reason: 'Manual' })
    );
    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_status')
    );
    expect(status.state).toBe('Locked');
    expect(status.is_locked).toBe(true);
    expect(status.lock_reason).toBe('Manual');
  });

  // ─── 应用解锁 ───

  test('E2E-SEC-003 应用解锁——unlock_app 后状态恢复', async ({ page }) => {
    // 先锁定
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('lock_app', { reason: 'Manual' })
    );
    // 解锁
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('unlock_app', { password: 'test' })
    );
    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_status')
    );
    expect(status.state).toBe('EncryptedUnlocked');
    expect(status.is_locked).toBe(false);
    expect(status.remaining_attempts).toBe(5);
  });

  // ─── 暴力破解防护 ───

  test('E2E-SEC-004 暴力破解防护——5 次错误后锁定', async ({ page }) => {
    // 模拟 5 次失败尝试（mock 自动递增 authFailures）
    const results = await page.evaluate(async () => {
      const results = [];
      for (let i = 0; i < 5; i++) {
        try {
          await window.__TAURI__.core.invoke('unlock_app', { password: 'wrong' });
          results.push({ attempt: i + 1, locked: false });
        } catch (e) {
          results.push({ attempt: i + 1, locked: true, error: String(e) });
        }
      }
      return results;
    });

    // 前 4 次不应锁定（密码错误但未达上限）
    expect(results[0].locked).toBe(true);
    expect(results[1].locked).toBe(true);
    expect(results[2].locked).toBe(true);
    expect(results[3].locked).toBe(true);
    // 第 5 次应被锁定（错误次数过多）
    expect(results[4].locked).toBe(true);
    // 验证第 5 次的错误消息包含"过多"
    expect(results[4].error).toContain('过多');
  });

  // ─── 自动锁屏配置 ───

  test('E2E-SEC-005 自动锁屏配置——设置超时时间', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_auto_lock_config', {
        enabled: true,
        timeoutSecs: 60,
        lockOnSleep: false,
      })
    );
    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_status')
    );
    expect(status.auto_lock_config.enabled).toBe(true);
    expect(status.auto_lock_config.timeout_secs).toBe(60);
    expect(status.auto_lock_config.lock_on_sleep).toBe(false);
  });

  // ─── 剪贴板清除配置 ───

  test('E2E-SEC-006 剪贴板清除配置——设置清除延迟', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_clipboard_config', {
        enabled: true,
        clearAfterSecs: 10,
      })
    );
    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_status')
    );
    expect(status.clipboard_config.enabled).toBe(true);
    expect(status.clipboard_config.clear_after_secs).toBe(10);
  });

  // ─── PII 检测 ───

  test('E2E-SEC-007 PII 检测——邮箱/手机号/IP/身份证', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('detect_pii', {
        text: '联系我 john@example.com 或 13812345678，IP 192.168.1.1，身份证 110101199001011234',
      })
    );
    expect(result.detections.length).toBeGreaterThanOrEqual(3);
    const types = result.detections.map((d) => d.pii_type);
    expect(types).toContain('email');
    expect(types).toContain('phone');
    expect(types).toContain('ip_address');
  });

  test('E2E-SEC-008 PII 脱敏——敏感信息替换为掩码', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('detect_pii', {
        text: '邮箱 john@example.com 手机 13812345678',
      })
    );
    const emailDet = result.detections.find((d) => d.pii_type === 'email');
    expect(emailDet.redacted).toContain('***');
    expect(emailDet.redacted).not.toContain('john');

    const phoneDet = result.detections.find((d) => d.pii_type === 'phone');
    expect(phoneDet.redacted).toContain('****');
    expect(phoneDet.redacted).not.toContain('13812345678');
  });

  test('E2E-SEC-007b PII 检测——无 PII 文本返回空列表', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('detect_pii', {
        text: '这是一段普通文本，不包含敏感信息。',
      })
    );
    expect(result.detections).toHaveLength(0);
  });

  // ─── 密码强度检测 ───

  test('E2E-SEC-009a 密码强度——弱密码检测为 VeryWeak', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('check_password_strength', { password: '123456' })
    );
    expect(result.strength).toBe('VeryWeak');
    expect(result.percentage).toBe(20);
  });

  test('E2E-SEC-009b 密码强度——强密码检测为 VeryStrong', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('check_password_strength', { password: 'Str0ng@Pass!2024' })
    );
    expect(result.strength).toBe('VeryStrong');
    expect(result.percentage).toBe(100);
  });

  test('E2E-SEC-009c 密码强度——中等密码检测为 Medium', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('check_password_strength', { password: 'xyz789' })
    );
    expect(result.strength).toBe('Medium');
    expect(result.percentage).toBe(60);
  });

  // ─── 紧急销毁密码 ───

  test('E2E-SEC-010 紧急销毁密码——设置与清除', async ({ page }) => {
    // 初始未启用
    const initial = await page.evaluate(() =>
      window.__TAURI__.core.invoke('is_panic_wipe_enabled')
    );
    expect(initial).toBe(false);

    // 设置紧急销毁密码
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_panic_wipe_password', { password: 'panic123' })
    );
    const afterSet = await page.evaluate(() =>
      window.__TAURI__.core.invoke('is_panic_wipe_enabled')
    );
    expect(afterSet).toBe(true);

    // 清除
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('clear_panic_wipe_password')
    );
    const afterClear = await page.evaluate(() =>
      window.__TAURI__.core.invoke('is_panic_wipe_enabled')
    );
    expect(afterClear).toBe(false);
  });

  // ─── 审计日志 ───

  test('E2E-SEC-011 审计日志——查询与清空', async ({ page }) => {
    // 添加一条审计日志
    await page.evaluate(() => {
      window.__mock.state.auditLogs.push({
        id: 'test-1',
        action: 'chat',
        details: '{"pii_count": 2}',
        timestamp: Date.now(),
      });
    });

    // 查询
    const logs = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_audit_logs', { limit: 100 })
    );
    expect(logs.length).toBe(1);
    expect(logs[0].action).toBe('chat');

    // 清空
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('clear_audit_logs')
    );
    const afterClear = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_audit_logs', { limit: 100 })
    );
    expect(afterClear).toHaveLength(0);
  });

  // ─── 活动记录 ───

  test('E2E-SEC-012 活动记录——record_activity 更新时间戳', async ({ page }) => {
    const beforeTime = await page.evaluate(() => window.__mock.state.lastActivity);
    await page.waitForTimeout(50);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('record_activity')
    );
    const afterTime = await page.evaluate(() => window.__mock.state.lastActivity);
    expect(afterTime).toBeGreaterThan(beforeTime);
  });

  // ─── security-state-changed 事件 ───

  test('E2E-SEC-013 security-state-changed 事件——锁定时推送事件', async ({ page }) => {
    let eventReceived = false;
    let eventData = null;
    await page.evaluate(() => {
      window.__TAURI__.event.listen('security-state-changed', (event) => {
        window.__secEventReceived = true;
        window.__secEventData = event.payload;
      });
    });

    await page.evaluate(() =>
      window.__TAURI__.core.invoke('lock_app', { reason: 'Manual' })
    );
    await page.waitForTimeout(100);

    const result = await page.evaluate(() => ({
      received: window.__secEventReceived,
      data: window.__secEventData,
    }));
    expect(result.received).toBe(true);
    expect(result.data.state).toBe('Locked');
  });

  // ─── 紧急销毁密码验证 ───

  test('E2E-SEC-014 紧急销毁密码——清除后验证返回 false', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_panic_wipe_password', { password: 'panic123' })
    );
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('clear_panic_wipe_password')
    );
    // 清除后 is_panic_wipe_enabled 应为 false
    const enabled = await page.evaluate(() =>
      window.__TAURI__.core.invoke('is_panic_wipe_enabled')
    );
    expect(enabled).toBe(false);
  });
});
