// E2E 富内容可视化渲染（REQ-VIZ-001/004、REQ-SEC-002）。
// TC-VIZ-001: Mermaid flowchart 渲染为 SVG
// TC-VIZ-002: 流式期间 mermaid 代码块占位提示
// TC-VIZ-003: Mermaid 语法错误优雅提示
// TC-VIZ-004: 暗色主题图表可读性
// TC-VIZ-006: 渲染 SVG 不含 <script> 标签
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, sendMessage, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';
test.describe('TC-VIZ-001~006 富内容可视化渲染', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    // 打开知识库弹框并导入文档（新 UI 中 #docList 在 KB Modal 内）
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/echomind-e2e.md'] }),
    );
    await page.locator('#docList [data-doc-name]').first().waitFor({ timeout: 5000 });
    await page.locator('#kbCloseBtn').click();
    await expect(page.locator('#kbModal')).toBeHidden();
  });

  test('TC-VIZ-001 Mermaid flowchart 渲染为 SVG', async ({ page }) => {
    // 设置 Mermaid flowchart token 序列
    const mermaidTokens = await page.evaluate(() => window.__mock.mermaidTokens());
    await page.evaluate((tokens) => window.__mock.setCustomTokens(tokens), mermaidTokens);

    await sendMessage(page, '画一个流程图');
    await waitForStreamDone(page);

    // chat_done 后应渲染为 SVG，非纯文本代码块
    const mermaidRendered = page.locator('#chatArea .mermaid-rendered').last();
    await expect(mermaidRendered).toBeVisible({ timeout: 10000 });

    // SVG 元素应存在
    const svg = mermaidRendered.locator('svg');
    await expect(svg).toHaveCount(1);

    // 不应是纯文本 code block（mermaid 代码块应被替换）
    const mermaidCodeBlock = page.locator('#chatArea pre code[class*="mermaid"]').last();
    await expect(mermaidCodeBlock).toHaveCount(0);
  });

  test('TC-VIZ-002 流式期间 mermaid 代码块占位提示', async ({ page }) => {
    const mermaidTokens = await page.evaluate(() => window.__mock.mermaidTokens());
    await page.evaluate((tokens) => window.__mock.setCustomTokens(tokens), mermaidTokens);

    await sendMessage(page, '画一个流程图');

    // 流式过程中：应出现 .mermaid-source 占位元素和「图表渲染中…」提示
    const mermaidSource = page.locator('#chatArea .mermaid-source').last();
    await expect(mermaidSource).toBeVisible({ timeout: 5000 });

    const placeholder = mermaidSource.locator('.mermaid-placeholder');
    await expect(placeholder).toBeVisible();
    await expect(placeholder).toContainText('图表渲染中');

    // 等待流结束
    await waitForStreamDone(page);
  });

  test('TC-VIZ-003 Mermaid 语法错误优雅提示', async ({ page }) => {
    const invalidTokens = await page.evaluate(() => window.__mock.mermaidInvalidTokens());
    await page.evaluate((tokens) => window.__mock.setCustomTokens(tokens), invalidTokens);

    await sendMessage(page, '画一个有语法错误的图');
    await waitForStreamDone(page);

    // 应显示错误提示，不崩溃
    const mermaidError = page.locator('#chatArea .mermaid-error').last();
    await expect(mermaidError).toBeVisible({ timeout: 10000 });
    await expect(mermaidError).toContainText('图表语法错误');

    // 保留源码供用户查看
    const sourcePre = mermaidError.locator('pre');
    await expect(sourcePre).toBeVisible();
  });

  test('TC-VIZ-004 暗色主题图表可读性', async ({ page }) => {
    const mermaidTokens = await page.evaluate(() => window.__mock.mermaidTokens());
    await page.evaluate((tokens) => window.__mock.setCustomTokens(tokens), mermaidTokens);

    await sendMessage(page, '画一个流程图');
    await waitForStreamDone(page);

    const mermaidRendered = page.locator('#chatArea .mermaid-rendered').last();
    await expect(mermaidRendered).toBeVisible({ timeout: 10000 });

    // SVG 应存在且可见
    const svg = mermaidRendered.locator('svg');
    await expect(svg).toBeVisible();

    // 暗色主题下 SVG 不应有白色背景（背景应透明或深色）
    const svgBg = await svg.evaluate((el) => {
      const style = getComputedStyle(el);
      return { background: style.background, backgroundColor: style.backgroundColor };
    });
    // background 不应为纯白色
    expect(svgBg.backgroundColor, 'SVG 背景不应为白色').not.toBe('rgb(255, 255, 255)');

    // SVG 内应有可见的 path/line/text 元素（图表内容）
    const pathCount = await svg.locator('path, line, text, rect, circle, polygon').count();
    expect(pathCount, 'SVG 应包含图表元素').toBeGreaterThan(0);
  });

  test('TC-VIZ-006 渲染 SVG 不含 <script> 标签', async ({ page }) => {
    const xssTokens = await page.evaluate(() => window.__mock.mermaidXssTokens());
    await page.evaluate((tokens) => window.__mock.setCustomTokens(tokens), xssTokens);

    await sendMessage(page, 'XSS 图表测试');
    await waitForStreamDone(page);

    // 等待渲染完成（可能成功渲染或报错，两种情况都不应有 <script>）
    await page.waitForTimeout(2000);

    // 检查整个 chatArea 的 HTML 不含 <script> 标签
    const html = await page.locator('#chatArea .md').last().innerHTML();
    expect(html, '渲染输出不得包含 <script> 标签').not.toContain('<script');
    expect(html, '渲染输出不得包含 onerror 事件').not.toContain('onerror');
    expect(html, '渲染输出不得包含 javascript: 协议').not.toContain('javascript:');
  });
});

