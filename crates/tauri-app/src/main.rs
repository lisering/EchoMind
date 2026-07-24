//! EchoMind（灵犀）应用入口。
//! Phase 1：启动骨架（REQ-ARCH-000）；Phase 2：注册 import_files 命令（REQ-ING-001）。
//! 体系三铁律：生产代码严禁 unwrap()/expect()；启动错误显式上报并以非零码退出。

mod commands;
mod state;

use state::AppState;
use tauri::Manager;

fn main() {
    let builder = tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            app.manage(AppState::new(data_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::import_files]);

    if let Err(err) = builder.run(tauri::generate_context!()) {
        eprintln!("EchoMind 启动失败: {err}");
        std::process::exit(1);
    }
}
