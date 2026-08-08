//! EchoMind Tauri 组装层（library 形态导出，供入口与 L2 集成测试复用）。
//! 体系三铁律：生产代码严禁 unwrap()/expect()；启动错误显式上报并以非零码退出。

pub mod commands;
pub mod settings_registry;
pub mod state;
pub mod stores;

use std::path::PathBuf;

use echomind_core::Storage as _;
use state::AppState;
use tauri::Emitter;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Manager, PhysicalPosition, PhysicalSize, WindowEvent};

/// 窗口最小尺寸约束（与 tauri.conf.json minWidth/minHeight 保持一致）
const WIN_MIN_WIDTH: i32 = 960;
const WIN_MIN_HEIGHT: i32 = 640;

/// 应用入口（main.rs 调用）。
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 系统菜单栏（精简方案：EchoMind / File / Edit / Window / Help）
            setup_menu(app)?;

            // E2E 测试钩子：ECHOMIND_DATA_DIR 环境变量可重定向数据目录（隔离测试数据）
            let data_dir = match std::env::var("ECHOMIND_DATA_DIR") {
                Ok(custom) => PathBuf::from(custom),
                Err(_) => app.path().app_data_dir()?,
            };
            std::fs::create_dir_all(&data_dir)?;
            // REQ-SEC-004：数据目录权限 0700（仅所有者可读写执行）
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o700));
            }
            let state = tauri::async_runtime::block_on(AppState::new(data_dir))
                .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:#}").into() })?;

            // REQ-ERR-004：启动时数据库完整性检查
            let integrity = tauri::async_runtime::block_on(state.storage.check_integrity())
                .unwrap_or(echomind_infra::sqlite_storage::IntegrityCheckResult::Ok);
            if let echomind_infra::sqlite_storage::IntegrityCheckResult::Corrupted(ref msg) =
                integrity
            {
                eprintln!("数据库完整性检查失败: {msg}");
                let _ = app.emit("db-integrity-error", msg.to_string());
            }

            // REQ-WIN-001：恢复窗口位置/尺寸/最大化状态
            tauri::async_runtime::block_on(restore_window_state(app, &state));

            // **性能优化（出答案速度）**：启动时后台预热向量化引擎（ONNX 模型加载到内存），
            // 消除首次对话时「初始化向量化引擎」的卡顿。
            //
            // **重要**：仅当模型文件已齐备时才预热。文件缺失时不触发下载——
            // 下载由首启向导（init_embedder 命令 + 进度事件）负责，避免与向导竞争。
            app.manage(state);
            {
                let app_for_warm = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_for_warm.state::<AppState>();
                    // 先检查文件是否齐备，避免与向导下载竞争
                    let cache_dir = state.data_dir.join("models");
                    let status = tokio::task::spawn_blocking(move || {
                        echomind_infra::local_embedder::LocalEmbedder::check_status(&cache_dir)
                    })
                    .await;
                    let should_warm = matches!(
                        status,
                        Ok(echomind_infra::local_embedder::EmbedderStatus::Ready)
                    );
                    if !should_warm {
                        eprintln!("[PERF] embedder 预热跳过（模型文件未齐备，由向导负责下载）");
                        return;
                    }
                    let t = std::time::Instant::now();
                    match state.embedder().await {
                        Ok(_) => {
                            eprintln!("[PERF] embedder 预热完成: {}ms", t.elapsed().as_millis())
                        }
                        Err(e) => {
                            eprintln!("[PERF] embedder 预热失败（首次对话将延迟初始化）: {e:#}")
                        }
                    }
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // REQ-WIN-001：窗口关闭时保存位置/尺寸/最大化状态
            if let WindowEvent::CloseRequested { .. } = event {
                save_window_state(window);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::import_files,
            commands::get_file_sizes,
            commands::get_file_size_limits,
            commands::chat,
            commands::create_conversation,
            commands::get_conversations,
            commands::get_conversations_paginated,
            commands::get_messages,
            commands::get_messages_paginated,
            commands::edit_user_message,
            commands::set_turn_active_version,
            commands::get_turn_active_versions,
            commands::delete_conversation,
            commands::abort_chat,
            commands::abort_import,
            commands::test_llm_connection,
            commands::update_llm_config,
            commands::get_settings,
            commands::set_vlm_enabled,
            commands::set_hybrid_search,
            commands::set_rerank_enabled,
            commands::set_hyde_enabled,
            commands::set_agent_enabled,
            // 多代理协调模式（REQ-RAG-025）
            commands::set_coordinator_mode,
            // 子代理舰队模式（REQ-RAG-025 扩展）
            commands::set_sub_agent_enabled,
            commands::set_embedding_model,
            commands::set_context_token_limit,
            commands::list_local_models,
            commands::get_recommended_models,
            commands::download_model,
            commands::pause_download,
            commands::abort_download,
            commands::get_download_status,
            commands::list_pending_downloads,
            commands::cleanup_partial_downloads,
            commands::scan_download_recovery,
            commands::delete_model,
            commands::set_llm_mode,
            commands::get_llm_mode,
            commands::set_local_model,
            #[cfg(feature = "pro")]
            commands::get_local_llm_device_kind,
            #[cfg(feature = "pro")]
            commands::set_paged_attn,
            #[cfg(feature = "pro")]
            commands::set_sampling_params,
            #[cfg(feature = "pro")]
            commands::set_kernel_mode,
            #[cfg(feature = "pro")]
            commands::get_kernel_mode,
            // KV cache 跨会话复用（Phase 4 Session 25，REQ-LLM-009）
            #[cfg(feature = "pro")]
            commands::save_kv_cache,
            #[cfg(feature = "pro")]
            commands::load_kv_cache,
            #[cfg(feature = "pro")]
            commands::clear_kv_cache,
            #[cfg(feature = "pro")]
            commands::get_kv_cache_status,
            #[cfg(feature = "pro")]
            commands::set_kv_cache_enabled,
            commands::get_documents,
            commands::delete_document,
            commands::retry_index,
            commands::reclassify_document,
            commands::activate_pro,
            commands::deactivate_pro,
            commands::get_pro_status,
            commands::audit_document,
            commands::abort_audit,
            commands::init_embedder,
            commands::check_embedder_status,
            commands::get_model_cache_info,
            commands::clear_model_cache,
            commands::get_locale,
            commands::set_locale,
            // REQ-UI-011：主题偏好持久化
            commands::get_theme,
            commands::set_theme,
            // REQ-NAV-001：侧栏折叠状态持久化
            commands::get_sidebar_collapsed,
            commands::set_sidebar_collapsed,
            // S69: Token 预算配置（Cherry Studio 借鉴 — token-budget 驱动 in-loop compaction）
            commands::get_token_budget_config,
            commands::set_token_budget_config,
            // S70: Trace 系统（Cherry Studio 借鉴 — RAG 链路追踪）
            commands::get_recent_traces,
            commands::get_trace_detail,
            commands::clear_traces,
            commands::get_trace_count,
            // 导出功能（REQ-EXP-001）
            commands::export_conversation_markdown,
            commands::save_text_file,
            // 文件监听 + 增量同步（REQ-SYNC-001~003）
            commands::add_watched_folder,
            commands::remove_watched_folder,
            commands::get_watched_folders,
            // 安全防御 IPC 命令
            commands::get_security_status,
            commands::set_auto_lock_config,
            commands::lock_app,
            commands::unlock_app,
            commands::record_activity,
            commands::detect_pii,
            commands::set_panic_wipe_password,
            commands::clear_panic_wipe_password,
            commands::is_panic_wipe_enabled,
            commands::set_clipboard_config,
            commands::get_audit_logs,
            commands::clear_audit_logs,
            commands::check_password_strength,
            // 安全态势分层（Q05 借鉴 QM SecurityPosture）
            commands::set_security_posture,
            commands::get_security_posture,
            // Shadow 安全筛查模式（Q06 借鉴 QM security-screen.ts）
            commands::get_security_screen_stats,
            commands::reset_security_screen_stats,
            // 可观测性 IPC 命令（REQ-OBS-001 / REQ-OBS-002）
            commands::get_log_level,
            commands::set_log_level,
            commands::export_logs,
            commands::export_diagnostics,
            // 数据目录操作（REQ-ERR-004-AC-3）
            commands::open_data_dir,
            // Token 用量追踪与预算
            commands::get_conversation_cost,
            commands::set_token_budget,
            // 语义缓存（REQ-PERF-001）
            commands::get_cache_stats,
            commands::clear_cache,
            commands::set_cache_settings,
            commands::get_cache_settings,
            // Prompt 压缩（REQ-PERF-002）
            commands::set_compression_ratio,
            commands::get_compression_ratio,
            // Contextual BM25 + 实体链接（REQ-PERF-005/006）
            commands::rebuild_bm25_index,
            // Proposition 级原子分割（REQ-PERF-007）
            commands::rebuild_proposition_index,
            // RAPTOR 摘要树（REQ-PERF-009）
            commands::build_summary_tree,
            // ColBERT 多向量嵌入模式切换（REQ-PERF-008, Pro feature）
            #[cfg(feature = "pro")]
            commands::set_embedder_model,
            // 后台空闲整理引擎（Auto Dream Engine）
            commands::trigger_dream,
            commands::get_dream_suggestions,
            commands::abort_dream,
            // 自进化检索记忆（REQ-PERF-012）
            commands::set_retrieval_memory_enabled,
            commands::get_retrieval_memory_stats,
            commands::reset_retrieval_memory,
            commands::record_retrieval_feedback,
            // 知识图谱图遍历检索（REQ-RAG-027）
            commands::set_graph_retriever_enabled,
            // 渐进式注入 / Speculative RAG / 质量门控（REQ-PERF-010/011, REQ-RAG-028）
            commands::set_progressive_injection,
            commands::set_speculative_enabled,
            commands::set_quality_gate_enabled,
            // 知识图谱可视化（REQ-RAG-027 前端图谱面板）
            commands::get_graph_data,
            commands::get_entity_relations,
            commands::get_graph_stats,
            // 知识图谱可视化增强（Session 4：实体类型图标 + 子图过滤 + 搜索定位 + 导出）
            commands::get_entity_types,
            // 知识图谱高级分析（Session 5：最短路径 + 社区检测 + 布局切换）
            commands::get_shortest_path,
            commands::get_communities,
            commands::get_graph_layout,
            // 知识图谱导出（REQ-EXP-006 GraphML/JSON-LD）
            commands::export_graph,
            // 文档摘要自动生成（REQ-ING-019 导入时 LLM 生成摘要）
            commands::get_document_summary,
            commands::regenerate_summary,
            // 代码符号引擎（REQ-RAG-031 代码感知 RAG）
            commands::search_symbols,
            commands::get_symbols_for_chunk,
            #[cfg(feature = "pro")]
            commands::rebuild_symbol_index,
            // DAG 工作流引擎（REQ-RAG-030）
            commands::save_workflow_template,
            commands::run_workflow,
            commands::list_workflows,
            commands::delete_workflow,
            // 代码执行沙箱（REQ-RAG-032，Pro feature）
            commands::execute_code_snippet,
            // 持久化记忆系统（REQ-RAG-032：IfAI 三层记忆 Wing/Hall/Room）
            commands::set_memory_enabled,
            commands::get_memories,
            commands::pin_memory,
            commands::promote_memory,
            commands::delete_memory,
            commands::clear_memories,
            // Scratch-Promote 记忆整合（Q01 借鉴 QM scratch-promote + consolidation）
            commands::trigger_memory_consolidation,
            commands::get_scratch_logs,
            // Burst Buffer 延迟批量记忆提取（Q02 借鉴 QM createBurstBuffer）
            commands::push_burst_turn,
            commands::flush_memory_burst_buffer,
            commands::get_burst_buffer_status,
            // 网页搜索集成（REQ-RAG-036：本地检索不足时搜索互联网）
            commands::set_web_search_enabled,
            commands::web_search,
            // 自定义 ONNX 嵌入模型上传（REQ-VEC-014，Pro 门控；Free 返回 PRO_REQUIRED）
            commands::upload_custom_embedding_model,
            commands::list_custom_models,
            commands::delete_custom_model,
            // 对话模板/快捷指令系统（S56 自定义快捷指令模板）
            commands::save_prompt_template,
            commands::list_prompt_templates,
            commands::delete_prompt_template,
            // Wiki 双向链接（REQ-ING-020 Markdown 笔记双向链接）
            commands::get_forward_links,
            commands::get_backlinks,
            commands::rebuild_wiki_links,
            // 对话分支/版本树（REQ-RAG-039）
            commands::get_conversation_tree,
            commands::branch_from_message,
            // 对话全文搜索（REQ-RAG-040）
            commands::search_conversations,
            // Contextual Retrieval 上下文增强嵌入（REQ-RAG-041）
            commands::set_contextual_retrieval,
            commands::rebuild_contextual_embeddings,
            // 文档标签系统（REQ-ING-022 用户自定义标签管理）
            commands::add_document_tag,
            commands::remove_document_tag,
            commands::list_all_tags,
            commands::filter_documents_by_tag,
            // Durable Prompt Admission（B05 持久化提示接纳）
            commands::admit_input,
            commands::promote_input,
            commands::get_pending_inputs,
            // Session Todo 持久化（B08 会话待办持久化）
            commands::add_session_todo,
            commands::update_todo_status,
            commands::get_session_todos,
            commands::delete_session_todo,
            commands::delete_session_todos,
            // Q08: 预算追踪（QM 借鉴 — LLM API 费用控制和速率限制）
            commands::get_budget_stats,
            commands::set_budget_limit,
        ]);

    if let Err(err) = builder.run(tauri::generate_context!()) {
        eprintln!("EchoMind 启动失败: {err}");
        std::process::exit(1);
    }
}

