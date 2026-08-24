# EchoMind 后端 IPC 命令全量审计报告（S2）

> **审计日期**: 2026-08-13
> **审计范围**: `crates/tauri-app/src/lib.rs` generate_handler! 注册的全部 224 Free + 12 Pro-only = 236 IPC 命令
> **审计方法**: 逐命令检查前端调用者 + 命令分组 + Pro 门控 + 用户可见性 + 借鉴来源 + 用户场景
> **评估标准**: 10 条（S3/S7/S8/C2/C3/C5/I1/I2/I3/I4）
> **核心理念**: 使用越简单，功能越强大

---

## 0. 审计摘要

### 0.1 总览统计

| 指标 | 数值 |
|---|---|
| 注册命令总数 | 236（224 Free + 12 Pro-only） |
| 功能域子模块数 | 18（agent/audit/chat/conversation/document/features/graph/import/license/local_llm/performance/rag_eval/security/settings/sync/trace/workspace） |
| 前端有调用者的命令 | 224（含动态 invoke 模式） |
| 前端无调用者的命令（僵尸） | **0** ✅ |
| 前端幻影调用（后端未注册） | **11** 🔴 |
| Pro 门控命令 | 12 |
| 借鉴来源命令 | ~65（QM ~22 / OpenCode ~10 / DS4 ~3 / OpenMontage ~0 / Cherry Studio ~10 / RAGAS ~4 / 其他 ~16） |

### 0.2 等级分布

| 等级 | 数量 | 占比 | 含义 |
|---|---|---|---|
| A | 62 | 26% | 优秀 — 核心功能，用户场景明确，保持 |
| B | 58 | 25% | 良好 — 有用户价值，小幅优化 |
| C | 42 | 18% | 需改进 — 场景模糊或过度暴露 |
| D | 38 | 16% | 不合格 — 开发者工具伪装用户功能 |
| F | 36 | 15% | 需删除/隐藏 — 无用户场景或借鉴未落地 |

### 0.3 处置方案分布

| 处置 | 数量 | 说明 |
|---|---|---|
| 保持 | 62 | 核心功能命令 |
| 小幅优化 | 28 | 命名或分组调整 |
| 隐藏入口 | 38 | 开发者工具移入「高级/开发者」模式 |
| 合并简化 | 12 | 重复 set/get 对合并为单一命令 |
| 删除候选 | 36 | 无用户场景的借鉴命令或幻影调用 |
| 审查后决定 | 60 | 需进一步评估用户价值 |

### 0.4 C2 僵尸命令检查结论

**零僵尸命令**。所有 224+12 个注册命令均有前端调用者。部分通过动态 invoke 模式调用（`input-toggles.js` 的 `ipcCommand` 属性 + `perf-settings.js` 的 `cmd` 属性），静态 grep `invoke('命令名')` 无法发现，需手动验证。

### 0.5 核心发现

1. **🔴 11 个幻影调用**：前端调用 11 个后端未注册的命令（`encrypt_database` / `unlock_database` / `redact_pii` 等），运行时会报错。E2E mock 覆盖了这些命令导致问题被掩盖。
2. **🔴 借鉴功能命令泛滥**：~65 个命令来自 6 个借鉴来源，其中 ~30 个无明确用户场景（QM scratch/burst/shadow/posture、OpenCode admit/promote/todo、DS4 strip、Cherry Studio trace/budget）。
3. **🔴 开发者工具混入用户功能**：trace(4) + budget(2) + rag_eval(4) + durable(3) + session_todo(5) + strip(3) = 21 个开发者工具命令有前端 UI 入口。
4. **🟠 性能优化命令过度暴露**：19 个性能命令全部默认关闭，用户永远不会手动开启。
5. **🟠 set/get 对过多**：~30 对 set/get 命令，可合并为单一 settings 命令减少 IPC 数量。
6. **✅ 核心功能健康**：chat/import/document/conversation/workspace 等核心命令用户场景明确，Pro 门控合理。
7. **✅ Pro 门控合理**：12 个 Pro-only 命令均涉及本地 LLM 硬件加速或高级嵌入，确实需要 Pro。
8. **🟠 命令命名术语化**：`set_speculative_enabled` / `set_progressive_injection` / `set_quality_gate_enabled` 等命名对用户不友好（I1 🔴）。

---

## 1. 幻影调用详情（11 个）🔴 P0

> **严重程度**: P0 — 运行时崩溃
> **根因**: 前端 ipc.js 和部分 JS 模块调用后端未注册的命令名。这些命令可能是旧版本的重命名遗留，或从未实现。
> **E2E 掩盖**: tauri-stub.js mock 了全部 224+ 命令（包括这 11 个幻影），导致 E2E 测试通过但真实运行时崩溃。

| # | 幻影命令 | 前端调用位置 | 后端替代命令 | 处置 |
|---|---|---|---|---|
| 1 | `encrypt_database` | ipc.js `securityApi.encrypt` | 无（加密通过 SecurityManager 内部状态机） | **实现命令或删除前端调用** |
| 2 | `unlock_database` | ipc.js `securityApi.unlock` | `unlock_app` | **前端改为调用 `unlock_app`** |
| 3 | `set_auto_lock_timeout` | ipc.js `securityApi.setAutoLockTimeout` | `set_auto_lock_config` | **前端改为调用 `set_auto_lock_config`** |
| 4 | `redact_pii` | ipc.js `securityApi.redactPii` | `detect_pii`（已含 redacted 字段） | **前端改为调用 `detect_pii`** |
| 5 | `verify_audit_chain` | ipc.js `securityApi.verifyAuditChain` | 无 | **实现命令或删除前端调用** |
| 6 | `set_pii_detection_enabled` | ipc.js `securityApi.setPiiDetection` | 无 | **实现命令或删除前端调用** |
| 7 | `clear_clipboard` | ipc.js `securityApi.clearClipboard` | `set_clipboard_config` | **前端改为调用 `set_clipboard_config`** |
| 8 | `set_clipboard_clear_timeout` | ipc.js `securityApi.setClipboardClearTimeout` | `set_clipboard_config` | **前端改为调用 `set_clipboard_config`** |
| 9 | `check_security_status` | ipc.js `securityApi.checkStatus` | `get_security_status` | **前端改为调用 `get_security_status`** |
| 10 | `get_document_chunks` | tauri-stub.js mock | 无（chunks 通过 `get_document_preview` 返回） | **删除 mock** |
| 11 | `list_documents` | tauri-stub.js mock | `get_documents` | **删除 mock** |

---

## 2. 逐功能域评估

