//! License 签发 CLI 工具入口（REQ-LIC-005）。
//!
//! 用法：
//! ```bash
//! # 生成密钥对（公钥 + 私钥写入指定目录，私钥权限 0600）
//! license-issuer genkey --output /path/to/keys
//!
//! # 用私钥签发 License Key
//! license-issuer issue --payload "EchoMind-Pro-v1" --key /path/to/keys/license_privkey.b64
//! ```
//!
//! 输出的 License Key 格式：`base64(payload)-base64(signature)`
//! 与客户端 `crates/core/src/license.rs::verify_license` 兼容。

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use license_issuer::{generate_keypair, issue_license, load_privkey, save_keypair_to_dir};

/// License 签发 CLI 工具。
#[derive(Parser)]
#[command(
    name = "license-issuer",
    about = "EchoMind License 签发 CLI 工具（REQ-LIC-005）"
)]
struct Cli {
    /// 子命令
    #[command(subcommand)]
    command: Commands,
}

/// 子命令定义。
#[derive(Subcommand)]
enum Commands {
    /// 生成 Ed25519 密钥对
    Genkey {
        /// 输出目录（公钥 + 私钥文件写入此目录）
        #[arg(long, default_value = ".")]
        output: PathBuf,
    },
    /// 用私钥签发 License Key
    Issue {
        /// 签名的 payload 文本（如 `EchoMind-Pro-v1`）
        #[arg(long)]
        payload: String,
        /// 私钥文件路径（base64 编码的 32 字节 seed）
        #[arg(long)]
        key: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Genkey { output } => {
            let (pubkey, privkey) = generate_keypair();
            let (pubkey_path, privkey_path) =
                save_keypair_to_dir(&pubkey, &privkey, &output).context("保存密钥对失败")?;
            println!("密钥对已生成：");
            println!("  公钥文件: {}", pubkey_path.display());
            println!("  私钥文件: {} (权限 0600)", privkey_path.display());
            println!();
            println!("下一步：");
            println!("  1. 将公钥 base64 替换 crates/core/src/license.rs 中的 BUILTIN_PUBKEY 常量");
            println!(
                "  2. 用私钥签发 License Key：license-issuer issue --payload \"EchoMind-Pro-v1\" --key {}",
                privkey_path.display()
            );
        }
        Commands::Issue { payload, key } => {
            let privkey_seed =
                load_privkey(&key).with_context(|| format!("加载私钥失败: {}", key.display()))?;
            let license_key =
                issue_license(&payload, &privkey_seed).context("签发 License Key 失败")?;
            println!("{license_key}");
        }
    }
    Ok(())
}