// ============================================================================
// REQ-WIN-001：窗口尺寸与约束 — 位置/尺寸/最大化状态持久化
// ============================================================================

/// 从 settings 表读取窗口状态并恢复（REQ-WIN-001-AC-3/AC-5）。
///
/// 仅在值有效时恢复（x/y ≥ 0，width/height ≥ minWidth/minHeight）。
/// 最大化状态优先恢复（最大化时位置/尺寸由系统管理）。
async fn restore_window_state(app: &tauri::App, state: &AppState) {
    let maximized = state
        .storage
        .get_setting("window.maximized")
        .await
        .ok()
        .flatten()
        .is_some_and(|v| v == "true");

    if maximized {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.maximize();
        }
        return;
    }

    let x = state.storage.get_setting("window.x").await.ok().flatten();
    let y = state.storage.get_setting("window.y").await.ok().flatten();
    let width = state
        .storage
        .get_setting("window.width")
        .await
        .ok()
        .flatten();
    let height = state
        .storage
        .get_setting("window.height")
        .await
        .ok()
        .flatten();

    // 验证值有效性（x/y ≥ 0, width/height ≥ 最小约束）
    let valid_x = x.and_then(|v| v.parse::<i32>().ok()).filter(|&v| v >= 0);
    let valid_y = y.and_then(|v| v.parse::<i32>().ok()).filter(|&v| v >= 0);
    let valid_w = width
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|&v| v >= WIN_MIN_WIDTH);
    let valid_h = height
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|&v| v >= WIN_MIN_HEIGHT);

    if let Some(window) = app.get_webview_window("main") {
        if let (Some(x), Some(y)) = (valid_x, valid_y) {
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
        if let (Some(w), Some(h)) = (valid_w, valid_h) {
            let _ = window.set_size(PhysicalSize::new(w, h));
        }
    }
}

