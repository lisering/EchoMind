//! EchoMind（灵犀）应用入口 —— Phase 1 启动骨架（REQ-ARCH-000）。
//! 体系三铁律：生产代码严禁 unwrap()/expect()；启动错误显式上报并以非零码退出。

fn main() {
    if let Err(err) = tauri::Builder::default().run(tauri::generate_context!()) {
        eprintln!("EchoMind 启动失败: {err}");
        std::process::exit(1);
    }
}
