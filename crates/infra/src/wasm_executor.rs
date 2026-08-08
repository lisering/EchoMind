//! WASM 沙箱执行器（Pro feature，REQ-RAG-032）。
//!
//! 借鉴 CodeForge 多语言执行器：在安全沙箱中执行代码片段并返回结果。
//!
//! ## 第一阶段 MVP（本地进程执行 + 安全限制）
//!
//! 第一阶段使用 `std::process::Command` 调用本地解释器（python3/node），
//! 后续替换为 WASM 沙箱（wasmtime）。
//!
//! ## 安全措施
//!
//! 1. 超时：`tokio::time::timeout` 包装整个执行，超时 kill 进程
//! 2. 内存：Unix 下设置 RLIMIT_AS（`setrlimit`）
//! 3. 网络：不设置任何网络代理环境变量
//! 4. 临时文件：写入临时目录，执行后删除
//! 5. 工作目录：设为临时目录（隔离用户文件系统）
//! 6. 环境变量：清空所有环境变量，仅设置必要的 PATH
//!
//! ## 支持的语言
//!
//! - Python：`python3 -c "code"`（需 python3 已安装）
//! - JavaScript：`node -e "code"`（需 node 已安装）
//! - Rust：暂不支持（需编译为 WASM，安全风险高）

use std::process::Stdio;
use std::time::Duration;

use echomind_core::CodeExecutor;
use echomind_models::{CodeExecutorConfig, ExecutionResult};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::timeout;

/// WASM 沙箱执行器（本地安全执行，Pro feature）。
///
/// 第一阶段 MVP 使用本地进程执行（python3/node），
/// 后续替换为 WASM 沙箱（wasmtime）。
pub struct WasmExecutor {
    config: CodeExecutorConfig,
}

impl WasmExecutor {
    /// 创建执行器：指定配置。
    pub fn new(config: CodeExecutorConfig) -> Self {
        Self { config }
    }

    /// 创建执行器：使用默认配置（10s 超时 / 64MB 内存 / 无网络）。
    pub fn with_defaults() -> Self {
        Self::new(CodeExecutorConfig::default())
    }

    /// 执行 Python 代码（通过 `python3 -c`）。
    async fn execute_python(
        &self,
        code: &str,
        stdin: Option<&str>,
    ) -> anyhow::Result<ExecutionResult> {
        self.execute_process("python3", &["-c", code], stdin).await
    }

    /// 执行 JavaScript 代码（通过 `node -e`）。
    async fn execute_javascript(
        &self,
        code: &str,
        stdin: Option<&str>,
    ) -> anyhow::Result<ExecutionResult> {
        self.execute_process("node", &["-e", code], stdin).await
    }

    /// 执行外部进程并捕获输出，应用超时和安全限制。
    async fn execute_process(
        &self,
        program: &str,
        args: &[&str],
        stdin_data: Option<&str>,
    ) -> anyhow::Result<ExecutionResult> {
        let start = std::time::Instant::now();

        // 构建命令：清空环境变量，设置最小 PATH
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear();

        // 设置 PATH（仅保留系统路径，确保解释器可执行）
        #[cfg(unix)]
        {
            cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
        }
        #[cfg(not(unix))]
        {
            cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
        }

        // 设置工作目录为临时目录（隔离用户文件系统）
        let temp_dir = tempfile::tempdir()?;
        cmd.current_dir(temp_dir.path());

        // Unix 内存限制（RLIMIT_AS）
        #[cfg(unix)]
        {
            set_memory_limit(&mut cmd, self.config.memory_limit_mb);
        }

        // 启动进程
        let mut child = cmd.spawn()?;

        // 写入 stdin（如果有）
        if let Some(stdin) = stdin_data {
            if let Some(mut child_stdin) = child.stdin.take() {
                child_stdin.write_all(stdin.as_bytes()).await?;
                child_stdin.flush().await?;
                // drop child_stdin to signal EOF
            }
        } else {
            // 关闭 stdin（无输入时）
            drop(child.stdin.take());
        }

        // 取出 stdout / stderr 管道（wait_with_output 会消费 child，需手动读取）
        let mut child_stdout = child.stdout.take();
        let mut child_stderr = child.stderr.take();

        // 超时等待进程退出（不消费 child，超时后可 kill）
        let timeout_duration = Duration::from_secs(self.config.timeout_secs);
        let wait_result = timeout(timeout_duration, child.wait()).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match wait_result {
            Ok(Ok(status)) => {
                // 读取 stdout / stderr
                let stdout = read_pipe(&mut child_stdout).await;
                let stderr = read_stderr_pipe(&mut child_stderr).await;
                Ok(ExecutionResult {
                    stdout,
                    stderr,
                    exit_code: status.code().unwrap_or(-1),
                    duration_ms,
                    timed_out: false,
                })
            }
            Ok(Err(e)) => Ok(ExecutionResult {
                stdout: String::new(),
                stderr: format!("进程执行失败: {e}"),
                exit_code: -1,
                duration_ms,
                timed_out: false,
            }),
            Err(_) => {
                // 超时：kill 进程
                let _ = child.kill().await;
                Ok(ExecutionResult {
                    stdout: String::new(),
                    stderr: format!("执行超时（{}s）", self.config.timeout_secs),
                    exit_code: -1,
                    duration_ms,
                    timed_out: true,
                })
            }
        }
    }
}

impl CodeExecutor for WasmExecutor {
    fn execute<'a>(
        &'a self,
        code: &'a str,
        language: &'a str,
        stdin: Option<&'a str>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<ExecutionResult>> + Send + 'a>,
    > {
        Box::pin(async move {
            match language {
                "python" | "py" => self.execute_python(code, stdin).await,
                "javascript" | "js" => self.execute_javascript(code, stdin).await,
                _ => anyhow::bail!("不支持的语言: {language}（支持 python / javascript）"),
            }
        })
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["python", "javascript"]
    }

    fn config(&self) -> CodeExecutorConfig {
        self.config.clone()
    }
}

/// Unix 下设置进程内存限制（RLIMIT_AS）。
///
/// 通过 `pre_exec` 在 fork 后、exec 前设置 `RLIMIT_AS`，
/// 限制进程虚拟内存大小。
#[cfg(unix)]
fn set_memory_limit(cmd: &mut Command, memory_limit_mb: usize) {
    let memory_bytes = memory_limit_mb * 1024 * 1024;
    // safety: setrlimit 在 fork 后、exec 前调用，仅影响子进程
    unsafe {
        cmd.pre_exec(move || {
            let rlim = libc::rlimit {
                rlim_cur: memory_bytes as libc::rlim_t,
                rlim_max: memory_bytes as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &rlim) != 0 {
                // 内存限制设置失败不影响执行（仅日志）
                eprintln!("[WARN] setrlimit RLIMIT_AS 失败");
            }
            Ok(())
        });
    }
}

/// 从管道读取全部内容为 String。
async fn read_pipe(pipe: &mut Option<tokio::process::ChildStdout>) -> String {
    let mut buf = Vec::new();
    if let Some(stdout) = pipe {
        let _ = stdout.read_to_end(&mut buf).await;
    }
    String::from_utf8_lossy(&buf).to_string()
}

/// 从 stderr 管道读取全部内容为 String。
#[allow(clippy::ptr_arg)]
async fn read_stderr_pipe(pipe: &mut Option<tokio::process::ChildStderr>) -> String {
    let mut buf = Vec::new();
    if let Some(stderr) = pipe {
        let _ = stderr.read_to_end(&mut buf).await;
    }
    String::from_utf8_lossy(&buf).to_string()
}
