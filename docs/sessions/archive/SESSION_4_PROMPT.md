# Session 4 衔接提示词 — 下阶段方向选择

## 上下文

Session 1-2 完成了前端架构升级（Z-index 分层 + 面板栈 + Focus Trap 栈 + State 补全 + 会话切换关闭面板 + 输入区统一 keydown），Session 3 完成了验证收尾。

### Session 3 验证结果

- **TypeScript**: 0 errors ✅
- **esbuild**: 13580 行 ✅
- **Vitest**: 419 passed, 0 failed ✅（修复 input-toggles.js 3 失败：`if (initialActive)` → 始终 `setState` 同步状态）
- **Rust Free**: 1028 passed, 9 failed (pre-existing), 20 ignored ✅
- **Rust Clippy Free**: 零警告 ✅
- **cargo fmt**: 通过 ✅
- **GitHub 同步**: 5 files, 820 insertions committed ✅
- **E2E 桥接测试**: 运行中（50+ spec 文件，需 30-60min）

### Pre-existing 失败（9 个，与前端改造无关）

Rust 集成测试 9 个失败全部是 pre-existing：
- `tc_edge_003_free_tier_50_doc_limit` — Free 层文档限制
- `tc_lic_003_free_tier_quota_blocked` — 配额限制
- `tc_lic_005_activate_pro_persists_across_restart` — 许可证持久化
- `tc_lic_005b_invalid_license_rejected` — 无效许可证
- `tc_lic_006_deactivate_pro` — Pro 停用
- `tc_pro_gate_001_free_tier_pro_disabled` — Pro 门控
- `tc_pro_gate_003_search_symbols_free_works` — 符号搜索 Pro 门控
- `tc_pro_gate_004_local_mode_free_still_not_pro` — 本地模式 Pro 门控
- `tc_vec_custom_005_free_user_rejected` — 自定义模型 Pro 门控

### 新增文件

- `ui/src/zindex.js` — 统一 Z-index 常量（12 层级）+ zClass()
- `ui/src/panel-stack.js` — 全局面板栈管理器（push/remove/closeTop/peek/isOpen/hasOpen/clear）
- `ui/src/input-keymap.js` — 统一输入 keydown 处理器（斜杠命令 > Enter > 历史 > Escape）
- `ui/src/input-history.js` — 输入历史 + 草稿持久化 + Token 估算

### 文档更新

- `docs/api/frontend-modules.md` — 新增 §11 前端架构基础设施模块
- `AGENTS.md` — 新增 4 个 key files
- `CLAUDE.md` — 新增 4 个 key files
- `docs/sessions/CURRENT_STATE.md` — 更新测试基线 + 前端模块数 50→54
- `docs/sessions/LOG.md` — 追加 Session 1-3 记录
- `docs/SOFTWARE_ASSET.md` — 更新前端架构章节

---

## 推荐的下阶段方向

### 首选：Git 状态清理（0.5 天）

**理由**：
1. 当前 101 个未提交文件（80 modified + 21 untracked），包含 QM 借鉴 Q01-Q11、OpenCode 借鉴 B01-B14、前端架构升级等大量成果
2. 未提交代码有丢失风险（磁盘故障、误删等）
3. 完成后为后续开发提供干净的起点
4. 耗时最短（0.5 天），不阻塞其他方向

**执行步骤**：
1. `git add -A && git status` 检查全部变更
2. 按功能域分组提交（QM 借鉴 / OpenCode 借鉴 / 前端架构 / Rust 改进）
3. `./sync-to-github.sh --push` 推送到 GitHub

### 后续可选方向（清理后选择）

| 方向 | 描述 | 预计工时 | 优先级 |
|---|---|---|---|
| E2E 质量提升 | 修复 9 个 pre-existing Rust 失败 + pre-existing E2E 失败 + 补充测试覆盖 | 2-3 天 | 高 |
| v1.5 新功能 | MCP 协议 / Late Chunking / RAGAS 评估 / 多窗口 | 5-10 天 | 中 |
| 架构优化 | 编译速度 / 依赖更新 / 日文 i18n | 2-3 天 | 中 |
| 生产就绪 | 日志增强 / 错误恢复 / 数据迁移 | 3-5 天 | 中 |
| 商业化准备 | License 购买流程 / 用户文档 / 官网 | 5-10 天 | 低 |

---

## 验证命令速查

```bash
# 前端
npx tsc --noEmit                    # TypeScript 类型检查
node scripts/build-ui.mjs           # esbuild 打包 IIFE
cd e2e-tests && npm run test:unit   # Vitest (419 tests)

# Rust
cargo test                            # Free (1028 passed, 9 pre-existing failed)
cargo test --features pro             # Pro (1094 passed)
cargo clippy --all-targets -- -D warnings          # Free Clippy
cargo clippy --all-targets --features pro -- -D warnings  # Pro Clippy
cargo fmt --check                     # 格式检查

# E2E
cd e2e-tests && npm run test:bridge   # Playwright E2E 桥接测试

# 同步
./sync-to-github.sh --commit         # 同步到发布仓库
./sync-to-github.sh --push           # 推送到 GitHub
```