test.describe('TC-VIZ-005~005d KaTeX 数学公式渲染', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    // 打开知识库弹框并导入文档（新 UI 中 #docList 在 KB Modal 内）
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/echomind-e2e.md'] }),
    );
    await page.locator('#docList [data-doc-name]').first().waitFor({ timeout: 5000 });
    await page.locator('#kbCloseBtn').click();
    await expect(page.locator('#kbModal')).toBeHidden();
  });

  test('TC-VIZ-005 KaTeX 行内公式渲染（含 mhchem 化学方程式）', async ({ page }) => {
    const tokens = await page.evaluate(() => window.__mock.katexInlineTokens());
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);

    await sendMessage(page, '什么是质能方程');
    await waitForStreamDone(page);

    // chat_done 后应渲染出 KaTeX 公式（.katex 类由 KaTeX 库生成）
    // 3 个行内公式：$E = mc^2$、$c$、$\ce{H2O}$
    const mdEl = page.locator('#chatArea .md').last();
    await expect(mdEl.locator('.katex')).toHaveCount(3, { timeout: 10000 });

    // 渲染的公式不应包含原始 $...$ 文本（已被替换为 KaTeX HTML）
    const html = await mdEl.innerHTML();
    expect(html).toContain('katex');
    // 不应有未渲染的 .katex-pending（chat_done 后应全部渲染或报错）
    const pending = await mdEl.locator('.katex-pending').count();
    expect(pending, 'chat_done 后不应有未渲染的 katex-pending').toBe(0);
  });

  test('TC-VIZ-005b KaTeX 块级公式居中渲染', async ({ page }) => {
    const tokens = await page.evaluate(() => window.__mock.katexBlockTokens());
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);

    await sendMessage(page, '什么是定积分');
    await waitForStreamDone(page);

    // 块级公式应有 .katex-block 容器
    const blockEl = page.locator('#chatArea .katex-block').last();
    await expect(blockEl).toBeVisible({ timeout: 10000 });

    // 块级公式应居中显示（text-align: center）
    const textAlign = await blockEl.evaluate((el) => getComputedStyle(el).textAlign);
    expect(textAlign, '块级公式应居中显示').toBe('center');

    // 内部应有 KaTeX 渲染的公式
    const katexEl = blockEl.locator('.katex');
    await expect(katexEl).toHaveCount(1);
  });

  test('TC-VIZ-005c KaTeX 语法错误优雅提示', async ({ page }) => {
    const tokens = await page.evaluate(() => window.__mock.katexInvalidTokens());
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);

    await sendMessage(page, '渲染一个有语法错误的公式');
    await waitForStreamDone(page);

    // 应显示错误提示，不崩溃
    const errorEl = page.locator('#chatArea .katex-error').last();
    await expect(errorEl).toBeVisible({ timeout: 10000 });
    await expect(errorEl).toContainText('公式语法错误');
  });

  test('TC-VIZ-005d KaTeX 渲染不含 XSS 载荷', async ({ page }) => {
    const tokens = await page.evaluate(() => window.__mock.katexXssTokens());
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);

    await sendMessage(page, 'XSS 公式测试');
    await waitForStreamDone(page);

    // 等待渲染完成
    await page.waitForTimeout(2000);

    // 检查整个 chatArea 的 HTML 不含 <script> 标签
    const html = await page.locator('#chatArea .md').last().innerHTML();
    expect(html, '渲染输出不得包含 <script> 标签').not.toContain('<script');
    expect(html, '渲染输出不得包含 onerror 事件').not.toContain('onerror');
    expect(html, '渲染输出不得包含 javascript: 协议').not.toContain('javascript:');
  });
});

