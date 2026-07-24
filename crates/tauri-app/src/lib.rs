//! EchoMind Tauri 组装层（library 形态导出，供入口与 L2 集成测试复用）。
//! 体系三铁律：生产代码严禁 unwrap()/expect()；启动错误显式上报并以非零码退出。

pub mod commands;
pub mod state;

use std::path::PathBuf;

use state::AppState;
use tauri::Manager;

/// 应用入口（main.rs 调用）。
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // E2E 测试钩子：ECHOMIND_DATA_DIR 环境变量可重定向数据目录（隔离测试数据）
            let data_dir = match std::env::var("ECHOMIND_DATA_DIR") {
                Ok(custom) => PathBuf::from(custom),
                Err(_) => app.path().app_data_dir()?,
            };
            std::fs::create_dir_all(&data_dir)?;
            let state = tauri::async_runtime::block_on(AppState::new(data_dir))
                .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:#}").into() })?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::import_files,
            commands::chat,
            commands::create_conversation,
            commands::get_conversations,
            commands::get_messages,
            commands::delete_conversation,
            commands::abort_chat,
            commands::test_llm_connection,
            commands::update_llm_config,
            commands::get_settings,
            commands::get_documents,
            commands::delete_document,
            commands::activate_pro,
            commands::get_pro_status,
        ]);

    if let Err(err) = builder.run(tauri::generate_context!()) {
        eprintln!("EchoMind 启动失败: {err}");
        std::process::exit(1);
    }
}