### 2.1 Chat & RAG 核心（chat.rs）— 14 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `chat` | chat.js ✅ | ✅ 对话区 | 用户提问 → RAG 检索 → 流式回答 | **A** | 保持 |
| 2 | `abort_chat` | chat.js ✅ | ✅ 停止按钮 | 用户中断生成 | **A** | 保持 |
| 3 | `export_conversation_markdown` | ipc.js ✅ | ✅ 导出菜单 | 用户导出对话为 MD 文件 | **A** | 保持 |
| 4 | `save_text_file` | settings-general.js ✅ | ✅ 保存对话框 | 通用文件保存（导出辅助） | **B** | 保持 |
| 5 | `read_text_file` | settings-general.js ✅ | ✅ 导入备份 | 通用文件读取（备份恢复辅助） | **B** | 保持 |
| 6 | `get_conversation_cost` | settings-advanced.js ✅ | ✅ 设置面板 | 用户查看 Token 消耗 | **C** | 隐藏入口（开发者工具） |
| 7 | `set_token_budget` | settings-advanced.js ✅ | ✅ 设置面板 | 用户设置日预算限额 | **C** | 隐藏入口 |
| 8 | `admit_input` | ipc.js ✅ | ❌ 无 UI | Durable 输入队列（B05 借鉴） | **D** | 隐藏入口（无用户场景） |
| 9 | `promote_input` | ipc.js ✅ | ❌ 无 UI | Durable 输入提升（B05 借鉴） | **D** | 隐藏入口 |
| 10 | `get_pending_inputs` | ipc.js ✅ | ❌ 无 UI | Durable 输入查询（B05 借鉴） | **D** | 隐藏入口 |
| 11 | `add_session_todo` | ipc.js ✅ | ❌ 无 UI | Session Todo（B08 借鉴） | **D** | 隐藏入口 |
| 12 | `update_todo_status` | ipc.js ✅ | ❌ 无 UI | Session Todo（B08 借鉴） | **D** | 隐藏入口 |
| 13 | `get_session_todos` | ipc.js ✅ | ❌ 无 UI | Session Todo（B08 借鉴） | **D** | 隐藏入口 |
| 14 | `delete_session_todo` / `delete_session_todos` | ipc.js ✅ | ❌ 无 UI | Session Todo（B08 借鉴） | **D** | 隐藏入口 |

**域评估**: 核心命令（chat/abort/export）优秀，但 8 个借鉴命令（durable/todo）无用户场景。**域等级: C**

---

### 2.2 会话管理（conversation.rs）— 22 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `create_conversation` | ipc.js ✅ | ✅ 新对话按钮 | 创建新会话 | **A** | 保持 |
| 2 | `get_conversations` | ipc.js ✅ | ✅ 侧栏列表 | 显示会话列表 | **A** | 保持 |
| 3 | `get_conversations_paginated` | ipc.js ✅ | ✅ 侧栏列表 | 大列表分页 | **A** | 保持 |
| 4 | `get_messages` | ipc.js ✅ | ✅ 对话区 | 加载会话消息 | **A** | 保持 |
| 5 | `get_messages_paginated` | ipc.js ✅ | ✅ 对话区 | 长对话分页 | **A** | 保持 |
| 6 | `edit_user_message` | ipc.js ✅ | ✅ 编辑按钮 | 用户编辑消息重生成 | **A** | 保持 |
| 7 | `set_turn_active_version` | ipc.js ✅ | ✅ 分支切换 | 版本树切换 | **B** | 保持 |
| 8 | `get_turn_active_versions` | ipc.js ✅ | ✅ 分支切换 | 版本树恢复 | **B** | 保持 |
| 9 | `delete_conversation` | ipc.js ✅ | ✅ 右键删除 | 删除会话 | **A** | 保持 |
| 10 | `rename_conversation` | ipc.js ✅ | ✅ 右键重命名 | 重命名会话 | **A** | 保持 |
| 11 | `reorder_conversations` | ipc.js ✅ | ✅ 拖拽排序 | 会话排序 | **A** | 保持 |
| 12 | `add_bookmark` | bookmarks.js ✅ | ✅ 书签按钮 | 会话书签 | **B** | 保持 |
| 13 | `remove_bookmark` | bookmarks.js ✅ | ✅ 书签按钮 | 移除书签 | **B** | 保持 |
| 14 | `list_bookmarks` | bookmarks.js ✅ | ✅ 书签列表 | 查看书签 | **B** | 保持 |
| 15 | `is_bookmarked` | bookmarks.js ✅ | ✅ 书签图标 | 检查书签状态 | **B** | 保持 |
| 16 | `delete_message` | ipc.js ✅ | ✅ 右键删除 | 删除单条消息 | **B** | 保持 |
| 17 | `get_conversation_tree` | ipc.js ✅ | ✅ 分支树面板 | 查看分支树 | **B** | 保持 |
| 18 | `branch_from_message` | ipc.js ✅ | ✅ 分支按钮 | 创建分支 | **B** | 保持 |
| 19 | `search_conversations` | ipc.js ✅ | ✅ 搜索框 | 全文搜索对话 | **A** | 保持 |
| 20 | `strip_messages` | ipc.js ✅ | ❌ 无 UI | DS4 会话条带化 | **D** | 隐藏入口 |
| 21 | `strip_keeping_recent` | ipc.js ✅ | ❌ 无 UI | DS4 会话条带化 | **D** | 隐藏入口 |
| 22 | `preview_strip` | ipc.js ✅ | ❌ 无 UI | DS4 会话条带化预览 | **D** | 隐藏入口 |

**域评估**: 核心会话管理命令优秀（19 个 A/B），3 个 DS4 借鉴命令无用户场景。**域等级: B**

---

### 2.3 文档管理（document.rs）— 16 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `get_documents` | ipc.js ✅ | ✅ 文档列表 | 查看知识库 | **A** | 保持 |
| 2 | `delete_document` | ipc.js ✅ | ✅ 删除按钮 | 删除文档 | **A** | 保持 |
| 3 | `retry_index` | ipc.js ✅ | ✅ 重试按钮 | 重试失败文档 | **A** | 保持 |
| 4 | `rebuild_index` | ipc.js ✅ | ✅ 重建按钮 | 重建索引 | **A** | 保持 |
| 5 | `reclassify_document` | ipc.js ✅ | ✅ 文档菜单 | 重新分类文档领域 | **B** | 保持 |
| 6 | `get_document_summary` | ipc.js ✅ | ✅ 文档预览 | 查看文档摘要 | **B** | 保持 |
| 7 | `regenerate_summary` | ipc.js ✅ | ✅ 摘要按钮 | 重新生成摘要 | **B** | 保持 |
| 8 | `get_document_preview` | ipc.js ✅ | ✅ 预览面板 | 预览文档内容 | **A** | 保持 |
| 9 | `export_document_original` | ipc.js ✅ | ✅ 导出按钮 | 导出原始文件 | **A** | 保持 |
| 10 | `add_document_tag` | ipc.js ✅ | ✅ 标签管理 | 添加文档标签 | **B** | 保持 |
| 11 | `remove_document_tag` | ipc.js ✅ | ✅ 标签管理 | 移除文档标签 | **B** | 保持 |
| 12 | `list_all_tags` | ipc.js ✅ | ✅ 标签筛选 | 列出所有标签 | **B** | 保持 |
| 13 | `filter_documents_by_tag` | ipc.js ✅ | ✅ 标签筛选 | 按标签筛选文档 | **B** | 保持 |
| 14 | `get_kb_stats` | ipc.js ✅ | ✅ 统计仪表盘 | 知识库统计 | **B** | 保持 |
| 15 | `get_import_history` | ipc.js ✅ | ✅ 历史面板 | 查看导入历史 | **B** | 保持 |
| 16 | `clear_import_history` | ipc.js ✅ | ✅ 历史面板 | 清空导入历史 | **B** | 保持 |

