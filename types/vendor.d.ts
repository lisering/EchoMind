/**
 * EchoMind 前端 vendor 库全局类型声明。
 *
 * 这些库通过 <script> 标签在 index.html 中加载，
 * 不在 npm 管理范围内，需要手动声明全局类型。
 */

/** marked.js — Markdown 解析器 */
declare const marked: {
  (src: string, options?: Record<string, unknown>): string;
  parse(src: string, options?: Record<string, unknown>): string;
  setOptions(options: Record<string, unknown>): void;
  use(extensions: Record<string, unknown>): void;
};

/** DOMPurify — HTML 消毒器 */
declare const DOMPurify: {
  sanitize(html: string, config?: Record<string, unknown>): string;
};

/** mermaid — 图表渲染器 */
declare const mermaid: {
  initialize(config: Record<string, unknown>): void;
  run(options?: Record<string, unknown>): Promise<void>;
  render(id: string, text: string): Promise<{ svg: string }>;
};

/** highlight.js — 代码高亮器 */
declare const hljs: {
  highlightElement(el: HTMLElement): void;
  highlight(code: string, options: { language: string }): { value: string };
  getLanguage(name: string): unknown;
};

/** KaTeX — 数学公式渲染器 */
declare const katex: {
  render(tex: string, element: HTMLElement, options?: Record<string, unknown>): void;
  renderToString(tex: string, options?: Record<string, unknown>): string;
};

/** Chart.js — 图表库 */
declare const Chart: {
  new (ctx: CanvasRenderingContext2D | HTMLCanvasElement, config: Record<string, unknown>): unknown;
  defaults: Record<string, unknown>;
  _echomindDefaults?: Record<string, unknown> | boolean;
};

/** D3.js v7 — 数据可视化库（force-directed graph） */
declare const d3: any;

// Global ambient declarations (no export = script scope)
