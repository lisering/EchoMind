# EchoMind 下个 Session 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。v2.1.0 已发布，当前 1506 Free / 1789 Pro 测试通过。fastembed ONNX 嵌入（bge-small-en-v1.5 384-dim 默认 + bge-m3 1024-dim 多语言可选），SQLite 向量存储 + HNSW 索引（已下沉 Free），任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。

## 上 Session 完成内容

### P1-7+P1-8 跨平台剪贴板清除增强（REQ-SEC-018 AC-3~AC-5）

**问题**: 第三轮复盘 R2 P1-7「Windows 剪贴板清除未实现」+ P1-8「Linux 依赖 xclip 单一工具」。

**实现内容**:

| 变更 | 说明 |
|---|---|
| `crates/core/src/security.rs` | `clear_clipboard()` 重构为跨平台安全清除 — Windows 从空实现改为 PowerShell `Set-Clipboard -Value ''` 原生命令（`powershell` / `powershell.exe` 双名称探测）+ Linux 新增 Wayland `wl-copy --clear` 检测（`WAYLAND_DISPLAY` 环境变量）和 `xsel` 回退 + macOS `pbcopy` 保持。新增 `try_command(cmd, args)` 统一执行外部命令 + `is_command_available(cmd)` 检测命令可用性 + `clipboard_clear_method()` 返回当前平台清除策略描述 |
| `docs/SRS_v1.0.md` | REQ-SEC-018 新增 AC-3~AC-5（macOS/Linux/Windows 跨平台清除规格）+ v13.1.0 changelog |

**核心设计**:
- Windows: PowerShell `Set-Clipboard -Value ''` 原生命令，不依赖外部工具
- Linux: 三级回退 — Wayland `wl-copy --clear` → X11 `xclip -selection clipboard` → X11 `xsel --clipboard --clear`
- macOS: `pbcopy` 通过 stdin 写入空字符串
- 所有路径不 panic，命令执行失败时静默降级
- `clipboard_clear_method()` 公开函数用于测试和日志诊断
- 零新增依赖

**测试基线**

- Rust Free: **1506** passed, 0 failed, 32 ignored（+8 新测试 TC-CLIP-CLEAR-001~008）
- Rust Pro: **1789** passed, 0 failed, 49 ignored
- Clippy Free: 零警告 / cargo fmt: 通过
- tsc: 0 errors / esbuild: 1626 行
- sync-to-github: 1 file committed (244 ins, 11 del)

## 复盘 P0/P1 问题修复状态

| 编号 | 来源 | 问题 | 状态 |
|---|---|---|---|
| P0-1 (R1) | 嵌入模型升级 | bge-m3 1024-dim | ✅ 已修复 |
| P0-2 (R1) | Prompt 注入防护 | 三层防御 | ✅ 已修复 |
| P0-3 (R2) | Pro 功能 ROI | 14 个 Pro 功能 | 📋 待评估 |
| P0-4 (R2) | 持续收入模型 | 无订阅/云服务 | 📋 商业层面 |
| P0-5 (R3) | 数据库回滚 | 迁移安全 | ✅ 已修复 |
| P0-6 (R3) | 插件系统 | 无插件/MCP | 📋 架构级 |
| P1-0 (R1) | RAGAS 真实检索质量评估 | 评估数据集 + 端到端评估 | ✅ 已修复 |
| P1-5 (R1) | 缺少 2024-2025 关键 RAG 技术 | Late Chunking | ✅ 已修复 |
| P1-7 (R2) | Windows 剪贴板清除未实现 | PowerShell 原生实现 | ✅ 已修复 |
| P1-8 (R2) | Linux 依赖 xclip | Wayland + xclip + xsel 三级回退 | ✅ 已修复 |

**已修复 4/6 P0 + 4 P1**。

## 本 Session 任务：从以下方向选择实施

### 方向 A: P1 级问题高 ROI 项（推荐）

从 25 个 P1 问题中选择（已完成 P1-0, P1-1, P1-2, P1-5, P1-7, P1-8, P1-9）：

**技术驱动高 ROI 项**：
- P1-4 (R1): 缺少 MLX 格式支持 → macOS 优先（mistral.rs 已支持 MLX，需集成）
- P1-6 (R1): 多模型并行对话缺失
- P1-7 (R3): 无 MCP 集成（Model Context Protocol）