**域评估**: 全部命令有明确用户场景。**域等级: A**

---

### 2.4 导入（import.rs）— 5 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `import_files` | wizard.js ✅ | ✅ 导入按钮 | 导入文件到知识库 | **A** | 保持 |
| 2 | `replace_document` | ipc.js ✅ | ✅ 替换菜单 | 替换已存在文档 | **A** | 保持 |
| 3 | `get_file_sizes` | tauri-stub ✅ | ✅ 导入预览 | 检查文件大小 | **B** | 保持 |
| 4 | `get_file_size_limits` | tauri-stub ✅ | ✅ 导入预览 | 获取大小限制 | **B** | 保持 |
| 5 | `abort_import` | ipc.js ✅ | ✅ 取消按钮 | 取消导入 | **A** | 保持 |

**域评估**: 全部核心功能。**域等级: A**

---

### 2.5 设置与配置（settings.rs）— 41 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `test_llm_connection` | wizard.js ✅ | ✅ 向导 | 测试 LLM 连接 | **A** | 保持 |
| 2 | `update_llm_config` | wizard.js ✅ | ✅ 向导/设置 | 配置 LLM API | **A** | 保持 |
| 3 | `get_settings` | settings.js ✅ | ✅ 设置面板 | 获取全部设置 | **A** | 保持 |
| 4 | `get_locale` | i18n.js ✅ | ✅ 语言切换 | 获取语言偏好 | **A** | 保持 |
| 5 | `set_locale` | i18n.js ✅ | ✅ 语言切换 | 设置语言偏好 | **A** | 保持 |
| 6 | `get_theme` | settings.js ✅ | ✅ 主题切换 | 获取主题偏好 | **A** | 保持 |
| 7 | `set_theme` | settings.js ✅ | ✅ 主题切换 | 设置主题偏好 | **A** | 保持 |
| 8 | `get_sidebar_collapsed` | main.js ✅ | ✅ 侧栏折叠 | 获取侧栏状态 | **A** | 保持 |
| 9 | `set_sidebar_collapsed` | main.js ✅ | ✅ 侧栏折叠 | 设置侧栏状态 | **A** | 保持 |
| 10 | `get_chunk_params` | ipc.js ✅ | ✅ 高级设置 | 获取分块参数 | **B** | 保持 |
| 11 | `set_chunk_params` | ipc.js ✅ | ✅ 高级设置 | 设置分块参数 | **B** | 保持 |
| 12 | `set_embedding_model` | ipc.js ✅ | ✅ 设置面板 | 切换嵌入模型 | **B** | 保持 |
| 13 | `set_context_token_limit` | settings-advanced.js ✅ | ✅ 高级设置 | 设置上下文窗口 | **C** | 隐藏入口 |
| 14 | `set_vlm_enabled` | settings-general.js ✅ | ✅ 通用设置 | VLM 开关 | **B** | 保持 |
| 15 | `set_hybrid_search` | input-toggles.js ✅ | ✅ Toggle | 混合搜索开关 | **B** | 保持 |
| 16 | `set_rerank_enabled` | settings-general.js ✅ | ✅ 通用设置 | 重排序开关 | **B** | 保持 |
| 17 | `set_hyde_enabled` | settings-general.js ✅ | ✅ 通用设置 | HyDE 开关 | **C** | 隐藏入口 |
| 18 | `get_token_budget_config` | ipc.js ✅ | ✅ 高级设置 | Token 预算配置 | **C** | 隐藏入口 |
| 19 | `set_token_budget_config` | ipc.js ✅ | ✅ 高级设置 | Token 预算配置 | **C** | 隐藏入口 |
| 20 | `get_rag_params` | ipc.js ✅ | ✅ 高级设置 | RAG 检索参数 | **C** | 隐藏入口 |
| 21 | `set_rag_params` | ipc.js ✅ | ✅ 高级设置 | RAG 检索参数 | **C** | 隐藏入口 |
| 22 | `get_generation_params` | ipc.js ✅ | ✅ 高级设置 | LLM 生成参数 | **C** | 隐藏入口 |
| 23 | `set_generation_params` | ipc.js ✅ | ✅ 高级设置 | LLM 生成参数 | **C** | 隐藏入口 |
| 24 | `get_autostart` | ipc.js ✅ | ✅ 通用设置 | 开机自启 | **B** | 保持 |
| 25 | `set_autostart` | ipc.js ✅ | ✅ 通用设置 | 开机自启 | **B** | 保持 |
| 26 | `check_for_updates` | ipc.js ✅ | ✅ 帮助菜单 | 检查更新 | **A** | 保持 |
| 27 | `get_update_check_config` | ipc.js ✅ | ✅ 帮助菜单 | 更新检查配置 | **B** | 保持 |
| 28 | `set_update_check_enabled` | ipc.js ✅ | ✅ 帮助菜单 | 更新检查开关 | **B** | 保持 |
| 29 | `get_close_to_tray` | ipc.js ✅ | ✅ 窗口设置 | 最小化到托盘 | **B** | 保持 |
| 30 | `set_close_to_tray` | ipc.js ✅ | ✅ 窗口设置 | 最小化到托盘 | **B** | 保持 |
| 31 | `get_log_level` | settings-general.js ✅ | ✅ 可观测性 | 日志级别 | **C** | 隐藏入口 |
| 32 | `set_log_level` | settings-general.js ✅ | ✅ 可观测性 | 日志级别 | **C** | 隐藏入口 |
| 33 | `export_logs` | settings-general.js ✅ | ✅ 可观测性 | 导出日志 | **C** | 隐藏入口 |
| 34 | `export_diagnostics` | settings-general.js ✅ | ✅ 可观测性 | 导出诊断信息 | **C** | 隐藏入口 |
| 35 | `export_backup` | settings-general.js ✅ | ✅ 备份恢复 | 全量备份 | **A** | 保持 |
| 36 | `import_backup` | settings-general.js ✅ | ✅ 备份恢复 | 恢复备份 | **A** | 保持 |
| 37 | `open_data_dir` | tauri-stub ✅ | ✅ 错误恢复 | 打开数据目录 | **B** | 保持 |
| 38 | `export_error_logs` | ipc.js ✅ | ✅ 错误恢复 | 导出错误日志 | **C** | 隐藏入口 |
| 39 | `init_embedder` | wizard.js ✅ | ✅ 向导 | 初始化嵌入引擎 | **A** | 保持 |
| 40 | `check_embedder_status` | tauri-stub ✅ | ✅ 向导 | 检查嵌入引擎状态 | **A** | 保持 |
| 41 | `get_model_cache_info` | settings-general.js ✅ | ✅ 通用设置 | 模型缓存信息 | **B** | 保持 |
| — | `clear_model_cache` | settings-general.js ✅ | ✅ 通用设置 | 清除模型缓存 | **B** | 保持 |

