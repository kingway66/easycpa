use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "easycpa")]
#[command(version = "0.1.0")]
#[command(about = "Standalone proxy server for Claude Code, Codex & Gemini CLI", long_about = None)]
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
    /// Add a provider to the database
    AddProvider {
        /// App type: claude, codex, gemini
        #[arg(long)]
        app_type: String,
        /// Provider name
        #[arg(long)]
        name: String,
        /// Base URL (e.g., https://api.example.com)
        #[arg(long)]
        base_url: String,
        /// API key / auth token
        #[arg(long)]
        api_key: String,
        /// Set as current provider
        #[arg(long, default_value = "false")]
        current: bool,
    },
    /// List configured providers
    ListProviders {
        /// Filter by app type
        #[arg(long)]
        app_type: Option<String>,
    },
    /// Remove a provider
    RemoveProvider {
        /// Provider ID
        #[arg(long)]
        id: String,
        /// App type
        #[arg(long)]
        app_type: String,
    },
    /// Import providers from legacy cc-switch database
    ImportProviders {
        /// Path to cc-switch database (default: ~/.cc-switch/cc-switch.db)
        #[arg(long)]
        from: Option<PathBuf>,
    },
    /// Show proxy status and config
    Status,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Serve { port: None, address: None }) {
        Commands::Serve { port, address } => cmd_serve(port, address).await,
        Commands::AddProvider { app_type, name, base_url, api_key, current } => {
            cmd_add_provider(&app_type, &name, &base_url, &api_key, current).await
        }
        Commands::ListProviders { app_type } => cmd_list_providers(app_type.as_deref()).await,
        Commands::RemoveProvider { id, app_type } => cmd_remove_provider(&id, &app_type).await,
        Commands::ImportProviders { from } => cmd_import_providers(from).await,
        Commands::Status => cmd_status().await,
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

async fn cmd_add_provider(
    app_type: &str,
    name: &str,
    base_url: &str,
    api_key: &str,
    set_current: bool,
) {
    let app_state = match easycpa_lib::init_app().await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("初始化失败: {e}");
            std::process::exit(1);
        }
    };

    let provider_id = uuid::Uuid::new_v4().to_string();

    // Build settings_config based on app_type
    let settings_config = match app_type {
        "claude" => serde_json::json!({
            "env": {
                "ANTHROPIC_BASE_URL": base_url,
                "ANTHROPIC_AUTH_TOKEN": api_key,
            }
        }),
        "codex" => serde_json::json!({
            "auth": {
                "OPENAI_API_KEY": api_key,
            }
        }),
        "gemini" => serde_json::json!({
            "env": {
                "GOOGLE_GEMINI_BASE_URL": base_url,
                "GEMINI_API_KEY": api_key,
            }
        }),
        _ => {
            eprintln!("不支持的 app_type: {app_type}，支持: claude, codex, gemini");
            std::process::exit(1);
        }
    };

    let provider = easycpa_lib::provider::Provider {
        id: provider_id.clone(),
        name: name.to_string(),
        settings_config,
        website_url: None,
        category: Some("custom".to_string()),
        created_at: Some(chrono::Utc::now().timestamp()),
        sort_index: None,
        notes: None,
        meta: None,
        icon: None,
        icon_color: None,
        in_failover_queue: false,
    };

    match app_state.db.save_provider(app_type, &provider) {
        Ok(_) => {
            println!("✓ Provider '{}' added with ID {}", name, provider_id);

            if set_current {
                if let Err(e) = app_state.db.set_current_provider(app_type, &provider_id) {
                    eprintln!("设置当前供应商失败: {e}");
                } else {
                    println!("✓ Set as current provider for {app_type}");
                }
            }
        }
        Err(e) => {
            eprintln!("添加供应商失败: {e}");
            std::process::exit(1);
        }
    }
}

async fn cmd_list_providers(app_type_filter: Option<&str>) {
    let app_state = match easycpa_lib::init_app().await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("初始化失败: {e}");
            std::process::exit(1);
        }
    };

    let app_types: Vec<&str> = match app_type_filter {
        Some(at) => vec![at],
        None => vec!["claude", "codex", "gemini"],
    };

    for at in app_types {
        let current_id = app_state.db.get_current_provider(at).ok().flatten();
        match app_state.db.get_all_providers(at) {
            Ok(providers) => {
                if providers.is_empty() {
                    println!("\n[{at}] (no providers)");
                    continue;
                }
                println!("\n[{at}]");
                for (id, provider) in providers {
                    let current = if current_id.as_deref() == Some(&id) { " (current)" } else { "" };
                    let failover = if provider.in_failover_queue { " [failover]" } else { "" };
                    println!("  {} | {}{}{}", id, provider.name, current, failover);
                }
            }
            Err(e) => {
                eprintln!("[{at}] 查询失败: {e}");
            }
        }
    }
}

async fn cmd_remove_provider(id: &str, app_type: &str) {
    let app_state = match easycpa_lib::init_app().await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("初始化失败: {e}");
            std::process::exit(1);
        }
    };

    match app_state.db.delete_provider(app_type, id) {
        Ok(_) => {
            println!("✓ Provider {id} removed from {app_type}");
        }
        Err(e) => {
            eprintln!("删除供应商失败: {e}");
            std::process::exit(1);
        }
    }
}

