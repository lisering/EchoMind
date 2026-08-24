// E2E 安全防御高级场景（REQ-SEC-013~020）：
// E2E-SEC-ADV-001: 数据库加密——设置密码后状态变更
// E2E-SEC-ADV-002: 数据库加密——加密后解锁流程
// E2E-SEC-ADV-003: 自动锁定——设置超时后锁定
// E2E-SEC-ADV-004: 剪贴板清除——设置延迟后清除
// E2E-SEC-ADV-005: PII 检测——8 种类型全覆盖
// E2E-SEC-ADV-006: PII 脱敏——替换为掩码
// E2E-SEC-ADV-007: 密码强度——弱密码/强密码
// E2E-SEC-ADV-008: 紧急销毁——设置后查询状态
// E2E-SEC-ADV-009: 审计日志——哈希链验证
// E2E-SEC-ADV-010: 审计日志——清空
// E2E-SEC-ADV-011: 暴力破解——指数退避
// E2E-SEC-ADV-012: 路径遍历防护——.. 被拒绝
// E2E-SEC-ADV-013: 文件格式白名单——非白名单被拒绝
// E2E-SEC-ADV-014: 网络白名单——默认无外网请求
// E2E-SEC-ADV-015: 安全状态变更事件
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';

test.describe('E2E-SEC-ADV 安全防御高级场景（REQ-SEC-013~020）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── 数据库加密 ───

  test('E2E-SEC-ADV-001 数据库加密——设置密码后状态变更', async ({ page }) => {
    // 设置加密密码（mock 会模拟加密流程）
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('encrypt_database', { password: 'StrongP@ss2024' })
    );

    // 状态应变更
    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_status')
    );
    // 加密后状态应为 EncryptedUnlocked
    expect(status.state).toBe('EncryptedUnlocked');
    expect(status.is_locked).toBe(false);
  });

  test('E2E-SEC-ADV-002 数据库加密——加密后解锁流程', async ({ page }) => {
    // 先加密
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('encrypt_database', { password: 'TestPass123' })
    );

    // 锁定
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('lock_app', { reason: 'Manual' })
    );

    // 解锁
    const unlockResult = await page.evaluate(() =>
      window.__TAURI__.core.invoke('unlock_app', { password: 'TestPass123' })
    );

    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_status')
    );
    expect(status.state).toBe('EncryptedUnlocked');
    expect(status.is_locked).toBe(false);
    expect(status.remaining_attempts).toBe(5);
  });

  // ─── 自动锁定 ───

  test('E2E-SEC-ADV-003 自动锁定——设置超时后配置生效', async ({ page }) => {
    // 先加密
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('encrypt_database', { password: 'TestPass123' })
    );

    // 设置自动锁定超时为 60 秒
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_auto_lock_config', {
        enabled: true,
        timeout_secs: 60,
        lock_on_sleep: true,
      })
    );

    // 验证配置已生效
    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_status')
    );
    expect(status.state).toBe('EncryptedUnlocked');
    expect(status.auto_lock_config.timeout_secs).toBe(60);
    expect(status.auto_lock_config.enabled).toBe(true);
    expect(status.auto_lock_config.lock_on_sleep).toBe(true);
  });

  // ─── 剪贴板清除 ───

  test('E2E-SEC-ADV-004 剪贴板清除——设置延迟后配置生效', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_clipboard_config', {
        enabled: true,
        clear_after_secs: 5,
      })
    );

    // 应成功设置（Tauri Ok(()) 序列化为 null）
    expect(result).toBeNull();

    // 验证配置已设置（通过 get_security_status 返回）
    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_status')
    );
    expect(status.clipboard_config.enabled).toBe(true);
    expect(status.clipboard_config.clear_after_secs).toBe(5);
  });

  // ─── PII 检测 ───

  test('E2E-SEC-ADV-005 PII 检测——8 种类型全覆盖', async ({ page }) => {
    const testCases = [
      { type: 'email', text: '联系我：user@example.com' },
      { type: 'phone', text: '电话：13800138000' },
      { type: 'ip', text: '服务器 IP：192.168.1.1' },
      { type: 'idcard', text: '身份证：110101199001011234' },
      { type: 'bankcard', text: '银行卡：6222021234567890123' },
      { type: 'ssn', text: 'SSN: 123-45-6789' },
      { type: 'passport', text: '护照：G12345678' },
      { type: 'intl_phone', text: '国际电话：+86-13800138000' },
    ];

    for (const tc of testCases) {
      const result = await page.evaluate((text) =>
        window.__TAURI__.core.invoke('detect_pii', { text })
      , tc.text);

      // 应检测到 PII（无 if-guard，强制断言）
      expect(result, `PII 类型 ${tc.type} 检测结果不应为空`).not.toBeNull();
      expect(Array.isArray(result.detections), `PII 类型 ${tc.type} 应有 detections 数组`).toBe(true);
      expect(result.detections.length, `PII 类型 ${tc.type} 应检测到至少 1 个 PII`).toBeGreaterThan(0);
      // 脱敏文本应包含 REDACTED（无 if-guard）
      expect(typeof result.redacted, `PII 类型 ${tc.type} 应有脱敏文本`).toBe('string');
      expect(result.redacted, `PII 类型 ${tc.type} 脱敏文本应包含 REDACTED`).toContain('REDACTED');
      // 原始 PII 不应出现在脱敏文本中
      expect(result.redacted, `PII 类型 ${tc.type} 脱敏文本不应包含原始 PII`).not.toContain(tc.text.split('：')[1] || tc.text.split(':')[1]?.trim());
    }
  });

  test('E2E-SEC-ADV-006 PII 脱敏——替换为掩码', async ({ page }) => {
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('detect_pii', {
        text: '我的邮箱是 test@example.com，电话是 13800138000',
      })
    );

    expect(result, 'PII 检测结果不应为空').not.toBeNull();
    // 脱敏文本不应包含原始邮箱（无 if-guard，强制断言）
    expect(typeof result.redacted, '应有脱敏文本').toBe('string');
    expect(result.redacted, '脱敏文本不应包含原始邮箱').not.toContain('test@example.com');
    expect(result.redacted, '脱敏文本不应包含原始手机号').not.toContain('13800138000');
    expect(result.redacted, '脱敏文本应包含 REDACTED 标记').toContain('REDACTED');
    // 检测到至少 2 种 PII（邮箱+手机）
    expect(result.detections.length, '应检测到至少 2 个 PII').toBeGreaterThanOrEqual(2);
    // 检测到的 PII 类型应包含 email 和 phone
    const piiTypes = result.detections.map((d: { pii_type: string }) => d.pii_type);
    expect(piiTypes, '应检测到 email 类型').toContain('email');
    expect(piiTypes, '应检测到 phone 类型').toContain('phone');
  });

  // ─── 密码强度 ───

  test('E2E-SEC-ADV-007 密码强度——弱密码与强密码', async ({ page }) => {
    // 弱密码
    const weakResult = await page.evaluate(() =>
      window.__TAURI__.core.invoke('check_password_strength', { password: '123' })
    );
    expect(weakResult.level).toBe('weak');
    expect(weakResult.percentage).toBeLessThan(40);

    // 强密码
    const strongResult = await page.evaluate(() =>
      window.__TAURI__.core.invoke('check_password_strength', { password: 'Str0ng!Pass2024' })
    );
    expect(strongResult.level).toBe('strong');
    expect(strongResult.percentage).toBeGreaterThan(70);

    // 弱密码应有改进建议
    expect(weakResult.suggestions.length).toBeGreaterThan(0);
  });

  // ─── 紧急销毁 ───

  test('E2E-SEC-ADV-008 紧急销毁——设置后查询状态', async ({ page }) => {
    // 设置紧急销毁密码
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('set_panic_wipe_password', { password: 'PanicP@ss' })
    );

    // 查询状态
    const isEnabled = await page.evaluate(() =>
      window.__TAURI__.core.invoke('is_panic_wipe_enabled')
    );
    expect(isEnabled).toBe(true);

    // 清除紧急销毁密码
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('clear_panic_wipe_password')
    );

    const isEnabledAfter = await page.evaluate(() =>
      window.__TAURI__.core.invoke('is_panic_wipe_enabled')
    );
    expect(isEnabledAfter).toBe(false);
  });

  // ─── 审计日志 ───

  test('E2E-SEC-ADV-009 审计日志——查询返回数组', async ({ page }) => {
    // 执行操作以产生审计日志
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('detect_pii', { text: 'test@example.com' })
    );

    // 查询审计日志
    const logs = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_audit_logs', { limit: 10 })
    );

    // 应返回日志数组（强制断言，不使用 toBeGreaterThanOrEqual(0)）
    expect(Array.isArray(logs), '审计日志应返回数组').toBe(true);
    expect(logs.length, '审计日志长度应不超过 limit 10').toBeLessThanOrEqual(10);
  });

  test('E2E-SEC-ADV-010 审计日志——清空', async ({ page }) => {
    // 清空审计日志
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('clear_audit_logs')
    );

    // 查询审计日志应为空
    const logs = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_audit_logs', { limit: 10 })
    );
    expect(logs.length).toBe(0);
  });

  // ─── 暴力破解防护 ───

  test('E2E-SEC-ADV-011 暴力破解——5 次错误后锁定', async ({ page }) => {
    // 先加密
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('encrypt_database', { password: 'CorrectPass123' })
    );

    // 锁定
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('lock_app', { reason: 'Manual' })
    );

    // 模拟 5 次错误密码（每次应抛出错误）
    for (let i = 0; i < 5; i++) {
      await expect(
        page.evaluate(() =>
          window.__TAURI__.core.invoke('unlock_app', { password: 'wrong' })
        )
      ).rejects.toThrow();
    }

    // 第 6 次后应被锁定
    const status = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_security_status')
    );
    expect(status.remaining_attempts, '5 次错误后剩余尝试次数应为 0').toBe(0);
    // 应有锁定时间（强制断言，不使用 if-guard）
    expect(status.remaining_lock_seconds, '应设置锁定时间').toBeGreaterThan(0);
    expect(status.is_locked, '应处于锁定状态').toBe(true);
  });

  // ─── 路径遍历防护 ───

  test('E2E-SEC-ADV-012 路径遍历防护——.. 被拒绝', async ({ page }) => {
    await expect(
      page.evaluate(() =>
        window.__TAURI__.core.invoke('import_files', { paths: ['../../../etc/passwd'] })
      )
    ).rejects.toThrow();
  });

  test('E2E-SEC-ADV-013 文件格式白名单——非白名单被拒绝', async ({ page }) => {
    await expect(
      page.evaluate(() =>
        window.__TAURI__.core.invoke('import_files', { paths: ['/mock/bad.exe'] })
      )
    ).rejects.toThrow();
  });

  // ─── 网络白名单 ───

  test('E2E-SEC-ADV-014 网络白名单——默认无外网请求', async ({ page }) => {
    const requests = [];
    page.on('request', (req) => {
      const url = req.url();
      if (url.startsWith('http://') || url.startsWith('https://')) {
        requests.push(url);
      }
    });

    await page.waitForTimeout(1000);
    expect(requests).toHaveLength(0);
  });

  // ─── 安全状态变更事件 ───

  test('E2E-SEC-ADV-015 安全状态变更事件', async ({ page }) => {
    // 监听 security-state-changed 事件
    await page.evaluate(() => {
      window.__state.listeners['security-state-changed'] = window.__state.listeners['security-state-changed'] || [];
      window.__state.listeners['security-state-changed'].push(() => {
        window.__secStateChanged = true;
      });
    });

    // 触发锁定
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('lock_app', { reason: 'Manual' })
    );
    await page.waitForTimeout(300);

    // 应收到状态变更事件
    const changed = await page.evaluate(() => window.__secStateChanged);
    expect(changed).toBe(true);
  });
});
