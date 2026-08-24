# EchoMind 僵尸清零 + 借鉴消化 100% Session 提示词

> **复制下方全部内容作为下个 Session 的第一条消息即可开始。**

---

## 项目上下文

EchoMind v1.6.0，Rust + Tauri v2 桌面 RAG 应用。六边形架构，7 crate workspace。197 个 Free IPC 命令 / 209 Pro。58 个前端 JS 模块。工作目录 `/Users/john/freesoft/EchoMind`。

铁律规则见 `.catpaw/rules/system-constraints.md`。需求规格见 `docs/SRS_v1.0.md`。会话状态见 `docs/sessions/CURRENT_STATE.md`。

## 目标

**僵尸率 0%（197 命令全部有前端调用者）+ 借鉴消化度 100%（37 项借鉴全部消化或正式归类）。**

## 当前僵尸命令（4 个，精确确认）

经 grep 全量扫描 `ui/src/*.js` + `tauri-stub.js` 确认，以下 4 个命令在后端注册但前端无任何调用：

| # | 命令 | 门控 | 原因 |
|---|------|------|------|
| 1 | `set_paged_attn` | Pro | PagedAttention 设置无 UI |
| 2 | `get_kernel_mode` | Pro | GEMV 内核模式无显示 |
| 3 | `set_kernel_mode` | Pro | GEMV 内核模式无选择器 |
| 4 | `get_graph_layout` | Free | 图谱布局算法无调用（D3 自管布局） |

## 借鉴消化度缺口（2 项）

5 个来源 37 项借鉴，当前 35 项已消化（直接 UI 或间接被功能消费），剩余 2 项需正式归类：

| # | 借鉴项 | 来源 | 当前状态 | 处置 |
|---|--------|------|----------|------|
| 1 | B12 Snapshot/Revert | OpenCode | ⏸ 暂不实施 | 正式 Deprecated |
| 2 | B13 Plugin V2 | OpenCode | ⏸ 暂不实施 | 正式 Deprecated |

## 任务清单

### S1：接线 4 个僵尸命令

**文件 `ui/src/settings-local-llm.js`**（3 个 Pro 命令）：

1. 新增 `renderPagedAttnSettings(container)` — PagedAttention toggle 开关 + block_size 输入 + gpu_memory_ctx 输入，调用 `ipc.js` 已有的 `localLlmApi.setPagedAttn(enabled, blockSize, gpuMemoryCtx)` 或新增 IPC 封装
2. 新增 `renderKernelModeSettings(container)` — 内核模式选择器（Mistral.rs / Custom GEMV 两选一按钮组），调用 `set_kernel_mode` / `get_kernel_mode`
3. 在 `initLocalLlmSettings()` 末尾调用以上两个渲染函数

**文件 `ui/src/graph-viewer.js`**（1 个 Free 命令）：

4. 在布局切换按钮（已有 force/hierarchical/radial 三个按钮）的点击事件中，调用 `get_graph_layout` 获取后端布局算法建议（当前 D3 自管布局，后端返回布局模式名称用于持久化用户偏好）

**文件 `ui/src/ipc.js`**：

5. 新增 `localLlmApi.getKernelMode()` / `localLlmApi.setKernelMode(mode)` 封装
6. 新增 `graphApi.getLayout()` 封装（如已有 graphApi 则扩展）

**文件 `e2e-tests/tests/bridge/tauri-stub.js`**：

7. 确认 `set_paged_attn` / `get_kernel_mode` / `set_kernel_mode` / `get_graph_layout` 四个 mock handler 已存在（复盘 S4 已添加），如缺失则补充

**文件 `ui/locales/{en,zh-CN}.json`**：

8. 新增 i18n 键：`llm.paged_attn` / `llm.paged_attn_desc` / `llm.block_size` / `llm.gpu_memory_ctx` / `llm.kernel_mode` / `llm.kernel_mode_desc` / `llm.kernel_mistral` / `llm.kernel_custom_gemv`（中英文各 8 键）

**文件 `types/dom-ext.d.ts`**：

9. 如需暴露测试钩子，补充 Window 属性声明

### S2：正式归类 B12/B13

**文件 `docs/architecture/OPENCODE_BORROW_ATOMIC_PLAN.md`**：

1. B12 Snapshot/Revert 状态 `⏸ 暂不实施` → `⛔ Deprecated`，补充废弃理由：EchoMind 是 Tauri 桌面应用，无 OpenCode 的 Web IDE 热重载场景；Tauri 窗口状态已由 `window.{x,y,width,height,maximized}` settings 键持久化，无需快照/回滚机制
2. B13 Plugin V2 状态 `⏸ 暂不实施` → `⛔ Deprecated`，补充废弃理由：EchoMind 当前架构为单体桌面应用，六边形架构的 ports & adapters 已提供足够的扩展性；插件系统会增加安全攻击面（铁律安全要求），且无用户需求驱动

**文件 `docs/SRS_v1.0.md`**：

3. 如有 B12/B13 对应的 Draft REQ，状态改为 Deprecated

### S3：E2E 测试

**新建文件 `e2e-tests/tests/zombie-zero.spec.ts`**：

1. TC-ZOMBIE-001：PagedAttention toggle 可见且可切换
2. TC-ZOMBIE-002：内核模式选择器可见且可切换
3. TC-ZOMBIE-003：图谱布局切换按钮点击后有响应
4. TC-ZOMBIE-004：全量 IPC 命令无僵尸验证（grep 后端注册命令 vs 前端 invoke 调用，断言差集为空）

### S4：文档同步 + GitHub 同步

1. `docs/api/ipc-commands.md` — 确认 4 个命令已有详情（S3 复盘已补充），如缺失则补充
2. `docs/api/frontend-modules.md` — 更新 `settings-local-llm.js` 函数列表（新增 3 个渲染函数）+ `graph-viewer.js` 函数列表
3. `docs/api/settings-reference.md` — 确认 `llm.kernel_mode` 已有（v7 已添加），`llm.paged_attn` 已有
4. `docs/sessions/CURRENT_STATE.md` — 更新僵尸率 0% / 消化度 100%
5. `docs/sessions/LOG.md` — 追加本次 session 记录
6. `./sync-to-github.sh --commit` — 同步到发布仓库

## 验证标准

```bash
# 僵尸率验证（必须输出空）
grep -o 'commands::[a-z_]*' crates/tauri-app/src/lib.rs | sed 's/commands:://' | sort -u > /tmp/all.txt
grep -oh "invoke(['\"\`][a-z_]*" ui/src/*.js e2e-tests/tests/bridge/tauri-stub.js | sed "s/invoke(['\"\`]//" | sort -u > /tmp/wired.txt
comm -23 /tmp/all.txt /tmp/wired.txt  # 必须为空

# 标准验证
npx tsc --noEmit                    # 0 errors
node scripts/build-ui.mjs           # 成功
cargo clippy --all-targets -- -D warnings  # 零警告
cargo fmt --check                   # 通过
cd e2e-tests && npx playwright test tests/zombie-zero.spec.ts --reporter=list  # 全通过
```

## 关键约束

- 铁律五：零 unwrap/expect/panic（workspace deny lint 强制）
- 铁律十三：Tailwind utility classes + design tokens，禁止硬编码 hex
- UI 是构建产物：编辑 `ui/src/*.js` 后运行 `node scripts/build-ui.mjs`
- `settings-local-llm.js` 中 Pro 门控功能在 Free 用户下应显示灰色不可用状态
- `get_graph_layout` 是 Free 命令，非 Pro 门控