/// 保存窗口位置/尺寸/最大化状态到 settings 表（REQ-WIN-001-AC-3）。
///
/// 在 `on_window_event(CloseRequested)` 中同步调用。
/// 异步写入通过 `tauri::async_runtime::spawn` 在后台执行，避免阻塞窗口关闭。
fn save_window_state(window: &tauri::Window) {
    let pos = window.outer_position().ok();
    let size = window.outer_size().ok();
    let maximized = window.is_maximized().unwrap_or(false);

    let app_handle = window.app_handle().clone();

    tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<AppState>();

        if maximized {
            let _ = state.storage.set_setting("window.maximized", "true").await;
            return;
        }

        // 非最大化时保存位置和尺寸
        let _ = state.storage.set_setting("window.maximized", "false").await;
        if let Some(pos) = pos {
            let _ = state
                .storage
                .set_setting("window.x", pos.x.to_string().as_str())
                .await;
            let _ = state
                .storage
                .set_setting("window.y", pos.y.to_string().as_str())
                .await;
        }
        if let Some(sz) = size {
            let _ = state
                .storage
                .set_setting("window.width", sz.width.to_string().as_str())
                .await;
            let _ = state
                .storage
                .set_setting("window.height", sz.height.to_string().as_str())
                .await;
        }
    });
}

