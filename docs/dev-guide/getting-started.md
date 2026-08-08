# 开发者入门

> **状态**：活跃 | **最后审阅**：2026-07-25

---

## 1. 前置条件

| 工具 | 最低版本 | 用途 |
|---|---|---|
| Rust | ≥1.85 (stable 或 nightly) | edition 2024 |
| Node.js | ≥18 | E2E 测试 + 前端构建工具 |
| Tauri CLI | v2 | 桌面应用壳 |

```bash
# 检查 Rust 工具链
rustc --version  # 应 ≥ 1.85

# 安装 Tauri CLI（如未安装）
cargo install tauri-cli --version "^2.0"
```

## 2. 获取代码并首次构建

```bash
git clone <repo-url>
cd EchoMind

# 安装 Git Hooks（一次性）
git config core.hooksPath .githooks

# 首次构建（较慢，因为 ML 依赖需要 opt-level=3 编译）
cargo build

# 运行热重载开发模式
cargo tauri dev
```

> ⚠️ **首次构建慢**：`[profile.dev.package."*"] opt-level = 3` 使 fastembed/ort/tokenizers 在开发模式下也可用。首次编译约 5-10 分钟，增量编译快。

## 3. 运行测试

```bash
# Rust 全量测试
cargo test

# 单个 crate 测试
cargo test -p echomind-core

# 单个测试
cargo test <test_name>

# 显示输出
cargo test -- --nocapture

# Lint（含铁律五 deny lint）
cargo clippy --all-targets -- -D warnings

# 格式检查
cargo fmt --check

# E2E 测试（零网络，使用 Mock）
cd e2e-tests && npm install && ECHOMIND_E2E_TEST=1 npx playwright test

# 真实 LLM 测试（需要 API Key 环境变量）
# 需要：ANTHROPIC_AUTH_TOKEN / BASE_URL / CLAUDE_CODE_SUBAGENT_MODEL

# 混沌测试语料生成
python3 tests/fixtures/generate_corpus.py
```

## 4. 项目结构

```
EchoMind/
├── crates/
│   ├── models/          # 领域契约（纯数据结构）
│   ├── core/            # 业务逻辑（trait + impl）
│   ├── infra/           # 适配器（SQLite, fastembed, OpenAI）
│   └── tauri-app/       # Tauri 壳 + 14 IPC 命令
├── ui/                  # 前端单文件 SPA（本地打包，禁止 CDN）
│   ├── index.html
│   └── vendor/          # marked.js, DOMPurify, highlight.js, Tailwind
├── docs/                # ← 你在寻找文档时看这里
├── e2e-tests/           # Playwright E2E 测试
├── tests/fixtures/      # 测试语料（混沌测试数据）
├── .github/workflows/   # CI 流水线
├── .githooks/           # Git Hooks（pre-commit + commit-msg）
├── .catpaw/rules/       # 铁律规则（跨会话持久化）
├── PROJECT_RULES.md     # 项目宪法（最高约束）
├── AGENTS.md            # Agent 操作手册
└── CLAUDE.md            # Claude Code 指南
```

## 5. 关键文件速查

| 文件 | 内容 |
|---|---|
| `crates/tauri-app/src/lib.rs` | Tauri 命令注册 + `run()` (14 IPC 命令) |
| `crates/tauri-app/src/commands.rs` | IPC 命令实现 + `*_inner` 测试复用函数 |
| `crates/tauri-app/src/state.rs` | AppState（存储、配置、懒加载 embedder、中断令牌） |
| `crates/core/src/lib.rs` | 端口 Trait 定义（Loader, Splitter, Embedder, Storage, LLMProvider, Retriever） |
| `crates/core/src/chat.rs` | ChatEngine（RAG 查询编排） |
| `crates/core/src/import.rs` | ImportService（文件导入 + 索引 + 嵌入） |
| `crates/infra/src/sqlite_storage.rs` | SQLite + WAL 模式 + 向量 BLOB 存储 |
| `crates/infra/src/local_embedder.rs` | fastembed/ONNX 嵌入（384 维） |
| `crates/infra/src/openai_provider.rs` | OpenAI 兼容 SSE 流式 |
| `crates/models/src/lib.rs` | 数据模型定义 |
| `ui/index.html` | 前端 SPA |

## 6. E2E 测试模式说明

`ECHOMIND_DATA_DIR` 环境变量可重定向数据目录（隔离测试数据）：
```bash
ECHOMIND_DATA_DIR=/tmp/echomind-test cargo tauri dev
```

`ECHOMIND_E2E_TEST=1` 环境变量启用 Mock 模式：
- `e2e-tests/tests/bridge/tauri-stub.js` 注入 `window.__TAURI__` mock
- 测试在普通 Chromium 中运行（非 Tauri 窗口）

## 7. 调试技巧

```bash
# 只编译检查（不生成二进制）
cargo check -p tauri-app

# 运行时日志
RUST_LOG=debug cargo tauri dev

# 查看数据库内容
sqlite3 <app_data_dir>/echomind.db

# 查看前端 console
# 在 Tauri 开发窗口中按 F12 或右键 → 检查元素
```

## 8. 常见问题

**Q: 构建时报 "edition 2024 is unstable"**
A: 运行 `rustup update stable` 或 `rustup toolchain install nightly`。

**Q: E2E 测试报 "window.__TAURI__ is undefined"**
A: 确保设置了 `ECHOMIND_E2E_TEST=1` 环境变量。

**Q: clippy 报 unwrap_used deny**
A: 生产代码严禁 unwrap/expect/panic。用 `?` 传播或 `match` 显式处理。测试代码不受限。

**Q: 首次 chat 命令很慢**
A: 懒加载 embedder 首次调用需下载 ONNX 模型（~30MB）。后续调用有缓存。
