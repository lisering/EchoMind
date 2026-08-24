/**
 * EchoMind 前端 DOM 类型扩展。
 *
 * vanilla JS 中经常用 querySelector 返回的 Element 访问
 * HTMLElement 特有属性（dataset / style / value / disabled 等）。
 * 此文件通过 interface merging 扩展 Element，使 tsc --checkJs
 * 不报误，同时保留 JSDoc 类型标注的其他收益。
 *
 * 注意：只添加 Element 上缺失但代码中通过 Element 访问的属性。
 * 不要重复声明标准 DOM lib 已有的属性（如 classList / className /
 * querySelector 等），否则会导致类型合并冲突。
 */

/**
 * ThinkingPanel 句柄类型（createThinkingPanel 返回值）。
 * 声明在此处以供 HTMLElement._thinkingPanel 使用。
 */
interface ThinkingPanelHandle {
  container: HTMLElement;
  update(text: string, phase?: string): void;
  appendReasoning(text: string): void;
  renderReasoning(text: string): void;
  finalizeReasoning(): void;
appendStage(text: string): void;
/** REQ-RAG-052: 追加 Agent 步骤卡片 */
appendAgentStep(step: { step_type: string; content: string; tool?: string; input?: string; iteration: number }): void;
/** REQ-RAG-052: 设置 Agent 进度条 */
setAgentProgress(current: number, max: number): void;
setMsgId(msgId: string | null): void;
  isExpanded(): boolean;
  collapse(): void;
  expand(): void;
  setComplete(text?: string): Promise<void>;
  reset(): void;
  clearContent(): void;
  getReasoning(): string | null;
  startThinking(): void;
  markFirstTokenReceived(): void;
  isFirstTokenReceived(): boolean;
  getThinkDuration(): number | null;
  ensureMinLoadingDelay(): Promise<void>;
}

/** 扩展 Element，添加代码中通过 Element 访问的 HTMLElement 特有属性 */
interface Element {
  dataset: DOMStringMap;
  style: CSSStyleDeclaration;
  value: string;
  disabled: boolean;
  checked: boolean;
  isContentEditable: boolean;
  placeholder: string;
  title: string;
  offsetHeight: number;
  offsetWidth: number;
  innerText: string;
  focus(): void;
  blur(): void;
  click(): void;
  onclick: ((this: Element, ev: MouseEvent) => unknown) | null;
  onchange: ((this: Element, ev: Event) => unknown) | null;
}

/** 扩展 HTMLElement，添加表单元素属性 + 自定义属性 */
interface HTMLElement {
  onchange: ((this: HTMLElement, ev: Event) => unknown) | null;
  /** 自定义属性：消息编辑版本历史 */
  _versions?: unknown[];
  /** 自定义属性：轮播版本数据 */
  _carouselVersions?: unknown[];
  /** 自定义属性：思维链面板引用 */
  _thinkingPanel?: ThinkingPanelHandle;
  /** 自定义属性：引用来源数据 */
  _sources?: unknown;
  /** 自定义属性：重新生成轮播引用 */
  _regenCarousel?: HTMLElement | null;
  /** 自定义属性：当前审计问题 */
  currentQuestion?: string;
  /** 自定义属性：本地模型选择回调 */
  selectLocalModel?: (filename: string) => void;
  /** 自定义属性：采样参数保存回调 */
  saveSamplingParams?: () => void;
  /** 自定义属性：采样参数重置回调 */
  resetSamplingParams?: () => void;
  /** 自定义属性：移除监听文件夹回调 */
  onRemoveWatchedFolder?: (path: string) => void;
}

/** 扩展 Node，添加 Element 常用方法（parentNode.querySelector 等场景） */
interface Node {
  querySelectorAll(selector: string): NodeListOf<Element>;
  querySelector(selectors: string): Element | null;
}

/** 扩展 Event，添加 KeyboardEvent / MouseEvent 属性（event.target 访问时需要） */
interface Event {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  clientX: number;
  clientY: number;
  target: EventTarget & Element;
  currentTarget: EventTarget & Element;
  preventDefault(): void;
  stopPropagation(): void;
}

/** 扩展 EventTarget，添加 Element 常用方法（event.target 类型兼容） */
interface EventTarget {
  closest(selector: string): Element | null;
  value?: string;
  isContentEditable?: boolean;
  classList?: DOMTokenList;
  dataset?: DOMStringMap;
}

// ============================================================
// Web Speech API 类型声明（REQ-RAG-034 / REQ-RAG-035）
// SpeechRecognition / speechSynthesis 在 Tauri WebView 中可能不存在，
// 所有属性声明为可选以支持 typeof window.XXX !== 'undefined' 检测。
// ============================================================

/** SpeechRecognition 识别结果项（单个备选） */
interface SpeechRecognitionAlternative {
  transcript: string;
  confidence: number;
}

/** SpeechRecognition 识别结果行（含多个备选） */
interface SpeechRecognitionResult {
  length: number;
  item(index: number): SpeechRecognitionAlternative;
  [index: number]: SpeechRecognitionAlternative;
  isFinal: boolean;
}

