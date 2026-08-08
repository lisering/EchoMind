# Session Archive: v1.2 S52 — 自定义 ONNX 嵌入模型上传（REQ-VEC-014，Pro 门控）

> **日期**: 2026-08-07 | **Session**: v1.2 S52 | **REQ**: REQ-VEC-014 | **状态**: ✅ Implemented

## 实现内容

### 核心实现（零新增 Rust 依赖，复用 fastembed `UserDefinedEmbeddingModel`）

- `crates/infra/src/local_embedder.rs` — `EmbeddingModel` 枚举新增 `Custom(String)` 变体（模型名称对应 `custom_models/{name}/` 目录）+ `new_with_custom_model()` / `init_custom()` 异步加载方法（查找 ONNX 文件 → 读取 tokenizer → 构建 `UserDefinedEmbeddingModel` + `TextEmbedding` 会话 → 试推理 `"test"` 检测向量维度）+ `find_onnx_file()` 按优先级查找（`model_quantized.onnx` → `model.onnx` → `model_optimized.onnx`）+ `validate_onnx_file()` 验证（非空 + 非 HTML 错误页）+ `validate_tokenizer_files()` 检测缺失文件 + `list_custom_models()` 扫描目录 + `delete_custom_model()` 路径遍历防护 + `copy_custom_model_files()` 复制并验证 + `CustomModelInfo` 结构体 + `REQUIRED_TOKENIZER_FILES` / `ONNX_FILE_CANDIDATES` 常量 + `is_custom()` / `dim()` 返回 0 for Custom

- `crates/tauri-app/src/stores/model_store.rs` — `parse_embedding_model()` 解析 `"custom:{name}"` 前缀 → `EmbeddingModel::Custom(name)` + `load_embedding_model()` 从 settings 读取 + `embedder()` / `init_embedder_with_progress()` 走 `new_with_custom_model()` 路径（自定义模型不需要下载进度回调）

- `crates/tauri-app/src/state.rs` — `custom_model_dir()` 返回 `{data_dir}/custom_models/`

### 3 个 Pro 门控 IPC 命令（`crates/tauri-app/src/commands.rs`）

1. `upload_custom_embedding_model(name, onnx_path, tokenizer_files)` → `CustomModelInfo`（Pro 门控 + ONNX 验证 + 文件复制 + 路径遍历防护）
2. `list_custom_models()` → `Vec<CustomModelInfo>`（Pro 门控 + 目录扫描）
3. `delete_custom_model(name)` → `()`（Pro 门控 + 路径遍历防护 + 当前模型删除时重置为默认）

- Free 版 stub：3 个命令均返回 `PRO_REQUIRED` 错误
- `lib.rs` 注册 3 命令（138 → 141）

### 前端 UI（`ui/src/settings.js` + `ui/index.html`）

- `loadCustomModels()` — 调用 `list_custom_models` IPC，渲染模型列表（名称 + 大小 + 有效性 + 切换/删除按钮）
- `onUploadCustomModel()` — 上传流程：选择 ONNX 文件 → 选择 tokenizer 文件（多选）→ 输入模型名称 → 调用 IPC → toast 结果
- `switchToCustomModel(name)` — 调用 `set_embedding_model('custom:{name}')` → toast 切换成功
- `deleteCustomModel(name)` — 确认对话框 → 调用 IPC → toast → 刷新列表
- `embedModelApi.uploadCustomModel/listCustomModels/deleteCustomModel` — IPC 封装

### i18n（17 键，中英文）

`custom_model_section` / `custom_model_description` / `custom_model_upload` / `custom_model_name` / `custom_model_select_onnx` / `custom_model_select_tokenizer` / `custom_model_upload_success` / `custom_model_upload_failed` / `custom_model_delete` / `custom_model_delete_confirm` / `custom_model_deleted` / `custom_model_empty` / `custom_model_invalid` / `custom_model_size` / `custom_model_pro_required` / `custom_model_switch` / `custom_model_switched`

