// E2E-UI-001 全链路冒烟（L3 真实 GUI：WebdriverIO + tauri-driver + Mock LLM SSE）。
// 六组断言与用户指令逐条对应：向导 → 配置注入 → 导入列表 → 流式对话 → 停止中断 → 文档删除。
import http from 'node:http';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

/** 启动 Mock LLM：非流式返回 pong；流式按 200ms/token 发送 SSE（含 rust 代码块），供增长与中断断言。 */
function startMockLlm() {
  const server = http.createServer((req, res) => {
    if (req.method === 'POST' && req.url === '/v1/chat/completions') {
      let body = '';
      req.on('data', (c) => (body += c));
      req.on('end', () => {
        const payload = JSON.parse(body || '{}');
        if (!payload.stream) {
          res.writeHead(200, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ choices: [{ message: { content: 'pong' } }] }));
          return;
        }
        res.writeHead(200, {
          'Content-Type': 'text/event-stream',
          'Cache-Control': 'no-cache',
          Connection: 'keep-alive',
        });
        const tokens = [
          '好的，', '这是', '流式', '回答', '：', '\n\n```rust\n', 'fn main() {\n',
          '    println!("hi");\n', '}\n', '```\n', '正在', '继续', '输出', '更多', '内容', '……',
        ];
        let i = 0;
        const timer = setInterval(() => {
          if (i >= tokens.length) {
            res.write('data: [DONE]\n\n');
            clearInterval(timer);
            res.end();
            return;
          }
          res.write(
            `data: ${JSON.stringify({ choices: [{ delta: { content: tokens[i++] } }] })}\n\n`,
          );
        }, 200);
        req.on('close', () => clearInterval(timer));
      });
      return;
    }
    res.writeHead(404);
    res.end();
  });
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      resolve({ server, url: `http://127.0.0.1:${server.address().port}` });
    });
  });
}

describe('E2E-UI-001 EchoMind 全链路冒烟', function () {
  let mockLlm;
  let fixturePath;
  let fixtureName;

  before(async () => {
    mockLlm = await startMockLlm();
    // 测试文档：内容与提问高度重合，确保越过检索阈值（REQ-RAG-003）
    fixtureName = `echomind-e2e-${Date.now()}.md`;
    fixturePath = path.join(os.tmpdir(), fixtureName);
    fs.writeFileSync(
      fixturePath,
      '# EchoMind 支持格式\n\nEchoMind 支持 md、txt、pdf 格式。EchoMind 支持哪些格式？答：md、txt、pdf（PDF 为 Pro 功能）。\n',
    );
  });

  after(async () => {
    mockLlm.server.close();
    fs.rmSync(fixturePath, { force: true });
  });

  it('01 首次启动向导 UI 可见', async () => {
    await expect($('#wizard')).toBeDisplayed();
  });

  it('02 注入 Mock 配置后重启进入主界面', async () => {
    await browser.execute((url) => {
      return window.__TAURI__.core.invoke('update_llm_config', {
        config: { api_key: 'sk-e2e-mock', base_url: url, model: 'mock-llm' },
      });
    }, mockLlm.url);
    // 刷新窗口让应用走真实启动流程（get_settings → 已配置 → 主界面）
    await browser.refresh();
    await expect($('#app')).toBeDisplayed();
  });

  it('03 导入文件后文档列表渲染文件名', async () => {
    await browser.execute(
      (p) => window.__TAURI__.core.invoke('import_files', { paths: [p], isPro: true }),
      fixturePath,
    );
    const item = await $('#docList [data-doc-name$=".md"]');
    // 首次嵌入模型加载可能耗时（共享缓存命中后秒级）
    await item.waitForExist({ timeout: 300000 });
    await expect(item).toHaveText(fixtureName);
  });

  it('04 流式对话：气泡出现、内容增长、代码块与复制按钮', async () => {
    await $('#queryInput').setValue('EchoMind 支持哪些格式？');
    await $('#sendBtn').click();

    const mdList = await $$('#chatArea .md');
    const lastMd = mdList[mdList.length - 1];
    await lastMd.waitForExist({ timeout: 60000 });

    // 流式增长断言：两次采样必须变长
    const len1 = (await lastMd.getText()).length;
    await browser.pause(700);
    const len2 = (await lastMd.getText()).length;
    expect(len2).toBeGreaterThan(len1);

    // 等待流结束（停止按钮隐藏即回到空闲态）
    await browser.waitUntil(async () => !(await $('#stopBtn').isDisplayed()), {
      timeout: 120000,
      timeoutMsg: '流式输出未在预期时间内结束',
    });
    const codes = await $$('#chatArea pre code');
    expect(codes.length).toBeGreaterThan(0);
    const copyBtns = await $$('#chatArea .copy-btn');
    expect(copyBtns.length).toBeGreaterThan(0);
  });

  it('05 停止生成：输出中断并出现「已中断」标记', async () => {
    await $('#queryInput').setValue('EchoMind 支持哪些格式？请再回答一次');
    await $('#sendBtn').click();
    await $('#stopBtn').waitForDisplayed({ timeout: 60000 });
    await browser.pause(500);
    await $('#stopBtn').click();
    await browser.waitUntil(async () => await $('#sendBtn').isDisplayed(), {
      timeout: 60000,
      timeoutMsg: '停止后输入框未恢复空闲态',
    });
    const badge = await $('*=⏹ 已中断');
    await expect(badge).toBeExisting();
  });

  it('06 删除文档：DOM 元素从列表中消失', async () => {
    const item = await $('#docList [data-doc-name$=".md"]');
    await item.waitForExist({ timeout: 15000 });
    await item.moveTo();
    const delBtn = await item.$('button');
    await delBtn.waitForClickable({ timeout: 10000 });
    await delBtn.click();
    await browser.waitUntil(
      async () => (await (await $('#docList [data-doc-name$=".md"]')).isExisting()) === false,
      { timeout: 15000, timeoutMsg: '文档 DOM 未从列表移除' },
    );
  });
});