/** SpeechRecognition 识别结果列表 */
interface SpeechRecognitionResultList {
  length: number;
  item(index: number): SpeechRecognitionResult;
  [index: number]: SpeechRecognitionResult;
}

/** SpeechRecognition 结果事件 */
interface SpeechRecognitionEvent extends Event {
  resultIndex: number;
  results: SpeechRecognitionResultList;
}

/** SpeechRecognition 错误事件 */
interface SpeechRecognitionErrorEvent extends Event {
  error: string;
  message: string;
}

/** SpeechRecognition 实例接口 */
interface SpeechRecognitionInstance extends EventTarget {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  maxAlternatives: number;
  start(): void;
  stop(): void;
  abort(): void;
  onresult: ((event: SpeechRecognitionEvent) => void) | null;
  onerror: ((event: SpeechRecognitionErrorEvent) => void) | null;
  onend: (() => void) | null;
  onstart: (() => void) | null;
}

/** SpeechRecognition 构造函数 */
interface SpeechRecognitionConstructor {
  new (): SpeechRecognitionInstance;
}

/** 扩展 Window，添加 Web Speech API 可选属性 + PDF 导出属性 + Webkit AudioContext */
interface Window {
  SpeechRecognition?: SpeechRecognitionConstructor;
  webkitSpeechRecognition?: SpeechRecognitionConstructor;
  speechSynthesis?: SpeechSynthesis;
  SpeechSynthesisUtterance?: { new (text?: string): SpeechSynthesisUtterance };
  /** Webkit AudioContext 别名（旧版 Safari / WKWebView 兼容） */
  webkitAudioContext?: typeof AudioContext;
  /** PDF 导出 mock 计数器（E2E 测试用，REQ-EXP-005） */
  __printMockCalled?: number;
  /** PDF 导出最近打印 HTML（E2E 测试用，REQ-EXP-005） */
  __lastPrintHtml?: string | null;
  /** 开发者模式快捷键处理器安装标志（S5 审计 P0-6） */
  _devModeHandlerInstalled?: boolean;
  /** 导出对话为 PDF（全局暴露供内联 onclick / E2E 测试调用，REQ-EXP-005） */
  exportConversationToPdf?: (conversationId: string, title?: string) => Promise<void>;
/** 导出文档为 PDF（全局暴露供内联 onclick / E2E 测试调用，REQ-EXP-005） */
exportDocumentToPdf?: (docId: string) => Promise<void>;
/** 导出对话为 HTML（全局暴露供内联 onclick / E2E 测试调用，REQ-EXP-007） */
exportConversationToHtml?: (conversationId: string, title?: string) => Promise<void>;
/** 导出文档为 HTML（全局暴露供内联 onclick / E2E 测试调用，REQ-EXP-007） */
exportDocumentToHtml?: (docId: string) => Promise<void>;
/** 日期格式化工具（E2E 测试用，REQ-I18N-002） */
__formatDate?: (timestamp: number | string | Date) => string;
/** 相对时间格式化工具（E2E 测试用，REQ-I18N-002） */
__formatRelativeTime?: (timestamp: number | string | Date) => string;
/** 文件大小格式化工具（E2E 测试用，REQ-I18N-003） */
__formatFileSize?: (bytes: number) => string;
/** 数字格式化工具（E2E 测试用，REQ-I18N-003） */
__formatNumber?: (num: number) => string;
/** 关于面板打开/关闭（E2E 测试用，REQ-HELP-003） */
__openAboutPanel?: () => void;
__closeAboutPanel?: () => void;
/** KB 统计仪表盘打开/关闭（E2E 测试用，REQ-KB-003 v1.5） */
__openKbStats?: () => Promise<void>;
__closeKbStats?: () => void;
/** 帮助面板打开/关闭（E2E 测试用，REQ-HELP-001 v1.5） */
__openHelpPanel?: (tab?: string) => void;
__closeHelpPanel?: () => void;
/** 会话列表刷新（E2E 测试用，REQ-IX-002 v1.16） */
__loadConversations?: () => Promise<void>;
/** 会话列表缓存（书签模块查询标题用，REQ-RAG-047） */
__echomindConversations?: Array<{ id: string; title: string }>;
/** 刷新侧栏书签列表（REQ-RAG-053 消息级书签按钮调用） */
__refreshBookmarks?: () => void;
/** 更新检查结果（REQ-HELP-004 S87，E2E 测试注入 mock 数据用） */
__updateInfo?: { has_update: boolean; current_version: string; latest_version: string; release_notes: string | null; download_url: string | null };
/** 导出全量数据备份（REQ-EXP-002 v1.17） */
exportBackup?: () => Promise<void>;
/** 从备份恢复数据（REQ-EXP-003 v1.17） */
importBackup?: () => Promise<void>;
}

/** 应用版本号（scripts/build-ui.mjs 构建期从 tauri.conf.json 注入） */
declare const __APP_VERSION__: string;
