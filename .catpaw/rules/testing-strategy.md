# EchoMind（灵犀）测试策略

> 本文件定义 EchoMind 项目的测试金字塔、命名约定和测试模式。AI Agent 开发时必须遵循。

---

## 1. 测试金字塔

```
          /\
         /  \           L4: E2E 桥接 (Playwright + tauri-stub.js)
        /    \          955 passed, 9 skipped
       /------\
      /        \        L3: 集成测试 (tests/integration.rs)
     /          \       115 tests (IPC *_inner 函数 + mock AppState)
    /------------\
   /              \     L2: Rust 单元测试 (*_tests.rs 同文件)
  /                \    822 Free + 274 Pro = 1096 tests (68 个测试文件)
 /------------------\
/                    \  L1: Lint 门禁
----------------------  fmt + clippy + tsc + esbuild
```

| 层级 | 工具 | 命令 | 数量 |
|---|---|---|---|
| L1 门禁 | cargo fmt + clippy + tsc + esbuild | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && npx tsc --noEmit && node scripts/build-ui.mjs` | 4 项全绿 |
| L2 单元 | `#[cfg(test)] mod *_tests` | `cargo test` / `cargo test --features pro` | 822 Free + 274 Pro |
| L2 前端 | Vitest | `cd e2e-tests && npm run test:unit` | 407 passed |
| L3 集成 | `tests/integration.rs` | `cargo test --test integration` | 115 tests |
| L4 E2E | Playwright + tauri-stub.js | `cd e2e-tests && npm run test:bridge` | 955 passed |

---

## 2. 测试命名约定

### Rust 单元测试
```
TC-<域>-<序号>-<描述>
```

| 域 | 示例 |
|---|---|
| ING | TC-ING-DOCX-001, TC-ING-EPUB-002 |
| VEC | TC-VEC-CUSTOM-001, TC-VEC-CUSTOM-UNIT-008 |
| RAG | TC-RAG-SEARCH-001, TC-RAG-BRANCH-003 |
| PERF | TC-CACHE-001, TC-COMP-010, TC-PROG-006 |
| SEC | TC-POSTURE-001, TC-SHADOW-003 |
| KEYED | TC-KEYED-001~006 |
| ROUTER | TC-ROUTER-001~012 |
| COORD | TC-COORD-001~005 |
| SCRATCH | TC-SCRATCH-001~007 |

### 测试文件命名
```
<module>_tests.rs    // 与实现同文件或同目录
```

### 测试结构（AAA 模式）
```rust
#[test]
fn tc_cache_001_exact_match_hit() {
    // Arrange
    let cache = SqliteCache::new(pool);
    let query = "hello world";
    let embedding = vec![0.1; 384];
    let answer = "test answer";

    // Act
    cache.put_exact(query, embedding.clone(), answer).unwrap();
    let hit = cache.get_exact(query, &embedding).unwrap();

    // Assert
    assert!(hit.is_some());
    assert_eq!(hit.unwrap().answer, answer);
}
```

---

## 3. TDD 流程（铁律二 + 铁律十五）

```
1. 需求分析 → 理解 AC
2. 规格编写 → SRS 新增 REQ + AC + TC
3. 测试编写（红灯）→ 测试必须失败
4. 实现编码（绿灯）→ 最少代码使测试通过
5. 重构 → 消除重复，保持绿灯
6. 验证闭环 → cargo test + clippy + fmt
7. 文档同步 → 铁律七
8. GitHub 同步 → 铁律十一
```

**禁止**：
- 空断言 `assert!(true)`
- `expect(x >= 0).toBe(true)` 恒真断言
- `toBeTruthy()` 弱断言（用具体属性断言替代）
- `if-guard`（if 块内断言，应改为强制断言）

---

## 4. Mock 模式

### Rust Mock（core 层）
```rust
// 在 *_tests.rs 中定义 mock 实现
struct MockStorage {
    documents: Arc<Mutex<Vec<Document>>>,
    chunks: Arc<Mutex<Vec<Chunk>>>,
    // ...
}

impl Storage for MockStorage {
    async fn add_document(&self, doc: Document) -> Result<()> { ... }
    // ...
}
```

### E2E Mock（tauri-stub.js）
```javascript
// e2e-tests/tests/bridge/tauri-stub.js
// 注入 window.__TAURI__ mock，无需 Tauri 运行时
// 所有 162 个 IPC 命令都有 mock
// E2E_SPEED 倍率缩放延迟（CI 用 0.2）
```

---

## 5. Pro 测试门控

Pro 功能测试用 `#[cfg(all(test, feature = "pro"))]`：

```rust
#[cfg(all(test, feature = "pro"))]
mod local_llm_tests {
    #[test]
    fn tc_llm_perf_001_cpu_inference() { ... }
}
```

运行命令：`cargo test --features pro`

---

## 6. E2E 测试模式

### 桥接测试（mock __TAURI__）
```typescript
// e2e-tests/tests/*.spec.ts
import { test, expect } from '@playwright/test';

test('TC-DS-013 用户消息气泡圆角 22px', async ({ page }) => {
    await page.goto('file:///path/to/ui/index.html');
    // tauri-stub.js 自动注入
    const bubble = page.locator('.msg-user').first();
    await expect(bubble).toHaveCSS('border-radius', '22px');
});
```

### 真实 GUI 测试（WebdriverIO + tauri-driver）
```bash
# macOS 不支持，需 Linux/Windows
cd e2e-tests && npm run test:e2e
```

---

## 7. 混沌测试

```bash
# 生成混沌语料
python3 tests/fixtures/generate_corpus.py

# 运行混沌测试（含 ignored）
cargo test --workspace -- --nocapture --include-ignored
```

---

## 8. CI 集成

| Workflow | 文件 | 功能 |
|---|---|---|
| CI | `.github/workflows/ci.yml` | 4 平台矩阵构建+测试+lint + 供应链安全 |
| Release | `.github/workflows/release.yml` | tag 触发跨平台安装包构建 |
| AI Review | `.github/workflows/ai-review.yml` | Claude AI 代码审查 |
| E2E | `.github/workflows/e2e.yml` | Playwright E2E 测试 |

---

## 9. 供应链安全

```bash
cargo audit        # RustSec 漏洞扫描
cargo deny check   # 许可证 + 漏洞 + 重复依赖检查（deny.toml）
```