### Mock（`e2e-tests/tests/bridge/tauri-stub.js`）

3 个 IPC mock：`upload_custom_embedding_model` / `list_custom_models` / `delete_custom_model`（Pro 门控 + `state.customModels` 状态管理）

### TDD 测试

- `crates/infra/src/embedder_tests.rs` — TC-VEC-CUSTOM-UNIT-001~008（8 个单元测试，纯逻辑不涉及 ONNX 加载）
  - UNIT-001: `EmbeddingModel::Custom` 变体属性（`is_custom()` / `dim()` 返回 0）
  - UNIT-002: `find_onnx_file` 按优先级查找
  - UNIT-003: `validate_onnx_file` 检测无效文件（空 / HTML）
  - UNIT-004: `validate_tokenizer_files` 检测缺失文件
  - UNIT-005: `list_custom_models` 扫描目录并返回模型列表（完整 vs 不完整）
  - UNIT-006: `delete_custom_model` 删除目录并返回大小
  - UNIT-007: `delete_custom_model` 路径遍历防护
  - UNIT-008: `copy_custom_model_files` 复制文件并验证完整性

- `crates/tauri-app/tests/integration.rs` — TC-VEC-CUSTOM-001~007（7 个集成测试，真实 AppState + SQLite）
  - 001: 上传有效 ONNX 模型 → 文件复制成功
  - 002: 切换到自定义模型 → 设置持久化 + embedder 缓存清除
  - 003: 上传无效文件（HTML 错误页）→ Err
  - 004: 删除自定义模型 → 文件删除
  - 005: Free 用户被拒绝 → PRO_REQUIRED
  - 006: `list_custom_models` 返回完整列表
  - 007: 路径遍历防护 — `../../etc/passwd` 被清理

### 文档同步

- `docs/SRS_v1.0.md` — REQ-VEC-014 Draft → Implemented + AC-1~AC-7 + changelog v5.5.0
- `docs/architecture/V1.2_ATOMIC_PLAN.md` — S52 验收标准全部 ✅
- `docs/api/ipc-commands.md` — §2.53 新增 3 个命令文档 + 命令总数 141
- `docs/api/settings-reference.md` — `vec.embedding_model` 新增 `custom:{name}` 前缀说明
- `docs/api/code-contracts.md` — `new_with_custom_model()` / `list_custom_models()` / `CustomModelInfo` struct
- `docs/api/frontend-modules.md` — §2.12.1 自定义模型函数清单

## 验证结果

| 检查项 | 结果 |
|---|---|
| TC-VEC-CUSTOM-UNIT-001~008（单元测试） | 8 passed, 0 failed |
| TC-VEC-CUSTOM-001~007（集成测试，Pro） | 7 passed, 0 failed |
| cargo clippy --all-targets -- -D warnings (Free) | 零警告 |
| cargo clippy --all-targets --features pro -- -D warnings (Pro) | 零警告 |
| cargo fmt --check | 通过 |
| node scripts/build-ui.mjs | 11670 行，成功 |
| npx tsc --noEmit | 0 errors |
| sync-to-github.sh --commit | 10 files synced |

## 衔接提示词（S53 v1.2 发布准备）

下一 Session S53 目标：v1.2 发布准备。

**必读文档**:
1. `docs/architecture/V1.2_ATOMIC_PLAN.md` — S53 章节（第 684-706 行）
2. `docs/sessions/CURRENT_STATE.md` — 当前项目状态快照

**关键任务**:
1. 全量回归测试（cargo test + cargo test --features pro + E2E）
2. 版本号升级 1.1.0 → 1.2.0（7 crate + tauri.conf.json）
3. Release build 验证（cargo build --release --features pro）
4. CHANGELOG_v1.2.md 发布说明
5. GitHub 发布同步 + git tag v1.2.0