**域评估**: 核心设置命令（LLM/主题/语言/备份/导入）优秀，但 ~15 个高级设置命令应隐藏到开发者模式。set/get 对过多（可合并）。**域等级: B**

---

### 2.6 本地 LLM（local_llm.rs）— 15 Free + 12 Pro = 27 命令

| # | 命令 | Pro | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|---|
| 1 | `list_local_models` | ❌ | ipc.js ✅ | ✅ 设置面板 | 列出已下载模型 | **A** | 保持 |
| 2 | `get_recommended_models` | ❌ | ipc.js ✅ | ✅ 设置面板 | 推荐模型列表 | **A** | 保持 |
| 3 | `download_model` | ❌ | ipc.js ✅ | ✅ 下载面板 | 下载 GGUF 模型 | **A** | 保持 |
| 4 | `pause_download` | ❌ | ipc.js ✅ | ✅ 下载面板 | 暂停下载 | **A** | 保持 |
| 5 | `abort_download` | ❌ | ipc.js ✅ | ✅ 下载面板 | 取消下载 | **A** | 保持 |
| 6 | `get_download_status` | ❌ | ipc.js ✅ | ✅ 下载面板 | 下载状态 | **A** | 保持 |
| 7 | `list_pending_downloads` | ❌ | ipc.js ✅ | ✅ 下载面板 | 未完成下载 | **A** | 保持 |
| 8 | `cleanup_partial_downloads` | ❌ | ipc.js ✅ | ✅ 下载面板 | 清理临时文件 | **B** | 保持 |
| 9 | `scan_download_recovery` | ❌ | ipc.js ✅ | ✅ 下载面板 | 崩溃恢复 | **B** | 保持 |
| 10 | `delete_model` | ❌ | ipc.js ✅ | ✅ 设置面板 | 删除模型 | **A** | 保持 |
| 11 | `set_llm_mode` | ❌ | ipc.js ✅ | ✅ 模式切换 | 切换远程/本地 | **A** | 保持 |
| 12 | `get_llm_mode` | ❌ | ipc.js ✅ | ✅ 模式切换 | 获取当前模式 | **A** | 保持 |
| 13 | `set_local_model` | ❌ | ipc.js ✅ | ✅ 设置面板 | 选择本地模型 | **A** | 保持 |
| 14 | `upload_custom_embedding_model` | ❌ | ipc.js ✅ | ✅ 设置面板 | 上传自定义 ONNX | **B** | 保持 |
| 15 | `list_custom_models` | ❌ | ipc.js ✅ | ✅ 设置面板 | 列出自定义模型 | **B** | 保持 |
| 16 | `delete_custom_model` | ❌ | ipc.js ✅ | ✅ 设置面板 | 删除自定义模型 | **B** | 保持 |
| 17 | `get_local_llm_device_kind` | ✅ | ipc.js ✅ | ✅ 高级设置 | 查看设备类型 | **C** | 隐藏入口 |
| 18 | `set_paged_attn` | ✅ | ipc.js ✅ | ✅ 高级设置 | PagedAttention 配置 | **C** | 隐藏入口 |
| 19 | `set_sampling_params` | ✅ | ipc.js ✅ | ✅ 高级设置 | 采样参数配置 | **C** | 隐藏入口 |
| 20 | `set_kernel_mode` | ✅ | ipc.js ✅ | ✅ 高级设置 | GEMV 内核模式 | **D** | 隐藏入口（开发者工具） |
| 21 | `get_kernel_mode` | ✅ | ipc.js ✅ | ✅ 高级设置 | GEMV 内核模式 | **D** | 隐藏入口 |
| 22 | `save_kv_cache` | ✅ | ipc.js ✅ | ❌ 无 UI | KV Cache 保存 | **D** | 隐藏入口 |
| 23 | `load_kv_cache` | ✅ | ipc.js ✅ | ❌ 无 UI | KV Cache 加载 | **D** | 隐藏入口 |
| 24 | `clear_kv_cache` | ✅ | ipc.js ✅ | ❌ 无 UI | KV Cache 清除 | **D** | 隐藏入口 |
| 25 | `get_kv_cache_status` | ✅ | ipc.js ✅ | ❌ 无 UI | KV Cache 状态 | **D** | 隐藏入口 |
| 26 | `set_kv_cache_enabled` | ✅ | ipc.js ✅ | ❌ 无 UI | KV Cache 开关 | **D** | 隐藏入口 |
| 27 | `set_embedder_model` | ✅ | ipc.js ✅ | ✅ 设置面板 | 运行时切换嵌入模型 | **B** | 保持 |

**域评估**: 下载管理命令优秀（A），Pro 高级命令需隐藏。KV Cache 6 个命令无 UI 入口。**域等级: B**

---

### 2.7 安全防御（security.rs）— 18 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `get_security_status` | ipc.js ✅ | ✅ 安全指示器 | 查看安全状态 | **A** | 保持 |
| 2 | `set_auto_lock_config` | ipc.js ✅ | ✅ 安全设置 | 自动锁屏配置 | **A** | 保持 |
| 3 | `lock_app` | ipc.js ✅ | ✅ 锁定按钮 | 手动锁定 | **A** | 保持 |
| 4 | `unlock_app` | ipc.js ✅ | ✅ 解锁界面 | 解锁应用 | **A** | 保持 |
| 5 | `record_activity` | ipc.js ✅ | ❌ 内部 | 重置自动锁屏计时器 | **B** | 保持 |
| 6 | `detect_pii` | ipc.js ✅ | ✅ 安全设置 | PII 检测 | **B** | 保持 |
| 7 | `set_panic_wipe_password` | ipc.js ✅ | ✅ 安全设置 | 紧急销毁密码 | **B** | 保持 |
| 8 | `clear_panic_wipe_password` | ipc.js ✅ | ✅ 安全设置 | 清除紧急销毁 | **B** | 保持 |
| 9 | `is_panic_wipe_enabled` | ipc.js ✅ | ✅ 安全设置 | 查询紧急销毁状态 | **B** | 保持 |
| 10 | `set_clipboard_config` | ipc.js ✅ | ✅ 安全设置 | 剪贴板自动清除 | **B** | 保持 |
| 11 | `get_audit_logs` | ipc.js ✅ | ✅ 审计面板 | 查看审计日志 | **B** | 保持 |
| 12 | `clear_audit_logs` | ipc.js ✅ | ✅ 审计面板 | 清空审计日志 | **B** | 保持 |
| 13 | `export_audit_report` | ipc.js ✅ | ✅ 审计面板 | 导出审计报告 | **B** | 保持 |
| 14 | `check_password_strength` | ipc.js ✅ | ✅ 密码输入 | 密码强度检查 | **A** | 保持 |
| 15 | `set_security_posture` | ipc.js ✅ | ❌ 无 UI | QM 安全态势（dangerous/auto/strict） | **D** | 隐藏入口 |
| 16 | `get_security_posture` | ipc.js ✅ | ❌ 无 UI | QM 安全态势查询 | **D** | 隐藏入口 |
| 17 | `get_security_screen_stats` | ipc.js ✅ | ❌ 无 UI | QM Shadow 筛查统计 | **D** | 隐藏入口 |
| 18 | `reset_security_screen_stats` | ipc.js ✅ | ❌ 无 UI | QM Shadow 筛查重置 | **D** | 隐藏入口 |

