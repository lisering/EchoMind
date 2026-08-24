// L3 真实 GUI 启动器：拉起 tauri-driver → 等待 4444 就绪 → 运行 WebdriverIO → 清理。
// 数据目录经 ECHOMIND_DATA_DIR 隔离（临时目录），模型缓存经 ECHOMIND_MODEL_CACHE 共享（CI 缓存友好）。
import { spawn } from 'node:child_process';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import fs from 'node:fs';

const platform = os.platform();
if (platform === 'darwin') {
  console.error(
    '[E2E] tauri-driver 不支持 macOS（WKWebView 无 WebDriver 实现，上游硬限制）。\n' +
      '      真实 GUI 层请在 Linux / Windows 或 CI 运行；本机请使用 npm run test:bridge（桥接层）。',
  );
  process.exit(2);
}

const dataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'echomind-e2e-data-'));
const modelCache = path.resolve(process.cwd(), '../target/fastembed-cache');
fs.mkdirSync(modelCache, { recursive: true });

console.log(`[E2E] 数据目录(隔离): ${dataDir}`);
console.log(`[E2E] 模型缓存(共享): ${modelCache}`);

const env = {
  ...process.env,
  ECHOMIND_DATA_DIR: dataDir,
  ECHOMIND_MODEL_CACHE: modelCache,
};

const tauriDriver = spawn('tauri-driver', [], { stdio: 'inherit', env });

async function waitPort(port, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const ok = await new Promise((resolve) => {
      const sock = net.connect(port, '127.0.0.1');
      sock.once('connect', () => {
        sock.end();
        resolve(true);
      });
      sock.once('error', () => resolve(false));
    });
    if (ok) return;
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error('tauri-driver 4444 端口等待超时');
}

try {
  await waitPort(4444, 20000);
  const wdio = spawn('npx', ['wdio', 'run', 'wdio.conf.mjs'], {
    stdio: 'inherit',
    shell: true,
    env,
  });
  const code = await new Promise((resolve) => wdio.on('exit', resolve));
  process.exitCode = code ?? 1;
} finally {
  tauriDriver.kill();
  fs.rmSync(dataDir, { recursive: true, force: true });
}
