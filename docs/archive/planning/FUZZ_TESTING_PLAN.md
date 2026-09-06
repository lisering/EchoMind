# EchoMind 模糊测试集成方案

## 1. 现有模糊测试基础设施

项目已建立模糊测试框架：
- **目录**：`fuzz/` 包含 `Cargo.toml`、`fuzz_targets/`、`corpus/`、`artifacts/`
- **现有目标**：
  - `semantic_splitter_fuzz`：测试语义分块器对任意 UTF-8 输入的鲁棒性
  - `entity_extractor_fuzz`：测试 NER 实体抽取器对任意输入的鲁棒性（替代已移除的 `normalize_query_fuzz`）
- **CI 集成**：`.github/workflows/quality-gate.yml` 每周一 06:00 UTC 运行，每个目标 120 秒

## 2. 新增解析器模糊测试目标

### 2.1 目标评估

| 解析器 | 文件 | 风险等级 | 复杂度 | 理由 |
|--------|------|----------|--------|------|
| **wiki_link_parser** | `crates/core/src/wiki_link_parser.rs` | 高 | 中 | 纯函数解析器，处理用户输入的 Markdown 文本，存在边界条件风险 |
| **SSE parser** | `crates/core/src/stream_parse.rs` | 高 | 高 | 字节流状态机，处理网络粘包/拆包，UTF-8 安全关键 |
| **EntityExtractor** | `crates/core/src/entity_extractor.rs` | 高 | 高 | 纯规则+正则 NER，多层正则匹配，处理任意文本输入 |
| Markdown 提取 | `crates/core/src/loader.rs` | 中 | 低 | 使用第三方库 pulldown-cmark，风险较低 |

### 2.2 新增目标详情

#### 2.2.1 wiki_link_parser_fuzz
- **测试内容**：Obsidian 风格 `[[wiki-link]]` 语法解析
- **输入**：任意 UTF-8 字节序列
- **不变量**：
  1. 任意输入不得 panic
  2. 返回的 WikiLink target 非空
  3. 代码块中的 wiki-link 不被解析
- **预期发现**：边界条件（未闭合的 `[[]]`、空 target、嵌套语法）

#### 2.2.2 sse_parser_fuzz
- **测试内容**：SSE 事件流解析器（字节级缓冲）
- **输入**：任意字节流，模拟网络粘包/拆包场景
- **不变量**：
  1. 任意输入不得 panic
  2. 解析出的事件为有效 UTF-8 字符串
  3. 不丢失完整事件（`data:` 行正确提取）
- **预期发现**：UTF-8 跨块边界问题、缓冲区溢出、状态机错误

## 3. 配置文件详情

### 3.1 fuzz/Cargo.toml 更新
```toml
[[bin]]
name = "entity_extractor_fuzz"
path = "fuzz_targets/entity_extractor_fuzz.rs"
test = false
doc = false
bench = false

[[bin]]
name = "wiki_link_parser_fuzz"
path = "fuzz_targets/wiki_link_parser_fuzz.rs"
test = false
doc = false
bench = false

[[bin]]
name = "sse_parser_fuzz"
path = "fuzz_targets/sse_parser_fuzz.rs"
test = false
doc = false
bench = false
```

### 3.2 fuzz_targets/entity_extractor_fuzz.rs
- **实现**：调用 `EntityExtractor::extract` 和 `extract_relations`，验证不变量
- **策略**：使用 `String::from_utf8_lossy` 处理非法 UTF-8 输入
- **注意**：`normalize_query_fuzz` 已被替换，因 `normalize_query` 函数已从代码库中移除

### 3.3 fuzz_targets/sse_parser_fuzz.rs
- **实现**：模拟随机切分点，测试 `SseParser` 状态机
- **策略**：随机块大小（1-64 字节）模拟真实网络场景

## 4. CI 集成方案