/// 配置系统菜单栏。
/// 精简方案：EchoMind(About/Quit) · File(Import) · Edit(标准编辑项) · Window(Minimize/Zoom) · Help(About)
/// 菜单文案为英文（macOS 系统菜单惯例），后续可通过 i18n 动态切换。
fn setup_menu(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // EchoMind 菜单（macOS app menu，Windows/Linux 显示为普通菜单）
    let app_menu = Submenu::with_items(
        app,
        "EchoMind",
        true,
        &[
            &MenuItem::with_id(app, "about", "About EchoMind", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit EchoMind", true, None::<&str>)?,
        ],
    )?;

    // File 菜单
    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[&MenuItem::with_id(
            app,
            "import_files",
            "Import Files…",
            true,
            None::<&str>,
        )?],
    )?;

    // Edit 菜单（标准编辑项，由系统提供实现）
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, Some("Undo"))?,
            &PredefinedMenuItem::redo(app, Some("Redo"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some("Cut"))?,
            &PredefinedMenuItem::copy(app, Some("Copy"))?,
            &PredefinedMenuItem::paste(app, Some("Paste"))?,
            &PredefinedMenuItem::select_all(app, Some("Select All"))?,
        ],
    )?;

    // Window 菜单
    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some("Minimize"))?,
            &MenuItem::with_id(app, "zoom", "Zoom", true, None::<&str>)?,
        ],
    )?;

    // Help 菜单
    let help_menu = Submenu::with_items(
        app,
        "Help",
        true,
        &[&MenuItem::with_id(
            app,
            "help_about",
            "About EchoMind",
            true,
            None::<&str>,
        )?],
    )?;

    let menu = Menu::with_items(
        app,
        &[&app_menu, &file_menu, &edit_menu, &window_menu, &help_menu],
    )?;
    app.set_menu(menu)?;
    Ok(())
}
