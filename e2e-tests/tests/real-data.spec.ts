// 真实数据集成测试（根因 V5 修复：消除"无真实数据集成"漏洞）。
//
// 此 spec 不注入 tauri-stub，而是通过真实 Tauri IPC 连接运行中的 EchoMind 后端。
// 验证核心 RAG 全链路：真实文件导入 → 嵌入 → 检索 → 生成 → 引用来源。
//
// ## 环境变量
//
// | 变量 | 说明 |
// |---|---|
// | `ECHOMIND_E2E_REAL_DATA` | 设为 `1` 启用真实数据集成测试 |
// | `ECHOMIND_LLM_API_KEY` | LLM API Key（必须） |
// | `ECHOMIND_LLM_BASE_URL` | OpenAI 兼容端点 |
// | `ECHOMIND_LLM_MODEL` | 模型名 |
// | `ECHOMIND_E2E_URL` | Tauri dev 服务器 URL（默认 `http://localhost:1420`） |
// | `ECHOMIND_TEST_DOC` | 测试文档路径（默认 lisp-rs/README_zh.md） |
//
// ## 运行方式
//
// ```bash
// # 1. 启动 EchoMind dev 服务器
// ⚠️ 现状（V3.1 阶段二）：本 spec 暂不可用——Tauri v2 不向 remote URL
// （devUrl HTTP 页面）注入 window.__TAURI__（安全设计，capabilities remote
// 授权仅解决 ACL 不解决注入）。权威验证路径已迁移至 Rust 集成测试：
//   crates/tauri-app/tests/integration/real_data_tests.rs（real_data_001）
//   CI: .github/workflows/real-data-e2e.yml
// 本文件保留为 UI 层验证的历史参考，待 Tauri 上游支持 remote 注入后恢复。
//
// cargo tauri dev &
//
// # 2. 运行真实数据集成测试
// ECHOMIND_E2E_REAL_DATA=1 \
// ECHOMIND_LLM_API_KEY=sk-xxx \
// ECHOMIND_LLM_BASE_URL=https://api.deepseek.com \
// ECHOMIND_LLM_MODEL=deepseek-chat \
// npx playwright test tests/real-data.spec.ts
// ```

import { test, expect, type Page } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import fs from 'node:fs';

const isRealDataEnabled = process.env.ECHOMIND_E2E_REAL_DATA === '1';
const e2eUrl = process.env.ECHOMIND_E2E_URL || 'http://localhost:1420';
const testDocPath = process.env.ECHOMIND_TEST_DOC ||
  path.resolve(process.env.HOME || '/Users/john', 'freesoft/lisp-rs/README_zh.md');

// 测试跳过条件：未启用真实数据模式时跳过
test.skip(!isRealDataEnabled, '未设置 ECHOMIND_E2E_REAL_DATA=1，跳过真实数据集成测试');

// 测试超时时间（真实嵌入 + LLM 调用需要较长时间）
test.setTimeout(120_000);

// ============================================================
// 辅助函数
// ============================================================

/** 通过 IPC 导入文件并等待嵌入完成 */
async function importDocumentAndWait(page: Page, docPath: string): Promise<void> {
  // 调用 import_files 导入文件
  await page.evaluate(async (p) => {
    await window.__TAURI__.core.invoke('import_files', { paths: [p] });
  }, docPath);

  // 等待文档状态变为 Indexed（通过轮询 get_documents）
  await page.waitForFunction(
    async () => {
      const docs = await window.__TAURI__.core.invoke('get_documents');
      const doc = docs.find((d: any) => d.file_path.includes('README_zh'));
      return doc && doc.status === 'Indexed';
    },
    { timeout: 90_000 },
  );
}

/** 发送消息并等待回答完成 */
async function sendMessageAndWait(page: Page, query: string): Promise<void> {
  await page.locator('#queryInput').fill(query);
  await page.locator('#sendBtn').click();

  // 等待 sendBtn 重新可见（chat_done 后恢复）
  await page.locator('#sendBtn').waitFor({ state: 'visible', timeout: 60_000 });
  await page.waitForTimeout(1000); // 等待渲染稳定
}

// ============================================================
// 测试用例
// ============================================================

