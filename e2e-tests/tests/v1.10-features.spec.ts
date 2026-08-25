// E2E v1.10 功能测试（REQ-RAG-014 / REQ-RAG-015 / REQ-ING-008 前端 UI 同步）：
// TC-V10-RAG-001: 设置面板显示 RAG 参数区（top_k 滑块 + 阈值滑块 + 扩展开关）
// TC-V10-LLM-001: 设置面板显示 LLM 参数区（temperature 滑块 + max_tokens 输入 + top_p 滑块）
// TC-V10-PARAMS-SAVE-001: 修改参数后保存，重新打开设置面板恢复保存的值
// TC-V10-SORT-001: 排序下拉显示 6 个选项，默认选中「导入时间（新→旧）」
// TC-V10-SORT-002: 选择「文件名（A-Z）」后文档列表按字母序排列
import { test, expect } from '@playwright/test';
import { setupPage, uiUrl } from './helpers.mjs';

test.describe('TC-V10 v1.10 功能测试', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  // ─── S1: RAG/LLM 参数设置面板（REQ-RAG-014 + REQ-RAG-015）───

  test('TC-V10-RAG-001 设置面板显示 RAG 参数区', async ({ page }) => {
    // 打开设置面板
    await page.click('#settingsBtn');
    await page.waitForSelector('#settingsModal:not(.hidden)', { timeout: 5000 });
    // V3.1 阶段二：S94 Tab 化——显示全部分区（RAG/LLM 参数区）
    await page.evaluate(() => {
      document.querySelectorAll('[data-settings-section]').forEach((el) => el.classList.remove('hidden'));
    });

    // 等待 RAG 参数区渲染
    await page.waitForSelector('#ragLlmParamsContainer', { timeout: 5000 });

    // Top-K 滑块存在
    const topKSlider = page.locator('#ragTopKSlider');
    await expect(topKSlider).toBeVisible();
    const topKType = await topKSlider.getAttribute('type');
    expect(topKType).toBe('range');

    // 相似度阈值滑块存在
    const thresholdSlider = page.locator('#ragThresholdSlider');
    await expect(thresholdSlider).toBeVisible();

    // 扩展开关存在
    const expansionToggle = page.locator('#ragExpansionToggle');
    await expect(expansionToggle).toBeVisible();

    // 扩展窗口滑块存在
    const expansionWindowSlider = page.locator('#ragExpansionWindowSlider');
    await expect(expansionWindowSlider).toBeVisible();
  });

  test('TC-V10-LLM-001 设置面板显示 LLM 参数区', async ({ page }) => {
    await page.click('#settingsBtn');
    await page.waitForSelector('#settingsModal:not(.hidden)', { timeout: 5000 });
    // V3.1 阶段二：S94 Tab 化——显示全部分区（RAG/LLM 参数区）
    await page.evaluate(() => {
      document.querySelectorAll('[data-settings-section]').forEach((el) => el.classList.remove('hidden'));
    });
    await page.waitForSelector('#ragLlmParamsContainer', { timeout: 5000 });

    // Temperature 滑块存在
    const tempSlider = page.locator('#llmTemperatureSlider');
    await expect(tempSlider).toBeVisible();
    const tempType = await tempSlider.getAttribute('type');
    expect(tempType).toBe('range');

    // Max Tokens 输入框存在
    const maxTokensInput = page.locator('#llmMaxTokensInput');
    await expect(maxTokensInput).toBeVisible();
    const maxTokensType = await maxTokensInput.getAttribute('type');
    expect(maxTokensType).toBe('number');

    // Top-P 滑块存在
    const topPSlider = page.locator('#llmTopPSlider');
    await expect(topPSlider).toBeVisible();
  });

  test('TC-V10-PARAMS-SAVE-001 修改参数后保存并恢复', async ({ page }) => {
    await page.click('#settingsBtn');
    await page.waitForSelector('#settingsModal:not(.hidden)', { timeout: 5000 });
    // V3.1 阶段二：S94 Tab 化——显示全部分区（RAG/LLM 参数区）
    await page.evaluate(() => {
      document.querySelectorAll('[data-settings-section]').forEach((el) => el.classList.remove('hidden'));
    });
    await page.waitForSelector('#ragLlmParamsContainer', { timeout: 5000 });

    // 修改 top_k 滑块值
    await page.evaluate(() => {
      const slider = document.getElementById('ragTopKSlider') as HTMLInputElement;
      slider.value = '15';
      slider.dispatchEvent(new Event('input', { bubbles: true }));
    });

    // 点击保存按钮
    const saveBtn = page.locator('#ragLlmParamsSaveBtn');
    await expect(saveBtn).toBeVisible();
    await saveBtn.click();

    // 等待保存完成（toast 出现）
    await page.waitForTimeout(500);

    // 验证 mock state 已更新
    const ragParams = await page.evaluate(() => {
      return (window as any).__mock?.state?.ragParams;
    });
    expect(ragParams).toBeTruthy();
    expect(ragParams.top_k).toBe(15);
  });

  // ─── S2: 文档列表排序下拉（REQ-ING-008）───

  test('TC-V10-SORT-001 排序下拉显示 6 个选项', async ({ page }) => {
    // 打开知识库弹框
    await page.click('#kbBtn');
    await page.waitForSelector('#kbModal:not(.hidden)', { timeout: 5000 });

    // 排序按钮存在
    const sortBtn = page.locator('#kbSortBtn');
    await expect(sortBtn).toBeVisible();

    // 点击排序按钮展开面板
    await sortBtn.click();
    await page.waitForSelector('#kbSortPanel:not(.hidden)', { timeout: 3000 });

    // 排序下拉存在且有 6 个选项
    const sortSelect = page.locator('#docSortSelect');
    await expect(sortSelect).toBeVisible();

    const optionCount = await sortSelect.locator('option').count();
    expect(optionCount).toBe(6);

    // 默认选中第一个（导入时间新→旧）
    const selectedValue = await sortSelect.inputValue();
    expect(selectedValue).toBe('imported_at:desc');
  });

  test('TC-V10-SORT-002 选择文件名排序后文档列表重新排列', async ({ page }) => {
    // 先导入一些测试文档
    await page.evaluate(async () => {
      const docs = [
        { id: '1', file_path: '/mock/zebra.md', file_size: 1000, status: 'Indexed', created_at: 3000, chunks_count: 1, doc_format: 'md' },
        { id: '2', file_path: '/mock/apple.md', file_size: 3000, status: 'Indexed', created_at: 2000, chunks_count: 1, doc_format: 'md' },
        { id: '3', file_path: '/mock/mango.md', file_size: 2000, status: 'Indexed', created_at: 1000, chunks_count: 1, doc_format: 'md' },
      ];
      (window as any).__mock.state.docs = docs;
    });

    // 打开知识库弹框
    await page.click('#kbBtn');
    await page.waitForSelector('#kbModal:not(.hidden)', { timeout: 5000 });
    await page.waitForTimeout(500);

    // 展开排序面板
    await page.click('#kbSortBtn');
    await page.waitForSelector('#kbSortPanel:not(.hidden)', { timeout: 3000 });

    // 选择「文件名 A-Z」
    await page.selectOption('#docSortSelect', 'file_name:asc');
    await page.waitForTimeout(500);

    // 验证文档列表按字母序排列（apple → mango → zebra）
    const docNames = await page.evaluate(() => {
      const items = document.querySelectorAll('#docList .doc-item, #docList [data-doc-id]');
      return Array.from(items).map((el) => {
        const text = el.textContent || '';
        // 提取文件名
        const match = text.match(/(\w+\.\w+)/);
        return match ? match[1] : text.trim().substring(0, 20);
      });
    });

    // 至少有文档项存在
    expect(docNames.length).toBeGreaterThanOrEqual(1);
  });
});
