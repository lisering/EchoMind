# NEXT SESSION 提示词 — V3.1 P4 结构性重构收尾

```markdown
你是 EchoMind 项目的 AI 开发工程师。请先读取以下文件恢复上下文：
1. `docs/sessions/CURRENT_STATE.md`（权威状态快照）
2. `docs/architecture/V3.1_ATOMIC_PLAN.md`（V3.1 计划与 P4 剩余项实施方案）
3. `.catpaw/rules/system-constraints.md`（铁律规则）
4. 本文件（任务衔接）

## 上 Session 摘要（2026-08-24）

V3.1 全面优化的 P1 清债 / P2 性能 / P3 i18n 已全部完成并验证，P4 完成前两项：
- ✅ P4-1 流式渲染限频（chat-render.js scheduleRender，15fps + 长文 8fps）
- ✅ P4-2 chat-render⇄chat 循环依赖解除（registerChatActions 单向注册）

**验证基线**：Rust Free 1699 passed / Vitest 1826 passed (93 files) /
E2E bridge 1566 passed 0 failed / tsc 0 errors。变更在工作区未提交。

## 本 Session 任务：P4 剩余四项（按序实施，每项独立验证）

### 任务 1: P4-5 长会话渐进渲染（方案已定型，优先做）
`ui/src/main.js loadConversation()`。
方案要点：
- 先跑全量循环**收集渲染单元列表**（turn_group 活跃版本 或 独立消息），不渲染 DOM
- 只对尾部最近 MSG_INITIAL_RENDER_LIMIT=20 个单元执行真实渲染
- 聊天区顶部插入「查看更早消息」哨兵按钮，点击后 prepend 更早批次
  （insertBefore(chatArea.firstChild)，prepend 前记录 scrollHeight 差值补偿滚动位置）
- 注意 turn_tree 已全量构建（setTurnTree 不变），只改 DOM 渲染段
- 测试：session.spec.ts + 手动验证百条消息会话

### 任务 2: P4-4 initChatEventListeners reducer 化
`ui/src/chat.js:722-1061`（339 行）。
- `_tempWebSearch/_tempAgent/window.__echomindRegenForceSearch` 三个闭包标志并入 state.js
- 15+ listen 回调改为「setState → 订阅侧渲染」或至少抽出独立 handler 函数
- 保持行为不变（E2E chat 系列 spec 是回归安全网）

### 任务 3: P4-6 导入管线流水线乒乓
`crates/tauri-app/src/commands/import.rs:71` 的串行 for 循环改为双任务重叠
（文件 A 的 embed_batch 与文件 B 的解析/分块并发）。注意 cancel_flag 边界与
doc-status-changed 事件顺序（状态机不能乱序）。用 import_bench 对比前后吞吐。

### 任务 4: P4-3 main.js 拆分（最后做，风险最大）
按四刀切：boot()（~550行→boot.js）/ conversation-list（loadConversations+
_renderConvItem+置顶+拖拽排序→conversation-list.js）/ kb-panel（loadDocuments+
筛选排序标签→kb-panel.js）/ 其余留 main.js。预计牵扯 ~40 个符号的 export/import。

## 约束提醒
- 每完成一项跑 L0-L2 验证：npx tsc --noEmit && node scripts/build-ui.mjs &&
  cd e2e-tests && npm run test:unit；P4-6 另加 cargo clippy + cargo test
- 全部完成后跑全量 E2E bridge（15 分钟）确认零回归
- 铁律七：同步 SOFTWARE_ASSET §11 / frontend-modules.md
- 铁律十一：完成后 ./sync-to-github.sh --commit
- 铁律十二：更新 CURRENT_STATE.md + LOG.md + 生成本文件的下期版本
- 发版决策：若本 session 完成 P4 全部，可评估是否升级版本号 v3.0.0→v3.1.0
  （需同步 8 crate Cargo.toml + tauri.conf.json + CHANGELOG_v3.1.md）

## 关键文件索引
| 文件 | 用途 |
|---|---|
| ui/src/main.js | boot/会话/KB 面板（1924 行，待拆） |
| ui/src/chat.js | send/initChatEventListeners（1061 行） |
| ui/src/chat-render.js | 渲染层（已解除循环依赖） |
| crates/tauri-app/src/commands/import.rs | 导入管线 |
| e2e-tests/tests/bridge/tauri-stub.js | IPC mock（已清 28 死 mock，勿再添加旧命令名） |
```