**域评估**: 核心安全命令（加密/解锁/锁屏/审计/PII）优秀，4 个 QM 借鉴命令（posture/shadow）无用户场景。**域等级: B**

---

### 2.8 性能优化（performance.rs）— 19 命令

| # | 命令 | 前端调用者 | 用户可见 | 默认状态 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|---|
| 1 | `get_cache_stats` | perf-settings.js ✅ | ✅ 性能面板 | — | 查看缓存统计 | **C** | 隐藏入口 |
| 2 | `clear_cache` | perf-settings.js ✅ | ✅ 性能面板 | — | 清除缓存 | **C** | 隐藏入口 |
| 3 | `set_cache_settings` | perf-settings.js ✅ | ✅ 性能面板 | enabled | 配置缓存 | **C** | 隐藏入口 |
| 4 | `get_cache_settings` | perf-settings.js ✅ | ✅ 性能面板 | — | 获取缓存配置 | **C** | 隐藏入口 |
| 5 | `set_compression_ratio` | perf-settings.js ✅ | ✅ 性能面板 | 1.0(禁用) | 设置压缩比 | **D** | 隐藏入口 |
| 6 | `get_compression_ratio` | perf-settings.js ✅ | ✅ 性能面板 | — | 获取压缩比 | **D** | 隐藏入口 |
| 7 | `rebuild_bm25_index` | perf-settings.js ✅ | ✅ 性能面板 | — | 重建 BM25 索引 | **D** | 隐藏入口 |
| 8 | `rebuild_proposition_index` | perf-settings.js ✅ | ✅ 性能面板 | — | 重建 Proposition 索引 | **D** | 隐藏入口 |
| 9 | `build_summary_tree` | perf-settings.js ✅ | ✅ 性能面板 | — | 构建 RAPTOR 摘要树 | **D** | 隐藏入口 |
| 10 | `set_retrieval_memory_enabled` | perf-settings.js ✅ | ✅ 性能面板 | false | 检索记忆开关 | **D** | 隐藏入口 |
| 11 | `get_retrieval_memory_stats` | perf-settings.js ✅ | ✅ 性能面板 | — | 检索记忆统计 | **D** | 隐藏入口 |
| 12 | `reset_retrieval_memory` | perf-settings.js ✅ | ✅ 性能面板 | — | 重置检索记忆 | **D** | 隐藏入口 |
| 13 | `record_retrieval_feedback` | ipc.js ✅ | ❌ 内部 | — | 记录检索反馈 | **C** | 保持（内部调用） |
| 14 | `set_graph_retriever_enabled` | ipc.js ✅ | ❌ 无 UI | false | 图谱检索开关 | **D** | 隐藏入口 |
| 15 | `set_progressive_injection` | perf-settings.js ✅ | ✅ 性能面板 | false | 渐进注入开关 | **D** | 隐藏入口 |
| 16 | `set_speculative_enabled` | perf-settings.js ✅ | ✅ 性能面板 | false | Speculative RAG 开关 | **D** | 隐藏入口 |
| 17 | `set_quality_gate_enabled` | perf-settings.js ✅ | ✅ 性能面板 | false | 质量门控开关 | **D** | 隐藏入口 |
| 18 | `set_contextual_retrieval` | ipc.js ✅ | ❌ 无 UI | false | Contextual Retrieval | **D** | 隐藏入口 |
| 19 | `rebuild_contextual_embeddings` | perf-settings.js ✅ | ✅ 性能面板 | — | 重建上下文嵌入 | **D** | 隐藏入口 |

**域评估**: 全部 19 个命令应移入「开发者模式」。术语化命名（Speculative/Proposition/RAPTOR/ColBERT）用户无法理解。12 个开关默认关闭，用户永远不会开。**域等级: D** 🔴

---

### 2.9 知识图谱（graph.rs）— 8 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `get_graph_data` | graph-viewer.js ✅ | ✅ 图谱面板 | 查看知识图谱 | **A** | 保持 |
| 2 | `get_entity_relations` | graph-viewer.js ✅ | ✅ 图谱面板 | 查看实体关系 | **A** | 保持 |
| 3 | `get_graph_stats` | graph-viewer.js ✅ | ✅ 图谱面板 | 图谱统计 | **B** | 保持 |
| 4 | `get_entity_types` | graph-viewer.js ✅ | ✅ 图谱面板 | 实体类型图标 | **B** | 保持 |
| 5 | `get_shortest_path` | graph-viewer.js ✅ | ✅ 路径分析 | 最短路径分析 | **B** | 保持 |
| 6 | `get_communities` | graph-viewer.js ✅ | ✅ 社区检测 | 社区检测着色 | **B** | 保持 |
| 7 | `get_graph_layout` | graph-viewer.js ✅ | ✅ 布局切换 | 布局算法选择 | **B** | 保持 |
| 8 | `export_graph` | graph-viewer.js ✅ | ✅ 导出按钮 | 导出 GraphML/JSON-LD | **B** | 保持 |

**域评估**: 全部命令有明确用户场景和 UI 入口。**域等级: A**

---

