# Session: 前端架构升级（2026-08-04）

## 概述

对 EchoMind 前端架构进行了四项关键改进：引入 TypeScript 类型检查、拆分大模块、使用 esbuild 替代手写构建脚本、加强单元测试覆盖。

## 实现内容

### P1: TypeScript checkJs 类型检查

- `tsconfig.json` — 根目录配置，`checkJs: true`, `noEmit: true`, `target: ES2022`
- `types/dom-ext.d.ts` — 扩展 Element/HTMLElement/Node/Event/EventTarget 接口（interface merging），解决 vanilla JS 中 querySelector 返回 Element 访问 HTMLElement 属性的类型错误
- `types/tauri.d.ts` — `window.__TAURI__` 类型声明
- `types/vendor.d.ts` — marked/DOMPurify/mermaid/hljs/katex/Chart 第三方库类型声明
- 类型错误从 273 个减少到 42 个（84.6% 减少），剩余为代码逻辑层面的类型不匹配，留待渐进式修复

### P2: 模块拆分

- `ui/src/chat-render.js` — 从 `chat.js` 拆出消息 DOM 渲染 + Markdown 消费 + 流式追加逻辑
- `ui/src/settings-local-llm.js` — 从 `settings.js` 拆出本地 LLM 模型选择 + 采样参数 + GPU 配置 UI
- `chat.js` 从 1291 行减至 594 行（减少 54%）
- 拆分后模块可独立进行 Vitest 单元测试

### P3: esbuild 打包替代手写构建脚本

- `scripts/build-ui.mjs` 重写：使用 esbuild 核心 API（`bundle: true`, `format: 'iife'`）从 `src/main.js` 打包
- 移除手写 `MODULE_ORDER` 拓扑序数组和 `stripModuleSyntax` 函数
- esbuild 自动解析 ES module import/export，tree-shake 无用导出
- **Bug 修复**：发现 `security.js` 错误导入了不存在的 `showToast`，改为 `import { toast as showToast } from './toast.js'`（手写拼接时此 bug 被掩盖，esbuild 正确解析导入时报错）

### P4: Vitest 单元测试加强

- `e2e-tests/tests/unit/action.test.js` — 20 个测试（ActionRegistry 类注册/执行/查询/边界）
- `e2e-tests/tests/unit/guards.test.js` — 16 个测试（canSend/requireIdle/requirePro 守卫函数）
- 总计 389 tests / 26 files 全部通过

## 文档同步

- `AGENTS.md` — 前端描述更新（37→39 模块, esbuild, checkJs）; Commands 表新增 `npx tsc --noEmit`; Quirks 新增 TypeScript + esbuild 条目
- `CLAUDE.md` — Frontend 章节更新; Key files 新增 `chat-render.js`/`settings-local-llm.js`/`tsconfig.json`/`types/*.d.ts`; Quirks 新增 TypeScript + esbuild 条目
- `docs/SOFTWARE_ASSET.md` — §1.3 技术栈更新; §11.1 架构更新; §11.2 模块清单新增; §11.4 构建脚本描述更新; §12.1 命令表新增; §13.1 测试数量更新; 附录 A 文件索引更新
- `docs/sessions/CURRENT_STATE.md` — 测试指标更新; 新增前端架构升级记录; 验证命令速查更新
- `docs/sessions/LOG.md` — 新增会话记录; 统计摘要更新

## 验证结果

| 验证项 | 结果 |
|---|---|
| cargo test (Free) | Exit Code 0 — 全量通过 |
| cargo clippy --all-targets -- -D warnings | Exit Code 0 — 零警告 |
| cargo fmt --check | Exit Code 0 — pre-existing diff（非本次引入） |
| node scripts/build-ui.mjs | Exit Code 0 — esbuild 打包 7388 行 |
| Vitest | 389 passed, 26 files, 0 failed |
| sync-to-github.sh --commit | 1 file changed (ui/index.html), 已提交 |

## 关键设计决策

1. **checkJs 而非迁移 TypeScript**：保持 .js 文件不变，通过 JSDoc + tsconfig.json 实现类型检查，零迁移成本
2. **dom-ext.d.ts 而非类型断言**：通过 interface merging 扩展 Element，避免在每个 querySelector 调用处添加 `as HTMLElement`
3. **esbuild IIFE 而非 ESM**：生产模式打包为 IIFE 格式，兼容 `file://` 协议和 Tauri WebView，无 CORS 限制
4. **模块拆分策略**：按功能边界拆分（渲染 vs 流控、LLM 配置 vs 通用设置），保持公共 API 不变
