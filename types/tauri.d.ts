/**
 * EchoMind 前端 Tauri 全局类型声明。
 *
 * 声明 window.__TAURI__ 的结构，使 tsc --checkJs 能对
 * ui/src/ipc.js 及其他模块中的 Tauri 调用进行类型检查。
 */

/** Tauri IPC 核心接口 */
interface TauriCore {
  invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown>;
}

/** Tauri 事件监听接口 */
interface TauriEvent {
  listen(
    name: string,
    cb: (event: { payload: unknown }) => void,
  ): Promise<() => void>;
}

/** Tauri 对话框接口 */
interface TauriDialog {
  open(options?: Record<string, unknown>): Promise<string | string[] | null>;
  save(options?: Record<string, unknown>): Promise<string | null>;
}

/** Tauri 窗口接口 */
interface TauriWindow {
  getCurrentWindow(): {
    isFullscreen(): Promise<boolean>;
    onResized(cb: () => void): void;
  };
}

/** Tauri opener 接口 */
interface TauriOpener {
  openUrl(url: string): Promise<void>;
  openPath(path: string): Promise<void>;
}

/** window.__TAURI__ 完整结构 */
interface TauriRuntime {
  core: TauriCore;
  event: TauriEvent;
  dialog: TauriDialog;
  window?: TauriWindow;
  opener?: TauriOpener;
}

/** 确认对话框选项 */
interface ConfirmDialogOptions {
  title?: string;
  body?: string;
  confirmText?: string;
  cancelText?: string;
  danger?: boolean;
}

interface Window {
  /** Tauri 运行时（代码中假设始终存在，E2E 测试通过 mock 注入） */
  __TAURI__: TauriRuntime;
  /** E2E 测试用看门狗超时覆盖 */
  __ECHOMIND_WATCHDOG_TIMEOUT_MS__?: number;
  /** E2E 测试用 Tauri mock 标记 */
  __ECHOMIND_MOCK__?: boolean;
  /** 右键菜单重新生成调用（REQ-IX-001） */
  __echomindSend?: () => Promise<void>;
  /** E2E 测试 mock 状态对象 */
  __mock?: { state?: Record<string, unknown> };
  /** 确认对话框全局函数（E2E 测试 + 内联 onclick 需要） */
  showConfirmDialog?: (options: ConfirmDialogOptions | string) => Promise<boolean>;
  /** 设置面板全局函数（内联 onclick 需要） */
  onRemoveWatchedFolder?: (path: string) => void;
  selectLocalModel?: (filename: string, name?: string) => Promise<void> | void;
  deleteLocalModel?: (filename: string, name?: string, sizeBytes?: number) => Promise<void> | void;
  downloadLocalModel?: (url: string, filename: string, displayName?: string) => Promise<void> | void;
  saveSamplingParams?: () => void;
  resetSamplingParams?: () => void;
  exportDiagnostics?: () => void;
  exportLogs?: () => void;
  /** 自定义嵌入模型全局函数（REQ-VEC-014，内联 onclick 需要） */
  switchToCustomModel?: (name: string) => Promise<void>;
  deleteCustomModel?: (name: string) => Promise<void>;
  onUploadCustomModel?: () => Promise<void>;
  /** 下载管理器全局函数（内联 onclick 需要） */
  pauseDownload?: (filename: string) => Promise<void>;
  resumeDownload?: (filename: string) => Promise<void>;
  cancelDownload?: (filename: string) => Promise<void>;
  startDownload?: (url: string, filename: string, displayName?: string) => Promise<void>;
  resumeAllRecovery?: () => Promise<void>;
  discardAllRecovery?: () => Promise<void>;
  clearCompletedDownloads?: () => void;
/** 主题切换全局函数（设置面板调用，REQ-UI-011） */
setTheme?: (theme: string) => Promise<void>;
/** REQ-RAG-051: 退出演示模式（LLM 配置保存后调用） */
exitDemoModeIfActive?: () => Promise<void>;
  /** P0-3：强制检索重生成标记（chat_done 后恢复混合搜索状态） */
  __echomindRegenForceSearch?: boolean;
}

// Global ambient declarations (no export = script scope)