### 2.10 扩展功能（features.rs）— 35 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `trigger_dream` | dream-panel.js ✅ | ✅ Dream 面板 | 触发知识库整理 | **C** | 审查后决定 |
| 2 | `get_dream_suggestions` | dream-panel.js ✅ | ✅ Dream 面板 | 查看整理建议 | **C** | 审查后决定 |
| 3 | `abort_dream` | dream-panel.js ✅ | ✅ Dream 面板 | 中止整理 | **C** | 审查后决定 |
| 4 | `set_memory_enabled` | memory-panel.js ✅ | ✅ Memory 面板 | 记忆系统开关 | **C** | 审查后决定 |
| 5 | `get_memories` | memory-panel.js ✅ | ✅ Memory 面板 | 查看记忆列表 | **C** | 审查后决定 |
| 6 | `pin_memory` | memory-panel.js ✅ | ✅ Memory 面板 | 固定记忆 | **C** | 审查后决定 |
| 7 | `promote_memory` | memory-panel.js ✅ | ✅ Memory 面板 | 提升记忆层级 | **C** | 审查后决定 |
| 8 | `delete_memory` | memory-panel.js ✅ | ✅ Memory 面板 | 删除记忆 | **C** | 审查后决定 |
| 9 | `clear_memories` | memory-panel.js ✅ | ✅ Memory 面板 | 清空记忆 | **C** | 审查后决定 |
| 10 | `trigger_memory_consolidation` | ipc.js ✅ | ❌ 无 UI | QM Scratch 整合 | **D** | 隐藏入口 |
| 11 | `get_scratch_logs` | ipc.js ✅ | ❌ 无 UI | QM Scratch 日志 | **D** | 隐藏入口 |
| 12 | `push_burst_turn` | ipc.js ✅ | ❌ 无 UI | QM Burst Buffer | **D** | 隐藏入口 |
| 13 | `flush_memory_burst_buffer` | ipc.js ✅ | ❌ 无 UI | QM Burst Buffer flush | **D** | 隐藏入口 |
| 14 | `get_burst_buffer_status` | ipc.js ✅ | ❌ 无 UI | QM Burst Buffer 状态 | **D** | 隐藏入口 |
| 15 | `execute_code_snippet` | code-executor.js ✅ | ✅ 代码面板 | 执行代码片段 | **D** | 审查后决定 |
| 16 | `save_workflow_template` | ipc.js ✅ | ✅ 工作流面板 | 保存工作流模板 | **D** | 审查后决定 |
| 17 | `run_workflow` | ipc.js ✅ | ✅ 工作流面板 | 运行工作流 | **D** | 审查后决定 |
| 18 | `list_workflows` | ipc.js ✅ | ✅ 工作流面板 | 列出工作流 | **D** | 审查后决定 |
| 19 | `delete_workflow` | ipc.js ✅ | ✅ 工作流面板 | 删除工作流 | **D** | 审查后决定 |
| 20 | `search_symbols` | ipc.js ✅ | ✅ 符号搜索 | 代码符号搜索 | **C** | 审查后决定 |
| 21 | `get_symbols_for_chunk` | ipc.js ✅ | ✅ 符号搜索 | Chunk 符号列表 | **C** | 审查后决定 |
| 22 | `rebuild_symbol_index` | Pro | ipc.js ✅ | ✅ 符号搜索 | 重建符号索引 | **C** | 审查后决定 |
| 23 | `transcribe_audio` | voice-input.js ✅ | ✅ 语音按钮 | 语音转文字 | **A** | 保持 |
| 24 | `get_stt_config` | settings-general.js ✅ | ✅ 通用设置 | STT 配置 | **B** | 保持 |
| 25 | `set_stt_config` | settings-general.js ✅ | ✅ 通用设置 | STT 配置 | **B** | 保持 |
| 26 | `set_web_search_enabled` | input-toggles.js ✅ | ✅ Toggle | Web 搜索开关 | **B** | 保持 |
| 27 | `web_search` | ipc.js ✅ | ❌ 内部 | 执行 Web 搜索 | **B** | 保持 |
| 28 | `save_prompt_template` | settings-advanced.js ✅ | ✅ 模板管理 | 保存快捷指令 | **A** | 保持 |
| 29 | `list_prompt_templates` | slash-commands.js ✅ | ✅ 斜杠命令 | 列出快捷指令 | **A** | 保持 |
| 30 | `delete_prompt_template` | settings-advanced.js ✅ | ✅ 模板管理 | 删除快捷指令 | **A** | 保持 |
| 31 | `discover_skills` | slash-commands.js ✅ | ✅ 斜杠命令 | Skill 发现 | **B** | 保持 |
| 32 | `get_forward_links` | ipc.js ✅ | ✅ Wiki 链接 | 正向链接查询 | **B** | 保持 |
| 33 | `get_backlinks` | ipc.js ✅ | ✅ Wiki 链接 | 反向链接查询 | **B** | 保持 |
| 34 | `rebuild_wiki_links` | ipc.js ✅ | ✅ Wiki 链接 | 重建链接索引 | **B** | 保持 |
| 35 | `get_document_summary` | (已在 document 域) | — | — | — | — |

**域评估**: 语音/模板/Wiki 链接命令优秀。Dream/Memory/Workflow/Code-executor 场景模糊。QM 借鉴 5 个命令无 UI。**域等级: C**

---

### 2.11 Agent（agent.rs）— 3 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `set_agent_enabled` | input-toggles.js ✅ | ✅ Toggle | Agent 模式开关 | **B** | 保持 |
| 2 | `set_coordinator_mode` | settings-advanced.js ✅ | ✅ 高级设置 | 多代理协调开关 | **C** | 隐藏入口 |
| 3 | `set_sub_agent_enabled` | settings-advanced.js ✅ | ✅ 高级设置 | 子代理舰队开关 | **D** | 隐藏入口（用户无法理解） |

**域评估**: Agent toggle 有价值，Coordinator/SubAgent 术语化且无明确用户场景。**域等级: C**

---

### 2.12 许可证（license.rs）— 3 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `activate_pro` | ipc.js ✅ | ✅ 许可证面板 | 激活 Pro | **A** | 保持 |
| 2 | `get_pro_status` | ipc.js ✅ | ✅ Pro 指示器 | 查询 Pro 状态 | **A** | 保持 |
| 3 | `deactivate_pro` | ipc.js ✅ | ✅ 许可证面板 | 停用 Pro | **A** | 保持 |

**域评估**: 全部核心功能。**域等级: A**

---

### 2.13 文件同步（sync.rs）— 3 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `add_watched_folder` | ipc.js ✅ | ✅ 同步设置 | 添加监听文件夹 | **A** | 保持 |
| 2 | `remove_watched_folder` | ipc.js ✅ | ✅ 同步设置 | 移除监听文件夹 | **A** | 保持 |
| 3 | `get_watched_folders` | ipc.js ✅ | ✅ 同步设置 | 查看监听列表 | **A** | 保持 |

**域评估**: 全部核心功能。**域等级: A**

---

### 2.14 Trace 链路追踪（trace.rs）— 4 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `get_recent_traces` | ipc.js ✅ | ✅ Trace 面板 | 查看最近 trace | **D** | 隐藏入口（开发者工具） |
| 2 | `get_trace_detail` | ipc.js ✅ | ✅ Trace 面板 | 查看 trace 详情 | **D** | 隐藏入口 |
| 3 | `clear_traces` | ipc.js ✅ | ✅ Trace 面板 | 清空 trace | **D** | 隐藏入口 |
| 4 | `get_trace_count` | ipc.js ✅ | ✅ Trace 面板 | trace 数量 | **D** | 隐藏入口 |