async fn cmd_import_providers(from: Option<PathBuf>) {
    let source_path = from.unwrap_or_else(|| {
        easycpa_lib::config::get_app_config_dir().join("cc-switch.db")
    });

    if !source_path.exists() {
        eprintln!("源数据库不存在: {}", source_path.display());
        std::process::exit(1);
    }

    println!("从 {} 导入供应商...", source_path.display());

    let source_db = match easycpa_lib::database::Database::init_at_path(&source_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("打开源数据库失败: {e}");
            std::process::exit(1);
        }
    };

    let mut models: Vec<serde_json::Value> = Vec::new();
    let mut total_imported = 0;

    if let Ok(providers) = source_db.get_all_providers("claude") {
        for (_id, provider) in &providers {
            let sc = &provider.settings_config;

            let base_url = sc.get("env")
                .and_then(|e| e.get("ANTHROPIC_BASE_URL"))
                .and_then(|v| v.as_str())
                .or_else(|| sc.get("base_url").and_then(|v| v.as_str()))
                .or_else(|| sc.get("baseURL").and_then(|v| v.as_str()))
                .unwrap_or("");

            if base_url.is_empty() {
                continue;
            }

            let api_key = sc.get("env")
                .and_then(|e| e.get("ANTHROPIC_AUTH_TOKEN").or_else(|| e.get("ANTHROPIC_API_KEY")))
                .and_then(|v| v.as_str())
                .or_else(|| sc.get("env").and_then(|e| e.get("OPENAI_API_KEY")).and_then(|v| v.as_str()))
                .unwrap_or("");

            let api_format = provider.meta.as_ref()
                .and_then(|m| m.api_format.as_deref())
                .or_else(|| sc.get("api_format").and_then(|v| v.as_str()))
                .unwrap_or("anthropic");

            models.push(serde_json::json!({
                "name": provider.name,
                "model": provider.name,
                "base_url": base_url,
                "api_key": api_key,
                "api_format": api_format,
            }));
            total_imported += 1;
        }
    }

    if models.is_empty() {
        eprintln!("未找到任何供应商");
        std::process::exit(1);
    }

    let listen = if let Ok(gc) = source_db.get_global_proxy_config().await {
        format!("{}:{}", gc.listen_address, gc.listen_port)
    } else {
        "127.0.0.1:15721".to_string()
    };

    let output = serde_json::json!({
        "models": models,
        "listen": listen,
    });

    let output_path = easycpa_lib::config::get_config_json_path();
    if let Some(parent) = output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let json_str = serde_json::to_string_pretty(&output).unwrap();
    match std::fs::write(&output_path, &json_str) {
        Ok(_) => {
            println!("\n✓ 已写入 {}", output_path.display());
            println!("  共 {total_imported} 条模型路由");
        }
        Err(e) => {
            eprintln!("写入配置文件失败: {e}");
            std::process::exit(1);
        }
    }
}

async fn cmd_status() {
    let app_state = match easycpa_lib::init_app().await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("初始化失败: {e}");
            std::process::exit(1);
        }
    };

    // Show proxy config
    match app_state.db.get_global_proxy_config().await {
        Ok(config) => {
            println!("Proxy Config:");
            println!("  Listen: {}:{}", config.listen_address, config.listen_port);
            println!("  Enabled: {}", config.proxy_enabled);
            println!("  Logging: {}", config.enable_logging);
        }
        Err(e) => {
            eprintln!("读取代理配置失败: {e}");
        }
    }

    // Show model routes
    let routes = easycpa_lib::get_model_routes();
    if !routes.is_empty() {
        println!("\nModel Routes ({}):", routes.len());
        for r in routes {
            println!("  [{}] → {} ({})", r.name, r.model, r.api_format);
        }
    }

    // Show per-app config
    for app_type in ["claude", "codex", "gemini"] {
        match app_state.db.get_proxy_config_for_app(app_type).await {
            Ok(config) => {
                println!("\n[{app_type}]");
                println!("  Enabled: {}", config.enabled);
                println!("  Failover: {}", config.auto_failover_enabled);
                println!("  Max retries: {}", config.max_retries);
            }
            Err(e) => {
                eprintln!("[{app_type}] 读取配置失败: {e}");
            }
        }
    }

    // Show provider counts
    for app_type in ["claude", "codex", "gemini"] {
        let current_id = app_state.db.get_current_provider(app_type).ok().flatten();
        match app_state.db.get_all_providers(app_type) {
            Ok(providers) => {
                let count = providers.len();
                let current_name = current_id
                    .as_ref()
                    .and_then(|id| providers.get(id))
                    .map(|p| p.name.as_str())
                    .unwrap_or("(none)");
                println!("\n[{app_type}] {count} provider(s), current: {current_name}");
            }
            Err(e) => {
                eprintln!("[{app_type}] 查询供应商失败: {e}");
            }
        }
    }
}
