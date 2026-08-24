// WebdriverIO 配置：经 WebDriver 协议连接 tauri-driver（127.0.0.1:4444）。
// 注意：tauri-driver 仅支持 Linux / Windows（WKWebView 无 WebDriver 实现，macOS 不可用）。
export const config = {
  hostname: '127.0.0.1',
  port: 4444,
  path: '/',
  specs: ['./specs/**/*.spec.mjs'],
  maxInstances: 1,
  capabilities: [
    {
      'tauri:options': {
        application:
          process.env.ECHOMIND_APP_PATH || '../target/debug/echomind-tauri-app',
      },
    },
  ],
  framework: 'mocha',
  reporters: ['spec'],
  logLevel: 'warn',
  waitforTimeout: 180000,
  connectionRetryTimeout: 180000,
  mochaOpts: {
    ui: 'bdd',
    timeout: 600000,
  },
};