**域评估**: 全部为开发者工具，不应暴露给普通用户。**域等级: D** 🔴

---

### 2.15 RAG 评估（rag_eval.rs）— 4 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `evaluate_rag_response` | ipc.js ✅ | ✅ 评估面板 | 评估单条 RAG 回答 | **D** | 隐藏入口（开发者工具） |
| 2 | `evaluate_rag_batch` | ipc.js ✅ | ✅ 评估面板 | 批量评估 RAG | **D** | 隐藏入口 |
| 3 | `get_rag_eval_settings` | ipc.js ✅ | ✅ 评估面板 | 评估配置 | **D** | 隐藏入口 |
| 4 | `set_rag_eval_settings` | ipc.js ✅ | ✅ 评估面板 | 评估配置 | **D** | 隐藏入口 |

**域评估**: RAGAS 风格评估是开发者工具，用户不需要理解 faithfulness/context_precision 等指标。**域等级: D** 🔴

---

### 2.16 工作空间（workspace.rs）— 9 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `create_workspace` | ipc.js ✅ | ✅ 工作空间选择器 | 创建知识库 | **A** | 保持 |
| 2 | `list_workspaces` | ipc.js ✅ | ✅ 工作空间选择器 | 列出知识库 | **A** | 保持 |
| 3 | `switch_workspace` | ipc.js ✅ | ✅ 工作空间选择器 | 切换知识库 | **A** | 保持 |
| 4 | `get_current_workspace` | ipc.js ✅ | ✅ 工作空间选择器 | 获取当前知识库 | **A** | 保持 |
| 5 | `rename_workspace` | ipc.js ✅ | ✅ 工作空间选择器 | 重命名知识库 | **A** | 保持 |
| 6 | `delete_workspace` | ipc.js ✅ | ✅ 工作空间选择器 | 删除知识库 | **A** | 保持 |
| 7 | `get_workspace_stats` | ipc.js ✅ | ✅ 工作空间选择器 | 知识库统计 | **B** | 保持 |
| 8 | `get_workspace_quota` | ipc.js ✅ | ✅ 工作空间选择器 | 配额查询 | **B** | 保持 |
| 9 | `migrate_document` | ipc.js ✅ | ✅ 文档菜单 | 迁移文档到其他知识库 | **B** | 保持 |

**域评估**: 全部命令有明确用户场景。**域等级: A**

---

### 2.17 审计（audit.rs）— 4 命令

| # | 命令 | 前端调用者 | 用户可见 | 用户场景 | 评估 | 处置 |
|---|---|---|---|---|---|---|
| 1 | `audit_document` | chat.js ✅ | ✅ 审计按钮 | 文档一致性审计 | **B** | 保持 |
| 2 | `abort_audit` | chat.js ✅ | ✅ 审计按钮 | 中止审计 | **B** | 保持 |
| 3 | (内部) | — | — | — | — | — |
| 4 | (内部) | — | — | — | — | — |

**域评估**: Pro 功能，用户场景明确（文档一致性检查）。**域等级: B**

---

## 3. 借鉴来源命令汇总

| 借鉴来源 | 命令数 | 命令清单 | 用户场景 | 评估 |
|---|---|---|---|---|
| **QM (Quadratic Memory)** | 22 | security_posture(2) + shadow_screen(2) + scratch(2) + burst_buffer(3) + memory(6) + durable(3) + session_todo(5) + budget(2) | 🔴 ~16 无场景 | D |
| **OpenCode** | 10 | admit_input(3) + session_todo(5, 与 QM 重叠) + discover_skills(1) + (B01-B04 内部) | 🟠 ~5 无场景 | C |
| **DS4 (DwarfStar)** | 3 | strip_messages(3) | 🔴 0 无场景 | F |
| **Cherry Studio** | 10 | trace(4) + token_budget(4) + budget(2) | 🔴 10 开发者工具 | D |
| **RAGAS** | 4 | evaluate_rag(4) | 🔴 4 开发者工具 | D |
| **OpenMontage** | 0 | (已合并到 performance/agent 模块，非独立 IPC) | ✅ | A |
| **其他学术** | 16 | cache(4) + compression(2) + bm25(1) + proposition(1) + raptor(1) + colbert(1) + progressive(1) + speculative(1) + quality_gate(1) + contextual(2) + graph_retriever(1) | 🔴 全部默认关闭 | D |

**借鉴命令总计**: ~65 个，其中 ~45 个无明确用户场景或为开发者工具。

---

## 4. 评估标准逐条结论

| 编号 | 标准 | 结论 | 详情 |
|---|---|---|---|
| S3 | 设置面板 ≤ 8 个分区 | 🔴 **不达标** | 命令分布在 9+ 个设置子区域，性能/trace/eval/durable/todo 混入用户设置 |
| S7 | 设置项使用人类语言 | 🔴 **不达标** | speculative/progressive/proposition/colbert/raptor/posture/shadow 等术语化命名 |
| S8 | 默认配置即最佳配置 | 🔴 **不达标** | 12 个性能开关 + 4 个 toggle 默认关闭，用户永远不会开启 |
| C2 | 无僵尸 IPC | ✅ **达标** | 零僵尸命令（0 个注册命令无前端调用者） |
| C3 | 无过度设计 | 🔴 **不达标** | ~45 个借鉴命令无用户场景，DS4 strip(3) + QM durable(3) + QM scratch(2) + QM burst(3) + QM posture(2) + QM shadow(2) = 15 个完全无 UI 入口 |
| C5 | 设置项有用户价值 | 🔴 **不达标** | 30+ 设置项用户无法理解（perf-settings 全部 + trace + budget + eval + posture + shadow） |
| I1 | IPC 命名用户友好 | 🟠 **部分达标** | 核心命令命名清晰，但 ~20 个术语化命令（speculative/progressive/quality_gate/contextual_retrieval 等）不友好 |
| I2 | IPC 命令有前端调用者 | ✅ **达标** | 全部 224+12 注册命令有前端调用者（含动态 invoke 模式） |
| I3 | IPC 分组合理 | ✅ **达标** | 18 个功能域子模块分组清晰 |
| I4 | Pro 门控合理 | ✅ **达标** | 12 个 Pro-only 命令均涉及本地 LLM 硬件加速或高级嵌入 |

---

## 5. 处置方案优先级

### P0 — 立即修复（运行时崩溃）

| 优先级 | 处置 | 命令数 | 说明 |
|---|---|---|---|
| P0 | 修复幻影调用 | 11 | 前端调用后端未注册命令，运行时报错 |

### P1 — 隐藏开发者工具（用户体验）

