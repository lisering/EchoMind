# 测试指南

> **状态**：活跃 | **最后审阅**：2026-07-25

---

## 1. 测试体系总览

| 层级 | 框架 | 运行命令 | 位置 |
|---|---|---|---|
| Rust 单元测试 | `#[test]` | `cargo test` | 各 `src/*.rs` 内嵌 `#[cfg(test)] mod *_tests` |
| Rust 集成测试 | `#[test]` | `cargo test --test integration` | `crates/tauri-app/tests/integration.rs` |
| E2E 测试 | Playwright | `cd e2e-tests && ECHOMIND_E2E_TEST=1 npx playwright test` | `e2e-tests/tests/` |
| 混沌测试语料 | Python 生成 | `python3 tests/fixtures/generate_corpus.py` | `tests/fixtures/` |
| CI 流水线 | GitHub Actions | 自动（push/PR） | `.github/workflows/` |

---

## 2. Rust 测试

### 2.1 测试文件分布

| 测试文件 | 覆盖模块 |
|---|---|
| `chat_tests.rs` | ChatEngine、RAG 查询编排 |
| `import_tests.rs` | ImportService、文件导入、去重、配额 |
| `license_tests.rs` | License 校验、Pro 状态 |
| `loader_tests.rs` | Markdown/Text/PDF 加载器 |
| `retriever_tests.rs` | VectorRetriever、相似度检索 |
| `splitter_tests.rs` | 文本分块、token 计数 |
| `stream_parse_tests.rs` | SSE 流式解析 |
| `embedder_tests.rs` | fastembed/ONNX 嵌入 |
| `storage_tests.rs` | SQLite 存储持久化 |

### 2.2 `*_inner` 测试复用模式

每个 IPC 命令有对应的 `*_inner` 函数，接收 `&AppState` 而非 `State<'_, AppState>`，集成测试可复用：

```rust
// 命令
#[tauri::command]
pub async fn import_files(app, paths, state) -> Result<Vec<String>, String> {
    import_files_inner(&app, &paths, state.inner()).await
}

// 测试复用
pub async fn import_files_inner<R: Runtime>(
    app: &AppHandle<R>,
    paths: &[String],
    state: &AppState,
) -> Result<Vec<String>, String> { ... }
```

### 2.3 运行特定测试

```bash
# 按名称匹配
cargo test vector_search

# 按模块
cargo test -p echomind-core

# 显示输出
cargo test -- --nocapture

# 并行/串行
cargo test -- --test-threads=1
```

---

## 3. E2E 测试（Playwright）

### 3.1 E2E 测试模式

`ECHOMIND_E2E_TEST=1` 环境变量启用 Mock 模式：
- `e2e-tests/tests/bridge/tauri-stub.js` 通过 Playwright 注入 `window.__TAURI__` mock
- 测试在普通 Chromium 中运行，不是 Tauri 窗口
- 零网络、零 API 费用

`ECHOMIND_DATA_DIR` 环境变量重定向数据目录（隔离测试数据）。

### 3.2 运行

```bash
cd e2e-tests
npm install

# Mock 模式（零网络）
ECHOMIND_E2E_TEST=1 npx playwright test

# 查看报告
npx playwright show-report
```

### 3.3 测试文件

| 文件 | 内容 |
|---|---|
| `bridge.spec.ts` | Tauri 桥接层测试 |
| `chat.spec.ts` | 对话交互 |
| `import.spec.ts` | 文档导入流程 |
| `session.spec.ts` | 会话管理 |
| `wizard.spec.ts` | 初始配置向导 |
| `edge-cases.spec.ts` | 边缘场景 |
| `visual.spec.ts` | 视觉回归 |

---

## 4. 混沌测试语料

| 文件 | 类型 | 场景 | 生成方式 |
|---|---|---|---|
| `large_real.md` | 大型真实 Markdown | 抗压测试（超长文档分块） | GitHub Raw 拉取 |
| `complex_paper.pdf` | 复杂学术论文 PDF | PDF 解析容错（图表/公式/多栏） | arXiv 下载 |
| `mixed_encoding.txt` | 混合编码文本 | 编码地狱容错（UTF-8/GBK/非法字节） | 脚本生成 |
| `malicious.md` | 恶意 Markdown | XSS 安全测试（script/img onerror） | 脚本生成 |
| `fake.pdf` | 格式欺骗文件 | PdfLoader 错误处理 | 脚本生成 |
| `empty.md` | 空文件 | 空内容边界处理 | 脚本生成 |
| `deep_nested/` | 深套目录（50 层） | 海量并发遍历 | 脚本生成 |

**生成命令**：`python3 tests/fixtures/generate_corpus.py`

---

## 5. 真实 LLM 联调测试

> **铁律**：需要真实 LLM 的集成测试通过环境变量注入凭据，严禁硬编码 API Key。

| 环境变量 | 用途 | 示例 |
|---|---|---|
| `ANTHROPIC_AUTH_TOKEN` | LLM API Key | `sk-ant-xxxx` |
| `BASE_URL` | LLM API Base URL | `https://api.anthropic.com` |
| `CLAUDE_CODE_SUBAGENT_MODEL` | 模型名称 | `claude-sonnet-4-20250514` |

**使用规范**：
1. 测试代码通过 `std::env::var()` 读取，缺失时 `skip` 而非 `fail`。
2. 严禁将环境变量值写入日志、断言消息或 Git 提交。
3. CI 中通过 GitHub Secrets 注入；本地开发通过 shell `export` 临时设置。

---

## 6. 测试质量原则（杜绝无意义用例）

1. **每个断言必须有实际意义**：不写 `assert!(true)` 这类永远通过的废话断言
2. **边界测试必须覆盖**：空字符串、null/None、超长输入、特殊字符、Unicode/CJK
3. **错误路径必须测试**：异常输入、网络失败、DB 错误
4. **测试名称要描述行为**：用 `test_vector_search_with_empty_db_returns_empty` 而非 `test1`
5. **真实数据测试（铁律四）**：禁止仅用 toy data，L2 及以上必须包含真实语料和边缘数据
