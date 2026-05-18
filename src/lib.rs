//! EasyCPA — CPA-compatible proxy for Claude Code & Codex

pub mod config;
pub mod database;
pub mod error;
pub mod provider;
pub mod provider_defaults;
pub mod proxy;
pub mod services;
pub mod store;

use error::AppError;
use proxy::types::{AppProxyConfig, GlobalProxyConfig};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::SystemTime;
use store::AppState;

// === 模型路由配置 ===

fn default_listen() -> String {
    "127.0.0.1:15721".to_string()
}

fn default_api_format() -> String {
    "openai_chat".to_string()
}

fn default_false() -> bool {
    false
}

/// 模型路由条目
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelRoute {
    #[serde(default)]
    pub name: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default = "default_api_format")]
    pub api_format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reasoning_level: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_reasoning_levels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_reasoning_summaries: Option<bool>,
    #[serde(default = "default_false")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
struct SimpleConfig {
    #[serde(default)]
    models: Vec<ModelRoute>,
    #[serde(default = "default_listen")]
    listen: String,
}

/// 全局模型路由表（启动时加载，支持热重载）
static MODEL_ROUTES: OnceLock<RwLock<(Vec<ModelRoute>, SystemTime)>> = OnceLock::new();

/// 检查 config.json mtime 并在需要时重载
fn check_and_reload_routes() {
    let Some(guard) = MODEL_ROUTES.get() else { return };
    let config_path = config::get_proxy_dir().join("config.json");
    let Ok(meta) = std::fs::metadata(&config_path) else { return };
    let Ok(mtime) = meta.modified() else { return };

    {
        let r = guard.read().unwrap();
        if mtime <= r.1 {
            return;
        }
    }

    let mut w = guard.write().unwrap();
    if mtime <= w.1 {
        return;
    }
    match reload_routes_from_file(&config_path) {
        Ok(new_routes) => {
            log::info!("[Config] 已重载模型路由: {} 条规则", new_routes.len());
            *w = (new_routes, mtime);
        }
        Err(e) => {
            log::warn!("[Config] 重载模型路由失败: {e}");
        }
    }
}

fn reload_routes_from_file(path: &Path) -> Result<Vec<ModelRoute>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("读取配置文件失败: {e}"))?;
    let mut config: SimpleConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置文件失败: {e}"))?;
    for r in &mut config.models {
        if r.name.is_empty() {
            r.name = r.model.clone();
        }
    }
    Ok(config.models)
}

pub fn find_model_route(name: &str) -> Option<ModelRoute> {
    check_and_reload_routes();
    let guard = MODEL_ROUTES.get()?.read().unwrap();
    if let Some(r) = guard.0.iter().find(|r| r.name == name && r.enabled) {
        return Some(r.clone());
    }
    guard.0.iter().find(|r| r.name == "*" && r.enabled).cloned()
}

pub fn get_model_routes() -> Vec<ModelRoute> {
    check_and_reload_routes();
    MODEL_ROUTES.get()
        .map(|rw| rw.read().unwrap().0.clone())
        .unwrap_or_default()
}

/// Initialize the app — load from config.json only
pub async fn init_app() -> Result<AppState, AppError> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let app_config_dir = config::get_proxy_dir();
    init_logging(&app_config_dir);

    log::info!("EasyCPA starting...");
    log::info!("Config dir: {}", app_config_dir.display());

    if let Err(e) = proxy::http_client::init(None) {
        log::warn!("HTTP 客户端初始化失败: {e}");
    }

    proxy::model_mapper::init_model_mapping_cache();

    let config_path = config::get_active_config_path()
        .ok_or_else(|| AppError::Config("未找到配置文件 ~/.easycpa/config.json".to_string()))?;

    log::info!("检测到配置文件，从 {} 加载", config_path.display());
    init_app_from_json(&config_path).await
}

async fn init_app_from_json(json_path: &Path) -> Result<AppState, AppError> {
    let content = std::fs::read_to_string(json_path).map_err(|e| {
        AppError::Config(format!("读取配置文件失败: {e}"))
    })?;

    let mut config: SimpleConfig = serde_json::from_str(&content).map_err(|e| {
        AppError::Config(format!("解析配置文件失败: {e}"))
    })?;

    for r in &mut config.models {
        if r.name.is_empty() {
            r.name = r.model.clone();
        }
    }

    let (listen_addr, listen_port) = parse_listen(&config.listen);

    log::info!("已加载 {} 条模型路由", config.models.len());
    for r in &config.models {
        log::info!("  [{}] {} → {} ({})", r.name, r.model, r.base_url, r.api_format);
    }

    MODEL_ROUTES.set(RwLock::new((config.models, SystemTime::now()))).map_err(|_| {
        AppError::Config("MODEL_ROUTES 已初始化".to_string())
    })?;

    let db = Arc::new(database::Database::memory().map_err(|e| {
        AppError::Message(format!("创建内存数据库失败: {e}"))
    })?);

    let global_config = GlobalProxyConfig {
        proxy_enabled: true,
        listen_address: listen_addr,
        listen_port,
        enable_logging: true,
    };
    let _ = db.update_global_proxy_config(global_config).await;

    for app_type in ["claude", "codex"] {
        let app_config = AppProxyConfig {
            app_type: app_type.to_string(),
            enabled: true,
            auto_failover_enabled: false,
            max_retries: 6,
            streaming_first_byte_timeout: 60,
            streaming_idle_timeout: 120,
            non_streaming_timeout: 0,
            circuit_failure_threshold: 5,
            circuit_success_threshold: 3,
            circuit_timeout_seconds: 60,
            circuit_error_rate_threshold: 0.5,
            circuit_min_requests: 10,
        };
        let _ = db.update_proxy_config_for_app(app_config).await;
    }

    let app_state = AppState::new(db);
    Ok(app_state)
}

fn parse_listen(listen: &str) -> (String, u16) {
    if let Some((addr, port_str)) = listen.rsplit_once(':') {
        if let Ok(port) = port_str.parse::<u16>() {
            return (addr.to_string(), port);
        }
    }
    ("127.0.0.1".to_string(), 15721)
}

fn init_logging(app_config_dir: &std::path::Path) {
    use std::io::Write;

    let log_dir = app_config_dir.join("logs");
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("创建日志目录失败: {e}");
    }

    let log_file_path = log_dir.join("easycpa.log");

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .unwrap_or_else(|e| {
            eprintln!("无法打开日志文件 {}: {e}", log_file_path.display());
            std::process::exit(1);
        });

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    log::info!("日志初始化完成，文件: {}", log_file_path.display());
}