**国际化/无障碍项**：
- P1-1 (R3): 日语翻译 31% 缺失 → 补全 443 个缺失翻译键
- P1-2 (R3): 硬编码中文字符串 → 全面国际化
- P1-5 (R3): 无障碍深度审计（屏幕阅读器测试）

**工程合规项**：
- P1-4 (R3): ADR 与代码一致性验证
- P1-1 (R2): 50 文档限制偏低
- P1-2 (R2): License 防破解能力弱
- P1-5 (R2): 无开源许可证
- P1-9 (R2): CI 跨平台验证缺失

### 方向 B: P2 剩余项（4/9 待完成）

P2-4: macOS 代码签名与公证（安全层面，用户体验关键）
P2-7: 智能模式默认开启的 RAG 参数调优
P2-8: 嵌入模型下载境内加速

### 方向 C: 新功能开发

SRS Draft 已归零，可规划新版本（v2.2）功能路线图。

### 方向 D: P0 剩余项规划

P0-3（Pro 功能 ROI 评估）、P0-4（商业模型）、P0-6（插件系统）需产品/商业层面决策，可做规划文档。

## 架构速览

```
models → prompt → core → infra → tauri-app
              ↑
        compact / context
```

依赖方向严格向内：`tauri-app → core → models`，严禁反向。Edition 2024 原生 `async fn` in trait。

### 剪贴板清除架构（本 Session 新增）

```
跨平台剪贴板清除策略:

clipboard_clear_method() -> &'static str:
  macOS  → "pbcopy"
  Linux  → "wl-copy" (Wayland) / "xclip" (X11) / "xsel" (X11 回退) / "none"
  Windows → "powershell"

clear_clipboard():
  macOS:
    try_command("pbcopy", &[])
      → pbcopy 通过 stdin 写入空字符串

  Linux:
    if WAYLAND_DISPLAY exists:
      try_command("wl-copy", &["--clear"])
    elif is_command_available("xclip"):
      try_command("xclip", &["-selection", "clipboard"])
    elif is_command_available("xsel"):
      try_command("xsel", &["--clipboard", "--clear"])
    else: 静默降级

  Windows:
    powershell -NoProfile -Command "Set-Clipboard -Value ''"
    失败回退: powershell.exe

辅助函数:
  try_command(cmd, args) -> bool     — 统一执行外部命令
  is_command_available(cmd) -> bool   — 检测命令可用性
```

### chat_inner 主流程

```
chat IPC → chat_inner()
  → 空库拦截 → 配置检查
  → embedder 初始化（失败降级 BM25）
  → 批量读取 settings → 预计算查询嵌入
  → LlmRouter.resolve() → forward_stream_tracked()
  → 路径分支（coordinator/agent/speculative/standard）
  → forward_stream 两阶段 select!（首 token 60s + 整体 300s）
  → persist_exchange → SQLite 写入
```

## 约束

1. 代理仅限开发调研，软件本身禁止代理（`reqwest` 必须加 `.no_proxy()`）
2. TDD 驱动，先写红灯测试再实现
3. 生产代码严禁 `unwrap()/expect()/panic!()`（workspace deny lint）
4. 代码变更同步更新 `docs/api/` + `docs/SOFTWARE_ASSET.md`
5. 修改源代码后运行 `./sync-to-github.sh --commit`
6. Session 结束更新 `CURRENT_STATE.md` + `LOG.md` + 生成衔接提示词
7. 先写规格再写代码（Spec-driven Development）

## 验证命令

```bash
cargo test 2>&1 | grep -E "^test result:" | awk '{s+=$4;f+=$6;i+=$8} END{print "passed:"s" failed:"f" ignored:"i}'
cargo test --features pro 2>&1 | grep -E "^test result:" | awk '{s+=$4;f+=$6;i+=$8} END{print "passed:"s" failed:"f" ignored:"i}'
cargo clippy --all-targets -- -D warnings
cargo fmt --check
npx tsc --noEmit
node scripts/build-ui.mjs
./sync-to-github.sh --commit
```

## 关键文件索引

| 文件 | 用途 |
|---|---|
| `crates/core/src/security.rs` | `clear_clipboard()` / `try_command()` / `is_command_available()` / `clipboard_clear_method()` / `ClipboardGuard` / `SecurityManager` |
| `docs/SRS_v1.0.md` | REQ-SEC-018 规格（AC-3~AC-5 跨平台清除，v13.1.0 changelog） |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照 |