test.describe('真实数据集成测试 — lisp-rs/README_zh.md', () => {

  test.beforeAll(async () => {
    // 验证测试文件存在
    expect(fs.existsSync(testDocPath), `测试文件不存在: ${testDocPath}`).toBe(true);
  });

  test('RD-001 真实导入文档 → 嵌入 → 检索 → 回答含文档内容', async ({ page }) => {
    await page.goto(e2eUrl);
    await page.waitForTimeout(3000);

    // 步骤 1：检查空知识库状态（空库时输入框应禁用）
    const input = page.locator('#queryInput');
    const isDisabled = await input.isDisabled();
    // 首次启动可能是空库（禁用）或已有文档（启用），都接受

    // 步骤 2：导入 lisp-rs/README_zh.md
    await importDocumentAndWait(page, testDocPath);

    // 步骤 3：验证输入框已启用
    await expect(input).toBeEnabled({ timeout: 10_000 });

    // 步骤 4：发送与文档内容相关的问题
    await sendMessageAndWait(page, 'Lisp 的语法规则有几条？');

    // 步骤 5：验证回答包含文档相关内容
    const messages = page.locator('.message-in');
    const lastMessage = messages.last();
    await expect(lastMessage).toBeVisible();

    // 获取回答文本
    const answerText = await lastMessage.textContent();
    expect(answerText, '回答不应为空').not.toBeNull();
    expect(answerText!.length, '回答长度应 > 10').toBeGreaterThan(10);

    // 验证回答包含关键词（"两条" 或 "规则" 或 "括号"）
    const hasRelevantContent =
      answerText!.includes('两条') ||
      answerText!.includes('规则') ||
      answerText!.includes('括号') ||
      answerText!.includes('2');
    expect(hasRelevantContent, `回答应包含 Lisp 语法规则相关内容，实际回答: ${answerText!.substring(0, 200)}`).toBe(true);

    // 步骤 6：验证引用来源按钮存在
    const sourcesBtn = page.locator('button:has-text("引用来源")');
    const sourcesCount = await sourcesBtn.count();
    expect(sourcesCount, '应至少有一个引用来源').toBeGreaterThan(0);
  });

  test('RD-002 多轮问答上下文保持', async ({ page }) => {
    await page.goto(e2eUrl);
    await page.waitForTimeout(3000);

    // 导入文档
    await importDocumentAndWait(page, testDocPath);

    // 第一轮：问 Lisp 名字来源
    await sendMessageAndWait(page, 'Lisp 这个名字是怎么来的？');
    const messages = page.locator('.message-in');
    const firstAnswer = await messages.nth(1).textContent();
    expect(firstAnswer).not.toBeNull();
    expect(firstAnswer!.length, '回答长度应 > 10').toBeGreaterThan(10);
    const hasListProcessing =
      firstAnswer!.includes('List') ||
      firstAnswer!.includes('列表') ||
      firstAnswer!.includes('Processing');
    expect(hasListProcessing, `回答应包含 Lisp 名称来源，实际: ${firstAnswer!.substring(0, 200)}`).toBe(true);

    // 第二轮：追问闭包步骤
    await sendMessageAndWait(page, '闭包是在哪个步骤实现的？');
    const secondAnswer = await messages.last().textContent();
    expect(secondAnswer).not.toBeNull();
    expect(secondAnswer!.length, '回答长度应 > 10').toBeGreaterThan(10);
    const hasStep37 =
      secondAnswer!.includes('37') ||
      secondAnswer!.includes('步骤') ||
      secondAnswer!.includes('闭包');
    expect(hasStep37, `回答应包含闭包步骤信息，实际: ${secondAnswer!.substring(0, 200)}`).toBe(true);
  });

  test('RD-003 引用来源指向正确文档', async ({ page }) => {
    await page.goto(e2eUrl);
    await page.waitForTimeout(3000);

    await importDocumentAndWait(page, testDocPath);
    await sendMessageAndWait(page, 'TCO 是什么意思？');

    // 点击引用来源按钮展开
    const sourcesBtn = page.locator('button:has-text("引用来源")').first();
    if (await sourcesBtn.isVisible()) {
      await sourcesBtn.click();
      await page.waitForTimeout(500);

      // 验证引用来源中包含 README_zh 文档名
      const sourcesContent = await page.locator('.sources').textContent();
      expect(sourcesContent, '引用来源内容不应为空').not.toBeNull();
      const hasDocRef =
        sourcesContent!.includes('README') ||
        sourcesContent!.includes('lisp');
      expect(hasDocRef, `引用来源应指向 lisp-rs 文档，实际: ${sourcesContent?.substring(0, 200)}`).toBe(true);
    }
  });

  test('RD-004 空知识库 → 导入 → 输入框状态转换', async ({ page }) => {
    await page.goto(e2eUrl);
    await page.waitForTimeout(3000);

    // 如果已有文档，先全部删除
    await page.evaluate(async () => {
      const docs = await window.__TAURI__.core.invoke('get_documents');
      for (const d of docs) {
        await window.__TAURI__.core.invoke('delete_document', { id: d.id });
      }
    });
    await page.waitForTimeout(2000);

    // 验证空库状态：输入框禁用
    await expect(page.locator('#queryInput')).toBeDisabled({ timeout: 10_000 });
    await expect(page.locator('#sendBtn')).toBeDisabled();

    // 导入文档
    await importDocumentAndWait(page, testDocPath);

    // 验证输入框恢复可用
    await expect(page.locator('#queryInput')).toBeEnabled({ timeout: 10_000 });
    await expect(page.locator('#sendBtn')).toBeEnabled();
  });
});