test.describe('TC-VIZ-007~007b Chart.js 数据图表渲染（REQ-VIZ-003）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    // 打开知识库弹框并导入文档（新 UI 中 #docList 在 KB Modal 内）
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/echomind-e2e.md'] }),
    );
    await page.locator('#docList [data-doc-name]').first().waitFor({ timeout: 5000 });
    await page.locator('#kbCloseBtn').click();
    await expect(page.locator('#kbModal')).toBeHidden();
  });

  test('TC-VIZ-007 Markdown 表格→图表切换 + 数据一致（AC-1/AC-3）', async ({ page }) => {
    // 设置含 Markdown 表格的 token 序列
    const tokens = await page.evaluate(() => window.__mock.chartTableTokens());
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);

    await sendMessage(page, '展示季度销售数据');
    await waitForStreamDone(page);

    const mdEl = page.locator('#chatArea .md').last();

    // AC-1: 表格旁出现「切换图表视图」按钮
    const toggleBtn = mdEl.locator('.chart-toggle');
    await expect(toggleBtn).toBeVisible({ timeout: 10000 });
    await expect(toggleBtn).toContainText('切换图表视图');

    // Markdown 表格已渲染
    const table = mdEl.locator('table').first();
    await expect(table).toBeVisible();

    // 点击切换图表视图
    await toggleBtn.click();

    // 图表容器出现，canvas 存在
    const chartContainer = mdEl.locator('.chart-container');
    await expect(chartContainer).toBeVisible({ timeout: 5000 });
    const canvas = chartContainer.locator('canvas');
    await expect(canvas).toHaveCount(1);

    // 表格已隐藏（切换为图表视图）
    await expect(table).toBeHidden();

    // 按钮文案切换为「切换表格视图」
    await expect(toggleBtn).toContainText('切换表格视图');

    // AC-3: 图表数据与原表格数据一致（通过 Chart.getChart 读取实例数据）
    const chartData = await canvas.evaluate((cv) => {
      const inst = (window as any).Chart.getChart(cv);
      if (!inst) return null;
      return {
        type: inst.config.type,
        labels: inst.data.labels as string[],
        datasets: (inst.data.datasets as any[]).map((ds) => ({
          label: ds.label as string,
          data: [...ds.data] as number[],
        })),
      };
    });
    expect(chartData, 'Chart.js 实例应存在').not.toBeNull();
    // 默认渲染柱状图（首个类型按钮自动点击）
    expect(chartData!.type).toBe('bar');
    // 表头首列「季度」后的列名作为 labels
    expect(chartData!.labels).toEqual(['产品A', '产品B']);
    // 每个数据行（季度）映射为一个 dataset，数据值与表格单元格一致
    expect(chartData!.datasets).toEqual([
      { label: 'Q1', data: [120, 80] },
      { label: 'Q2', data: [150, 95] },
      { label: 'Q3', data: [180, 110] },
      { label: 'Q4', data: [200, 130] },
    ]);

    // 再次点击恢复表格视图（切换可逆）
    await toggleBtn.click();
    await expect(mdEl.locator('.chart-container')).toHaveCount(0);
    await expect(table).toBeVisible();
    await expect(toggleBtn).toContainText('切换图表视图');
  });

  test('TC-VIZ-007b 柱/折/饼三种图表类型可切换（AC-2）', async ({ page }) => {
    const tokens = await page.evaluate(() => window.__mock.chartTableTokens());
    await page.evaluate((t) => window.__mock.setCustomTokens(t), tokens);

    await sendMessage(page, '展示季度销售数据');
    await waitForStreamDone(page);

    const mdEl = page.locator('#chatArea .md').last();
    const toggleBtn = mdEl.locator('.chart-toggle');
    await expect(toggleBtn).toBeVisible({ timeout: 10000 });

    // 切换到图表视图
    await toggleBtn.click();
    const chartContainer = mdEl.locator('.chart-container');
    await expect(chartContainer).toBeVisible({ timeout: 5000 });

    // AC-2: 类型切换栏含三种类型：柱状图/折线图/饼图
    const typeBtns = chartContainer.locator('.chart-type-bar button');
    await expect(typeBtns).toHaveCount(3);
    await expect(typeBtns.nth(0)).toContainText('柱状图');
    await expect(typeBtns.nth(1)).toContainText('折线图');
    await expect(typeBtns.nth(2)).toContainText('饼图');

    const canvas = chartContainer.locator('canvas');
    const chartType = () =>
      canvas.evaluate((cv) => (window as any).Chart.getChart(cv)?.config.type);

    // 默认渲染柱状图（首个按钮自动点击 + active 高亮）
    await expect(typeBtns.nth(0)).toHaveClass(/active/);
    expect(await chartType()).toBe('bar');

    // 切换折线图
    await typeBtns.nth(1).click();
    await expect(typeBtns.nth(1)).toHaveClass(/active/);
    await expect(typeBtns.nth(0)).not.toHaveClass(/active/);
    expect(await chartType()).toBe('line');

    // 切换饼图
    await typeBtns.nth(2).click();
    await expect(typeBtns.nth(2)).toHaveClass(/active/);
    expect(await chartType()).toBe('pie');

    // 切回柱状图（往返可逆）
    await typeBtns.nth(0).click();
    await expect(typeBtns.nth(0)).toHaveClass(/active/);
    expect(await chartType()).toBe('bar');
  });
});
