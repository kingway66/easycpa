use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "easycpa")]
#[command(version = "0.1.0")]
#[command(about = "CPA-compatible proxy for Claude Code & Codex", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the proxy server (default)
    Serve {
        /// Override listen port
        #[arg(long)]
        port: Option<u16>,
        /// Override listen address
        #[arg(long)]
        address: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Serve { port: None, address: None }) {
        Commands::Serve { port, address } => cmd_serve(port, address).await,
    }
}

async fn cmd_serve(override_port: Option<u16>, override_address: Option<String>) {
    let app_state = match easycpa_lib::init_app().await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("初始化失败: {e}");
            std::process::exit(1);
        }
    };

    // Apply port/address overrides if specified
    if override_port.is_some() || override_address.is_some() {
        if let Ok(mut config) = app_state.db.get_global_proxy_config().await {
            if let Some(p) = override_port {
                config.listen_port = p;
            }
            if let Some(a) = override_address {
                config.listen_address = a;
            }
            if let Err(e) = app_state.db.update_global_proxy_config(config).await {
                log::warn!("Failed to apply port/address override: {e}");
            }
        }
    }

    // Start the proxy
    match app_state.proxy_service.start().await {
        Ok(info) => {
            log::info!("Proxy started on {}:{}", info.address, info.port);
        }
        Err(e) => {
            log::error!("Failed to start proxy: {e}");
            eprintln!("代理启动失败: {e}");
            std::process::exit(1);
        }
    }

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await.expect("failed to listen for ctrl+c");
    log::info!("收到退出信号，正在关闭...");

    // Stop proxy gracefully
    if let Err(e) = app_state.proxy_service.stop().await {
        log::warn!("停止代理服务器时出错: {e}");
    }

    log::info!("EasyCPA 已退出");
}