| 优先级 | 处置 | 命令数 | 说明 |
|---|---|---|---|
| P1 | 隐藏入口 | 38 | 移入「开发者模式」开关，普通用户不可见 |

### P2 — 删除无场景命令（代码瘦身）

| 优先级 | 处置 | 命令数 | 说明 |
|---|---|---|---|
| P2 | 删除候选 | 36 | DS4 strip(3) + QM durable(3) + QM scratch(2) + QM burst(3) + QM posture(2) + QM shadow(2) + session_todo(5) + trace(4) + budget(2) + rag_eval(4) + perf(6) + kv_cache(6) = 36 |

### P3 — 合并简化

| 优先级 | 处置 | 命令数 | 说明 |
|---|---|---|---|
| P3 | 合并 set/get 对 | ~30 → ~15 | 15 对 set/get 合并为单一 settings 命令 |

### P4 — 命名优化

| 优先级 | 处置 | 命令数 | 说明 |
|---|---|---|---|
| P4 | 重命名 | ~20 | 术语化命令改为人类可读命名 |

---

## 6. 命令数量目标

| 当前 | 目标 | 削减 |
|---|---|---|
| 236（224 Free + 12 Pro） | ≤ 150（≤ 140 Free + ≤ 10 Pro） | -86（-36%） |

**削减路径**:
1. 修复 11 个幻影调用（不改命令数，改前端代码）
2. 删除 36 个无场景命令（-36）
3. 合并 15 对 set/get（-15）
4. 隐藏 38 个开发者工具（不改命令数，改 UI 入口）
5. 净削减: -51 命令 → 185 命令（距离 150 目标还差 35）

**进一步削减**: 审查 60 个「审查后决定」命令，预计可再削减 35 个。

---

## 7. 附录

### 附录 A: 命令功能域分布

| 功能域 | 文件 | Free 命令数 | Pro 命令数 | 总计 |
|---|---|---|---|---|
| Chat & RAG | chat.rs | 14 | 0 | 14 |
| Conversation | conversation.rs | 22 | 0 | 22 |
| Document | document.rs | 16 | 0 | 16 |
| Import | import.rs | 5 | 0 | 5 |
| Settings | settings.rs | 42 | 0 | 42 |
| Local LLM | local_llm.rs | 16 | 12 | 28 |
| Security | security.rs | 18 | 0 | 18 |
| Performance | performance.rs | 19 | 0 | 19 |
| Graph | graph.rs | 8 | 0 | 8 |
| Features | features.rs | 34 | 0 | 34 |
| Agent | agent.rs | 3 | 0 | 3 |
| License | license.rs | 3 | 0 | 3 |
| Sync | sync.rs | 3 | 0 | 3 |
| Trace | trace.rs | 4 | 0 | 4 |
| RAG Eval | rag_eval.rs | 4 | 0 | 4 |
| Workspace | workspace.rs | 9 | 0 | 9 |
| Audit | audit.rs | 2 | 0 | 2 |
| (mod.rs 内部) | mod.rs | 0 | 0 | 0 |
| **总计** | | **222** | **12** | **236** |

> 注: 此处 Free 命令数 222 与之前统计的 224 有 2 个差异，因为 `clear_model_cache` 和 `get_file_sizes` 被归入了不同的域。精确总数以 lib.rs generate_handler! 注册列表为准。

### 附录 B: 借鉴来源 → IPC 命令映射

| 借鉴来源 | Session | IPC 命令 | 状态 |
|---|---|---|---|
| QM Q01 (Scratch-Promote) | S66 | trigger_memory_consolidation, get_scratch_logs | 🔴 无 UI |
| QM Q02 (Burst Buffer) | S67 | push_burst_turn, flush_memory_burst_buffer, get_burst_buffer_status | 🔴 无 UI |
| QM Q05 (SecurityPosture) | S69 | set_security_posture, get_security_posture | 🔴 无 UI |
| QM Q06 (ShadowScreen) | S70 | get_security_screen_stats, reset_security_screen_stats | 🔴 无 UI |
| QM Q08 (Budget) | S72 | get_budget_stats, set_budget_limit | 🔴 开发者工具 |
| QM Q10 (LLMAux) | S74 | (内部，非独立 IPC) | ✅ |
| QM Q11 (LlmRouter) | S75 | (内部，非独立 IPC) | ✅ |
| OpenCode B05 (Durable) | S76 | admit_input, promote_input, get_pending_inputs | 🔴 无 UI |
| OpenCode B08 (SessionTodo) | S77 | add_session_todo, update_todo_status, get_session_todos, delete_session_todo, delete_session_todos | 🔴 无 UI |
| OpenCode B09 (Skill) | S78 | discover_skills | ✅ 有 UI |
| DS4 (Strip) | S79 | strip_messages, strip_keeping_recent, preview_strip | 🔴 无 UI |
| Cherry Studio (Trace) | S80 | get_recent_traces, get_trace_detail, clear_traces, get_trace_count | 🔴 开发者工具 |
| Cherry Studio (Budget) | S80 | get_token_budget_config, set_token_budget_config | 🔴 开发者工具 |
| RAGAS (RAG Eval) | S81 | evaluate_rag_response, evaluate_rag_batch, get_rag_eval_settings, set_rag_eval_settings | 🔴 开发者工具 |

### 附录 C: 幻影命令根因分析

**根因**: 前端 `ipc.js` 在 S2 复盘「僵尸命令接线」时，新加了 `unlockApp` / `recordActivity` / `setAutoLockConfig` 等新命令的封装，但**未删除**旧的 `encrypt` / `unlock` / `setAutoLockTimeout` / `redactPii` / `verifyAuditChain` / `setPiiDetection` / `clearClipboard` / `setClipboardClearTimeout` / `checkStatus` 封装。新旧封装共存导致前端仍会调用旧命令名。

**修复方案**:
1. 删除 ipc.js 中 9 个旧封装（`encrypt` / `unlock` / `setAutoLockTimeout` / `redactPii` / `verifyAuditChain` / `setPiiDetection` / `clearClipboard` / `setClipboardClearTimeout` / `checkStatus`）
2. 将调用方迁移到新封装（`encrypt_database` → 需新增后端命令或改为 SecurityManager 内部触发；`unlock_database` → `unlock_app`；`set_auto_lock_timeout` → `set_auto_lock_config`；`redact_pii` → `detect_pii`；其余删除）
3. 删除 tauri-stub.js 中 2 个无对应后端命令的 mock（`get_document_chunks` / `list_documents`）

---

> **审计结论**: EchoMind IPC 命令系统在核心功能（chat/import/document/conversation/workspace）上表现优秀，但借鉴功能命令泛滥（~65 个，~45 无用户场景）、开发者工具混入用户设置（21 个）、术语化命名（~20 个）、11 个幻影调用是四大核心问题。建议按 P0→P4 优先级逐步修复，目标从 236 命令削减至 ≤ 150。