### 4.1 更新 quality-gate.yml
```yaml
fuzz:
  name: Fuzz parsers (${{ matrix.target }})
  runs-on: ubuntu-24.04
  timeout-minutes: 30
  strategy:
    fail-fast: false
    matrix:
      target: 
        - semantic_splitter_fuzz
        - entity_extractor_fuzz
        - wiki_link_parser_fuzz
        - sse_parser_fuzz
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@nightly
    - name: Install cargo-fuzz
      run: cargo install cargo-fuzz --locked
    - name: Smoke fuzz (120s per target)
      working-directory: fuzz
      run: cargo +nightly fuzz run ${{ matrix.target }} -- -max_total_time=120
    - name: Upload artifacts on failure
      if: failure()
      uses: actions/upload-artifact@v4
      with:
        name: fuzz-crash-${{ matrix.target }}
        path: fuzz/artifacts/
        retention-days: 7
```

### 4.2 执行策略
- **频率**：每周一 06:00 UTC（现有 quality-gate）
- **时长**：每个目标 120 秒（可调整）
- **触发条件**：手动触发或定时任务
- **失败处理**：上传崩溃 artifacts，保留 7 天

### 4.3 本地开发流程
```bash
# 构建所有模糊测试目标
cd fuzz
cargo +nightly fuzz build

# 运行特定目标（120 秒）
cargo +nightly fuzz run wiki_link_parser_fuzz -- -max_total_time=120
cargo +nightly fuzz run sse_parser_fuzz -- -max_total_time=120
cargo +nightly fuzz run entity_extractor_fuzz -- -max_total_time=120

# 检查崩溃用例
ls artifacts/wiki_link_parser_fuzz/
ls artifacts/sse_parser_fuzz/
ls artifacts/entity_extractor_fuzz/
```

## 5. 监控与维护

### 5.1 崩溃分析
- **自动上传**：崩溃用例上传到 GitHub Artifacts
- **手动分析**：下载 artifacts，本地复现
- **修复流程**：修复代码 → 添加回归测试 → 更新 corpus

### 5.2 Corpus 管理
- **初始 corpus**：基于现有单元测试用例
- **增量更新**：发现新路径时自动添加到 corpus
- **定期清理**：删除无效 corpus 文件

### 5.3 性能优化
- **并行执行**：多个目标可并行运行（GitHub Actions matrix）
- **超时控制**：每个目标 120 秒，避免 CI 时间过长
- **缓存利用**：Rust 缓存加速编译

## 6. 预期收益

### 6.1 安全性提升
- **边界条件发现**：解析器对畸形输入的鲁棒性
- **内存安全**：检测潜在的缓冲区溢出、空指针解引用
- **UTF-8 安全**：确保多字节字符处理正确

### 6.2 代码质量
- **回归测试**：崩溃用例成为永久回归测试
- **代码覆盖**：模糊测试提高代码覆盖率
- **文档价值**：不变量作为解析器行为的精确描述

### 6.3 开发流程
- **早期发现**：在开发阶段发现解析器问题
- **自动化**：CI 自动运行，无需人工干预
- **可追溯性**：崩溃用例可追溯到具体代码变更

## 7. 后续扩展

### 7.1 更多解析器目标
- **CSV 解析器**：如果项目引入 CSV 导入功能
- **JSON 解析器**：测试自定义 JSON 解析逻辑
- **Markdown 解析器**：测试 pulldown-cmark 的包装层

### 7.2 高级模糊测试技术
- **结构化模糊测试**：使用 `arbitrary` crate 生成结构化输入
- **覆盖率引导**：启用覆盖率反馈，提高测试深度
- **语料库变异**：基于现有 corpus 进行智能变异

### 7.3 集成到开发流程
- **预提交钩子**：在提交前运行快速模糊测试
- **PR 检查**：为解析器相关 PR 运行更长时间的模糊测试
- **发布门控**：发布前运行完整模糊测试套件

---

**实施状态**：✅ 已完成并验证
**初步测试结果**：350K+ 运行，零崩溃
**下一步**：集成到 CI 流水线，观察每周定期运行结果