use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "easycpa")]
#[command(version)]
#[command(about = "CPA-compatible proxy for Claude Code & Codex", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the proxy server (default, daemonizes unless --foreground)
    Start {
        /// Override listen port
        #[arg(long)]
        port: Option<u16>,
        /// Override listen address
        #[arg(long)]
        address: Option<String>,
        /// Run in foreground (no daemonize)
        #[arg(long)]
        foreground: bool,
        /// Don't open browser after start
        #[arg(long)]
        no_open: bool,
    },
    /// Stop the running daemon
    Stop,
    /// Restart the daemon
    Restart {
        /// Override listen port
        #[arg(long)]
        port: Option<u16>,
        /// Override listen address
        #[arg(long)]
        address: Option<String>,
    },
    /// Reload config (send SIGHUP to daemon)
    Reload,
    /// Check if daemon is running
    Status,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Start {
        port: None,
        address: None,
        foreground: false,
        no_open: false,
    }) {
        Commands::Start {
            port,
            address,
            foreground,
            no_open,
        } => {
            if foreground {
                cmd_serve(port, address, !no_open).await;
            } else {
                cmd_start_daemon(port, address, no_open);
            }
        }
        Commands::Stop => cmd_stop(),
        Commands::Restart { port, address } => cmd_restart(port, address),
        Commands::Reload => cmd_reload(),
        Commands::Status => cmd_status(),
    }
}

fn cmd_start_daemon(port: Option<u16>, address: Option<String>, no_open: bool) {
    if let Err(e) = easycpa_lib::daemon::daemonize(port, address, no_open) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn cmd_stop() {
    if let Err(e) = easycpa_lib::daemon::stop_daemon() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn cmd_restart(port: Option<u16>, address: Option<String>) {
    // Stop if running (ignore "not running" error)
    let _ = easycpa_lib::daemon::stop_daemon();
    // Wait briefly for cleanup
    std::thread::sleep(std::time::Duration::from_millis(300));
    // Start daemon (always use --no-open on restart)
    if let Err(e) = easycpa_lib::daemon::daemonize(port, address, true) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn cmd_reload() {
    if let Err(e) = easycpa_lib::daemon::reload_daemon() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn cmd_status() {
    easycpa_lib::daemon::print_status();
}

async fn cmd_serve(
    override_port: Option<u16>,
    override_address: Option<String>,
    open_browser: bool,
) {
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

    // Write PID file for daemon mode
    if let Err(e) = easycpa_lib::daemon::write_pid() {
        log::warn!("Failed to write PID file: {e}");
    }

    // Start the proxy
    let info = match app_state.proxy_service.start().await {
        Ok(info) => {
            log::info!("Proxy started on {}:{}", info.address, info.port);
            info
        }
        Err(e) => {
            log::error!("Failed to start proxy: {e}");
            eprintln!("代理启动失败: {e}");
            easycpa_lib::daemon::remove_pid();
            std::process::exit(1);
        }
    };

    // Open browser
    if open_browser {
        let url = format!("http://{}:{}", info.address, info.port);
        easycpa_lib::daemon::open_browser(&url);
    }

    // Wait for shutdown signal (Ctrl+C) or SIGHUP (reload)
    let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()).unwrap();
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log::info!("收到退出信号，正在关闭...");
                break;
            }
            _ = sighup.recv() => {
                log::info!("收到 SIGHUP，重新加载配置...");
                if let Err(e) = easycpa_lib::reload_model_routes_now() {
                    log::warn!("SIGHUP 重载模型路由失败: {e}");
                }
                // Re-read proxy config from DB (which reflects config.json)
                if let Ok(proxy_config) = app_state.db.get_proxy_config().await {
                    if let Err(e) = app_state.proxy_service.update_config(&proxy_config).await {
                        log::warn!("SIGHUP reload config failed: {e}");
                    } else {
                        app_state.proxy_service.reload_runtime_config(&proxy_config).await;
                        log::info!("配置已重新加载");
                    }
                }
                // Keep running
                continue;
            }
        }
    }

    // Stop proxy gracefully
    if let Err(e) = app_state.proxy_service.stop().await {
        log::warn!("停止代理服务器时出错: {e}");
    }

    easycpa_lib::daemon::remove_pid();
    log::info!("EasyCPA 已退出");
}
