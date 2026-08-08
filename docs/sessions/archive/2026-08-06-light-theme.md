# Session Archive: 2026-08-06 v1.1 S46 浅色主题切换

## 概述

实现 EchoMind 浅色主题切换功能（REQ-UI-011），通过 CSS 变量覆盖实现零组件代码改动。支持暗色/浅色/跟随系统三种模式，主题偏好持久化到 settings 表。

## 实现内容

### 1. `ui/styles/tokens.css` — 浅色主题 CSS 变量集

- 新增 `[data-theme="light"]` 选择器，覆盖 ~40 个颜色变量（Surface/Border/Text/Accent/Semantic/Bg 别名/消息组件令牌）
- 新增 `--accent-text` 变量：暗色主题 `var(--accent)`，浅色主题 `#0369A1`（sky-700，在白色上 6.4:1）
- 新增 `[data-theme="light"] body { color-scheme: light; }` — 浏览器原生控件浅色
- 新增 `@media (prefers-color-scheme: light) [data-theme="system"]` — 跟随系统自动切换
- 新增浅色滚动条样式
- 对比度修复：`--text-quaternary` 从 `#64748B` 改为 `#475569`（在 surface-3 上 6.2:1）
- 对比度修复：`--success` 从 `#16A34A` 改为 `#15803D`（green-700，5.2:1）

### 2. `ui/src/main.js` — 主题管理函数

- `applyTheme(theme)` — 同步应用主题到 DOM：设置 `dataset.theme` + 双 rAF 过渡禁用防闪烁 + Mermaid 主题同步 + state 更新
- `setTheme(theme)` — 异步持久化：先 `applyTheme` 再 `invoke('set_theme')`，降级 `localStorage`
- `initTheme()` — 启动时从后端或 localStorage 恢复主题 + 注册 `prefers-color-scheme` 变化监听器
- 在 `boot()` 函数中 `initI18n()` 之前调用 `initTheme()`

### 3. `ui/src/settings.js` — 外观区域 UI

- `initThemeSwitcher()` — 读取当前主题、高亮活跃按钮、绑定点击事件
- 设置面板新增「外观」区域（暗色/浅色/跟随系统三选一分段控件 + SVG 图标）

### 4. `ui/index.html` — 设置面板 HTML

- 在语言切换区域之后新增外观区域 HTML（`#themeSwitcher` 容器 + 3 个 `.theme-btn` 按钮）

### 5. `crates/tauri-app/src/commands.rs` — IPC 命令

- `get_theme` / `get_theme_inner` — 从 settings 表读取 `ui.theme`，默认返回 `"dark"`
- `set_theme` / `set_theme_inner` — 持久化到 settings 表 `ui.theme`

### 6. `crates/tauri-app/src/lib.rs` — IPC 注册

- 在 `generate_handler!` 宏中注册 `get_theme` 和 `set_theme`

### 7. i18n 文案

- `ui/locales/en.json`: `theme_section: "Appearance"`, `theme_dark: "Dark"`, `theme_light: "Light"`, `theme_system: "Follow System"`
- `ui/locales/zh-CN.json`: `theme_section: "外观"`, `theme_dark: "暗色"`, `theme_light: "浅色"`, `theme_system: "跟随系统"`

### 8. E2E mock + 测试

- `tauri-stub.js` 新增 `get_theme` / `set_theme` mock（localStorage 持久化）
- `theme.spec.ts` 5 个 E2E 测试：
  - TC-UI-LIGHT-001: 切换到浅色主题后 `data-theme === 'light'`
  - TC-UI-LIGHT-002: 浅色主题下 backgroundColor 为浅色（RGB > 230）
  - TC-UI-LIGHT-003: 主题持久化（刷新后保持）
  - TC-UI-LIGHT-004: 跟随系统模式自动切换（`colorScheme: 'light'` 模拟）
  - TC-UI-LIGHT-005: 浅色主题对比度 axe-core 零 color-contrast violation

### 9. CSS 对比度修复

- `components.css` 新增 `[data-theme="light"] .text-accent { color: var(--accent-text); }` — 覆盖 Tailwind 硬编码 accent 色
- `components.css` 新增 `@media (prefers-color-scheme: light) [data-theme="system"] .text-accent` 同样覆盖
- `.empty-state-import-btn` 的 `color` 从 `var(--accent)` 改为 `var(--accent-text)`

### 10. 类型声明

- `types/tauri.d.ts` — Window 接口新增 `setTheme?: (theme: string) => Promise<void>`
- `ui/src/state.js` — EchoState JSDoc 新增 `@property {string} theme` + initialState 新增 `theme: 'dark'`

## 文档同步

- `docs/api/settings-reference.md` — 新增 `ui.theme` 设置键
- `docs/SRS_v1.0.md` — REQ-UI-011 状态 Draft → Implemented + v4.3.0 changelog
- `docs/architecture/V1.1_ATOMIC_PLAN.md` — S46 验收标准 checkbox 勾选 + 附录 D 状态更新

## 验证结果

| 检查项 | 结果 |
|---|---|
| `cargo clippy --all-targets -- -D warnings` (Free) | 零警告 |
| `cargo clippy --all-targets --features pro -- -D warnings` (Pro) | 零警告 |
| `cargo fmt --check` | 通过 |
| `npx tsc --noEmit` | 0 errors |
| `node scripts/build-ui.mjs` | 成功（10901 行） |
| E2E `theme.spec.ts` | 5 passed, 0 failed |
| `./sync-to-github.sh --commit` | 7 files changed, 484 insertions |

## 关键设计决策

1. **CSS 变量覆盖而非组件代码改动**：利用已有 CSS 变量体系，浅色主题只需覆盖变量值，零组件代码改动
2. **`--accent-text` 变量**：Tailwind CDN JIT 生成的 `.text-accent` 使用硬编码 `#38BDF8`，不随 CSS 变量切换。新增 `--accent-text` 变量 + CSS 覆盖规则解决此问题
3. **双 rAF 过渡禁用**：主题切换时临时添加 `body.change-theme` 类禁用所有过渡动画，双 rAF 后恢复，避免闪烁瀑布
4. **后端持久化降级 localStorage**：Tauri 环境通过 `set_theme` IPC 持久化到 settings 表；E2E 测试环境降级到 localStorage
5. **`prefers-color-scheme` 监听器**：system 模式下自动跟随系统主题切换
6. **WCAG AA 对比度**：浅色主题下 `--text-quaternary` 和 `--success` 颜色加深以确保 ≥ 4.5:1
