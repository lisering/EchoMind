/**
 * EchoMind Mermaid 自定义打包入口。
 *
 * 从 npm mermaid v11 ESM 源码打包，利用 esbuild tree-shaking + 压缩。
 * 替代 vendor/mermaid.min.js (3.4MB UMD 预构建)，预期减小 15-30%。
 *
 * 运行时通过 lazy-loader.js 按需加载（与原 UMD 方式一致）。
 * 加载后 window.mermaid 全局对象可用，API 完全兼容。
 */
import mermaid from 'mermaid';
window.mermaid = mermaid;
