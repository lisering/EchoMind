# CHANGELOG v3.1.0 — 全面 UI/UX 优化 + 性能提升 + 历史债务清算

> 发布日期：2026-08-24 ｜ 计划文档：`docs/architecture/V3.1_ATOMIC_PLAN.md` ｜ 评估报告：`docs/PROJECT_ASSESSMENT_2026-08-24.md`

本版本以「全面复盘、清除历史债务」为主线，源自三路深度调研（前端 58 模块架构审计 / 历史债务盘点 / 性能瓶颈分析），按四阶段原子计划（P1 清债 → P2 性能 → P3 i18n → P4 重构）实施。

---

## 🔴 修复（P0 级）

- **输入区 Toggle 功能坏死修复**：S09 IPC 合并后 `set_hybrid_search`/`set_agent_enabled` 已从后端注销，但 input-toggles 仍调用旧命令名——真实构建中点击「混合搜索 / Agent 模式」会乐观更新后立即回滚。现迁移至 `update_setting` 统一入口。同类活体幻影共修复 3 处（perf-settings 压缩比读取、chat-render 文档列表刷新）。
- **E2E mock 盲区铲除**：清除 tauri-stub.js 中 28 个后端从未注册的死 mock（历史上「11 个幻影调用逃过测试」的复发土壤），36 处 E2E 调用点迁移到 `update_setting`。
- **About 面板版本号错误**：硬编码 `1.6.0` → 构建期从 tauri.conf.json 注入 `__APP_VERSION__`。
- **顶栏下拉被侧栏遮挡**：`#topBar` 层叠上下文（z=0）低于 sidebar（fixed z=20），工具菜单内按钮无法点击 → topBar 提升至 z=30。

## ⚡ 性能

- **流式渲染限频**：rAF 合帧之上增加最小帧间隔（≈15fps，>32KB 文本降至 ≈8fps），消除 token 高频到达时的 CPU 空转
- **hljs 高亮 + 彩虹括号流式豁免**：纳入既有 skipHeavy 通道（KaTeX 先例模式），chat_done 后一次性增强——长代码回答不再掉帧
- **导入进度合流刷新**：doc-status-changed 触发的全量列表刷新加 500ms trailing debounce，批量导入 50 文件的 ~200 次 IPC + DOM 重建降为每静默窗口 ≤1 次
- **启动并行化**：boot 的 4 个独立 IPC（embedder 状态/设置/演示模式/Pro 状态）Promise.all 并行；Tauri 就绪守卫前置避免主题/i18n 重复初始化
- **会话列表分批渲染**：激活死常量 CONV_INITIAL_RENDER_LIMIT=50，超出部分经「加载更多」续渲
- **检索/嵌入热路径埋点**：vector_search 与 embed_batch 增加 tracing debug 计时（路径选择/批大小/耗时）

## 🌐 国际化与交互

- **三语言包 100% 对齐**：en/zh-CN/ja 补齐 111 个缺失 key（ja 此前缺 47 个）
- 隐私状态文案与 LLM 再生成指令模板（简洁/详细模式）i18n 化
- 书签消息导航去重：4 处复制回调统一为 `createBookmarkNavigator`
- Esc 关闭静态面板改为数据驱动注册表（新增面板不再需要改 keyboard.js if 链）
- 会话/文档列表项键盘可达（tabindex + Enter/Space，WCAG 2.1.1）

## 🏗️ 结构优化（P4）

- **chat-render ⇄ chat 循环依赖解除**：send/sendFromEdit 经 `registerChatActions` 单向注入
- **长会话两段式渐进渲染**：首屏只渲染最近 20 条，「查看更早消息」按批 prepend 并补偿滚动位置
- **main.js 上帝模块拆分**：2033 行 → ~1000 行；拆出 `kb-panel.js`（文档面板 444 行）与 `conversation-list.js`（会话列表 + 渐进渲染 ~560 行）；boot() 作为组装层保留
- chat.js 的 /web、/agent、强制检索临时标志并入 state.js store
- 导入流水线乒乓经技术论证关闭（ONNX 已全核运行，CPU 密集阶段并行无收益）

## 🧹 历史债务清算

- 仓库卫生：移除被 git 跟踪的运行日志（e2e-output.txt 等）、AI 工具私有状态（.reasonix）、弃用的 figma-plugin；7 个根目录 CHANGELOG 归档至 docs/changelog/
- 死测试清理：TC-DL-006（功能已删）、E2E-AUDIT-001b 空占位 skip
- bench_real_import_phases 标记 `#[ignore]` + 语料路径参数化（离线环境不再失败）
- 3 个僵尸磁盘命令补齐前端三件套（新 REQ-ERR-007：存储空间卡片 + 缓存清理）
- 新增可选 CI workflow `real-data-e2e.yml`（手动触发，真实 Tauri + ONNX + LLM 全链路验证，关闭 V5 体系性漏洞）
- 修复 S94 设置 Tab 化遗留的 4 个存量 E2E 失败（hybrid-search/ui-tools-menu/v1.6 About）

## 📚 文档

- `docs/architecture/V3.1_ATOMIC_PLAN.md`（新建）：四阶段计划与实施记录
- `docs/PROJECT_ASSESSMENT_2026-08-24.md`（新建）：项目综合评估报告
- SRS v19 changelog + REQ-ERR-007；SOFTWARE_ASSET §11 更新为 61 模块；ipc-commands.md §2.58 磁盘空间管理

## ✅ 回归验证

| 项 | 结果 |
|---|---|
| Rust Free | 1699 passed, 0 failed |
| Vitest | 1826 passed (93 files) |
| E2E bridge | 1602 passed, 0 failed |
| TypeScript | 0 errors |
| Clippy (Free) + fmt | 零警告 |

零新增运行时依赖。
