//! EasyCPA — CPA-compatible proxy for Claude Code, Codex & Gemini CLI
//!
//! Extracted from cc-switch (https://github.com/farion1231/cc-switch).
//! All proxy logic, protocol conversion, circuit breaker, and failover
//! are preserved identically to the upstream project.

pub mod app_config;
pub mod claude_desktop_config;
pub mod claude_mcp;
pub mod claude_plugin;
pub mod codex_config;
pub mod config;
pub mod database;
pub mod error;
pub mod gemini_config;
pub mod gemini_mcp;
pub mod hermes_config;
pub mod mcp;
pub mod openclaw_config;
pub mod opencode_config;
pub mod panic_hook;
pub mod prompt;
pub mod prompt_files;
pub mod provider;
pub mod provider_defaults;
pub mod proxy;
pub mod services;
pub mod settings;
pub mod store;
pub mod usage_script;

use app_config::AppType;
use error::AppError;
use provider::Provider;
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
    "anthropic".to_string()
}

/// 模型路由条目
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelRoute {
    /// 路由名称（唯一标识，客户端请求中的 model 字段匹配此项）
    /// 默认等于 model 字段
    #[serde(default)]
    pub name: String,
    /// 实际转发给上游的模型名
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default = "default_api_format")]
    pub api_format: String,
    // 模型能力字段（可选，用于 /v1/models 端点返回 Codex ModelInfo）
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
}

/// 新格式：按模型路由
#[derive(Debug, Deserialize)]
struct SimpleConfig {
    #[serde(default)]
    models: Vec<ModelRoute>,
    #[serde(default = "default_listen")]
    listen: String,
}

/// 旧的 providers 数组格式（向后兼容）
#[derive(Debug, Deserialize)]
struct LegacySimpleProvider {
    id: String,
    name: String,
    base_url: String,
    api_key: String,
    #[serde(default = "default_api_format")]
    api_format: String,
    #[serde(default)]
    current: bool,
}

#[derive(Debug, Deserialize)]
struct LegacySimpleConfig {
    #[serde(default)]
    providers: Vec<LegacySimpleProvider>,
    #[serde(default = "default_listen")]
    listen: String,
}

/// 全局模型路由表（启动时加载，支持热重载）
/// RwLock 内存 (routes, last_mtime)
static MODEL_ROUTES: OnceLock<RwLock<(Vec<ModelRoute>, SystemTime)>> = OnceLock::new();

/// 检查 config.json mtime 并在需要时重载
fn check_and_reload_routes() {
    let Some(guard) = MODEL_ROUTES.get() else { return };
    let config_path = config::get_proxy_dir().join("config.json");
    let Ok(meta) = std::fs::metadata(&config_path) else { return };
    let Ok(mtime) = meta.modified() else { return };

    // 快速检查：读锁内比较 mtime
    {
        let r = guard.read().unwrap();
        if mtime <= r.1 {
            return; // 未变更
        }
    }

    // mtime 变了，获取写锁重载
    let mut w = guard.write().unwrap();
    // 双重检查
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

/// 从 config 文件解析模型路由
fn reload_routes_from_file(path: &Path) -> Result<Vec<ModelRoute>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("读取配置文件失败: {e}"))?;
    let raw: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置文件失败: {e}"))?;

    let has_models = raw.get("models").and_then(|m| m.as_array()).is_some();
    let has_providers_array = raw.get("providers").and_then(|p| p.as_array()).is_some();

    if has_models {
        let mut config: SimpleConfig = serde_json::from_value(raw)
            .map_err(|e| format!("解析配置失败: {e}"))?;
        for r in &mut config.models {
            if r.name.is_empty() {
                r.name = r.model.clone();
            }
        }
        Ok(config.models)
    } else if has_providers_array {
        let config: LegacySimpleConfig = serde_json::from_value(raw)
            .map_err(|e| format!("解析配置失败: {e}"))?;
        Ok(config.providers.iter().map(|p| {
            let model = if p.current { "*".to_string() } else { p.id.clone() };
            ModelRoute {
                name: model.clone(),
                model,
                base_url: p.base_url.clone(),
                api_key: p.api_key.clone(),
                api_format: p.api_format.clone(),
                context_window: None,
                max_output_tokens: None,
                default_reasoning_level: None,
                supported_reasoning_levels: Vec::new(),
                supports_parallel_tool_calls: None,
                supports_reasoning_summaries: None,
            }
        }).collect())
    } else {
        Err("不支持的配置格式".to_string())
    }
}

/// 根据路由名称查找路由（精确匹配优先，`*` 通配符兜底）
/// 每次调用检查 config.json mtime，变更时自动重载
pub fn find_model_route(name: &str) -> Option<ModelRoute> {
    check_and_reload_routes();
    let guard = MODEL_ROUTES.get()?.read().unwrap();
    if let Some(r) = guard.0.iter().find(|r| r.name == name) {
        return Some(r.clone());
    }
    guard.0.iter().find(|r| r.name == "*").cloned()
}

/// 获取所有模型路由（供 status 命令使用）
/// 每次调用检查 config.json mtime，变更时自动重载
pub fn get_model_routes() -> Vec<ModelRoute> {
    check_and_reload_routes();
    MODEL_ROUTES.get()
        .map(|rw| rw.read().unwrap().0.clone())
        .unwrap_or_default()
}

/// 旧格式兼容（providers.json）
#[derive(Debug, Deserialize)]
struct LegacyProviderGroup {
    current: Option<String>,
    list: Vec<Provider>,
}

#[derive(Debug, Deserialize)]
struct LegacyConfig {
    providers: std::collections::HashMap<String, LegacyProviderGroup>,
    #[serde(default)]
    proxy_config: LegacyGlobalConfig,
}

#[derive(Debug, Deserialize, Default)]
struct LegacyGlobalConfig {
    #[serde(default = "default_listen_addr")]
    listen_address: String,
    #[serde(default = "default_listen_port")]
    listen_port: u16,
    #[serde(default = "default_true")]
    proxy_enabled: bool,
    #[serde(default = "default_true")]
    enable_logging: bool,
    #[serde(default)]
    apps: std::collections::HashMap<String, LegacyAppConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct LegacyAppConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    auto_failover_enabled: bool,
    #[serde(default = "default_max_retries")]
    max_retries: u32,
}

fn default_listen_addr() -> String { "127.0.0.1".to_string() }
fn default_listen_port() -> u16 { 15721 }
fn default_true() -> bool { true }
fn default_max_retries() -> u32 { 6 }

/// Initialize the app — load from JSON config if available, else use SQLite.
pub async fn init_app() -> Result<AppState, AppError> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let app_config_dir = config::get_proxy_dir();
    panic_hook::init_app_config_dir(app_config_dir.clone());
    init_logging(&app_config_dir);

    log::info!("EasyCPA starting...");
    log::info!("Config dir: {}", app_config_dir.display());

    // 初始化全局 HTTP 客户端（直连，不走上游代理）
    if let Err(e) = proxy::http_client::init(None) {
        log::warn!("HTTP 客户端初始化失败: {e}");
    }

    // 初始化模型映射缓存（支持热重载）
    proxy::model_mapper::init_model_mapping_cache();

    if let Some(config_path) = config::get_active_config_path() {
        log::info!("检测到配置文件，从 {} 加载", config_path.display());
        settings::set_use_json_config(true);
        init_app_from_json(&config_path).await
    } else {
        log::info!("未找到配置文件，使用 SQLite 数据库");
        init_app_from_db(&app_config_dir).await
    }
}

/// 从 JSON 配置文件加载到内存数据库（支持三种格式）
async fn init_app_from_json(json_path: &Path) -> Result<AppState, AppError> {
    let content = std::fs::read_to_string(json_path).map_err(|e| {
        AppError::Config(format!("读取配置文件失败: {e}"))
    })?;

    let raw: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        AppError::Config(format!("解析配置文件失败: {e}"))
    })?;

    let db = Arc::new(database::Database::memory().map_err(|e| {
        AppError::Message(format!("创建内存数据库失败: {e}"))
    })?);

    // 检测格式
    let has_models = raw.get("models").and_then(|m| m.as_array()).is_some();
    let has_providers_array = raw.get("providers").and_then(|p| p.as_array()).is_some();
    let has_providers_object = raw.get("providers").and_then(|p| p.as_object()).is_some();

    if has_models {
        // === 新格式：按模型路由 ===
        let mut config: SimpleConfig = serde_json::from_value(raw).map_err(|e| {
            AppError::Config(format!("解析配置失败: {e}"))
        })?;

        // name 默认等于 model
        for r in &mut config.models {
            if r.name.is_empty() {
                r.name = r.model.clone();
            }
        }

        let (listen_addr, listen_port) = parse_listen(&config.listen);

        log::info!("✓ 已加载 {} 条模型路由", config.models.len());
        for r in &config.models {
            log::info!("  [{}] {} → {} ({})", r.name, r.model, r.base_url, r.api_format);
        }

        MODEL_ROUTES.set(RwLock::new((config.models, SystemTime::now()))).map_err(|_| {
            AppError::Config("MODEL_ROUTES 已初始化".to_string())
        })?;
        let global_config = GlobalProxyConfig {
            proxy_enabled: true,
            listen_address: listen_addr,
            listen_port,
            enable_logging: true,
        };
        let _ = db.update_global_proxy_config(global_config).await;

        for app_type in ["claude", "codex", "gemini"] {
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
    } else if has_providers_array {
        // === 旧的 providers 数组格式（向后兼容，转为模型路由） ===
        let config: LegacySimpleConfig = serde_json::from_value(raw).map_err(|e| {
            AppError::Config(format!("解析配置失败: {e}"))
        })?;

        let (listen_addr, listen_port) = parse_listen(&config.listen);

        let routes: Vec<ModelRoute> = config.providers.iter().map(|p| {
            let model = if p.current { "*".to_string() } else { p.id.clone() };
            ModelRoute {
                name: model.clone(),
                model,
                base_url: p.base_url.clone(),
                api_key: p.api_key.clone(),
                api_format: p.api_format.clone(),
                context_window: None,
                max_output_tokens: None,
                default_reasoning_level: None,
                supported_reasoning_levels: Vec::new(),
                supports_parallel_tool_calls: None,
                supports_reasoning_summaries: None,
            }
        }).collect();

        log::info!("✓ 已加载 {} 条模型路由（从 providers 转换）", routes.len());
        MODEL_ROUTES.set(RwLock::new((routes, SystemTime::now()))).map_err(|_| {
            AppError::Config("MODEL_ROUTES 已初始化".to_string())
        })?;

        let global_config = GlobalProxyConfig {
            proxy_enabled: true,
            listen_address: listen_addr,
            listen_port,
            enable_logging: true,
        };
        let _ = db.update_global_proxy_config(global_config).await;

        for app_type in ["claude", "codex", "gemini"] {
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
    } else if has_providers_object {
        // === 旧格式兼容（cc-switch 的 providers.json） ===
        let config: LegacyConfig = serde_json::from_value(raw).map_err(|e| {
            AppError::Config(format!("解析配置失败: {e}"))
        })?;

        let mut total_providers = 0;
        for (app_type, group) in &config.providers {
            for provider in &group.list {
                if let Err(e) = db.save_provider(app_type, provider) {
                    log::warn!("导入供应商 {} 失败: {e}", provider.name);
                } else {
                    total_providers += 1;
                }
            }
            if let Some(ref current_id) = group.current {
                let _ = db.set_current_provider(app_type, current_id);
            }
        }
        log::info!("✓ 已加载 {total_providers} 个供应商（旧格式）");

        let global_config = GlobalProxyConfig {
            proxy_enabled: config.proxy_config.proxy_enabled,
            listen_address: config.proxy_config.listen_address.clone(),
            listen_port: config.proxy_config.listen_port,
            enable_logging: config.proxy_config.enable_logging,
        };
        let _ = db.update_global_proxy_config(global_config).await;

        for app_type in ["claude", "codex", "gemini"] {
            let app_cfg = config.proxy_config.apps.get(app_type);
            let app_config = AppProxyConfig {
                app_type: app_type.to_string(),
                enabled: app_cfg.map(|c| c.enabled).unwrap_or(false),
                auto_failover_enabled: app_cfg.map(|c| c.auto_failover_enabled).unwrap_or(false),
                max_retries: app_cfg.map(|c| c.max_retries).unwrap_or(6),
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
    } else {
        return Err(AppError::Config("无法识别的配置文件格式".to_string()));
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

/// 从 SQLite 数据库初始化（原有逻辑）
async fn init_app_from_db(app_config_dir: &Path) -> Result<AppState, AppError> {
    let db_path = app_config_dir.join("easycpa.db");
    let old_json_path = config::get_app_config_dir().join("config.json");

    let has_json = old_json_path.exists();
    let has_db = db_path.exists();

    let migration_config = if !has_db && has_json {
        log::info!("检测到旧版配置文件，验证配置文件...");
        match app_config::MultiAppConfig::load() {
            Ok(config) => {
                log::info!("✓ 配置文件加载成功");
                Some(config)
            }
            Err(e) => {
                log::error!("加载旧配置文件失败: {e}，跳过迁移");
                None
            }
        }
    } else {
        None
    };

    let db = Arc::new(database::Database::init_at_path(&db_path).map_err(|e| {
        log::error!("Failed to init database: {e}");
        AppError::Message(format!("数据库初始化失败: {e}"))
    })?);

    if let Some(config) = migration_config {
        log::info!("开始执行数据迁移...");
        match db.migrate_from_json(&config) {
            Ok(_) => {
                log::info!("✓ 配置迁移成功");
                let archive_path = old_json_path.with_extension("json.migrated");
                if let Err(e) = std::fs::rename(&old_json_path, &archive_path) {
                    log::warn!("归档旧配置文件失败: {e}");
                } else {
                    log::info!("✓ 旧配置已归档为 config.json.migrated");
                }
            }
            Err(e) => {
                log::error!("配置迁移失败: {e}，将从现有配置导入");
            }
        }
    }

    let app_state = AppState::new(db);

    for app_type in AppType::all().filter(|t| !t.is_additive_mode()) {
        if !services::provider::should_import_default_config_on_startup(&app_state, &app_type)
            .unwrap_or(false)
        {
            continue;
        }
        match services::provider::import_default_config(&app_state, app_type.clone()) {
            Ok(true) => log::info!("✓ Imported live config for {}", app_type.as_str()),
            Ok(false) => {}
            Err(e) => log::debug!("○ No live config for {}: {e}", app_type.as_str()),
        }
    }

    match app_state.db.init_default_official_providers() {
        Ok(count) if count > 0 => log::info!("✓ Seeded {count} official provider(s)"),
        _ => {}
    }

    match services::provider::import_opencode_providers_from_live(&app_state) {
        Ok(count) if count > 0 => log::info!("✓ Imported {count} OpenCode provider(s)"),
        _ => {}
    }
    match services::provider::import_openclaw_providers_from_live(&app_state) {
        Ok(count) if count > 0 => log::info!("✓ Imported {count} OpenClaw provider(s)"),
        _ => {}
    }
    match services::provider::import_hermes_providers_from_live(&app_state) {
        Ok(count) if count > 0 => log::info!("✓ Imported {count} Hermes provider(s)"),
        _ => {}
    }

    Ok(app_state)
}

/// Restore proxy state on startup — re-enables takeover for apps that were
/// proxied when the server last shut down.
pub async fn restore_proxy_state(state: &AppState) {
    let mut apps_to_restore = Vec::new();
    for app_type in ["claude", "codex", "gemini"] {
        if let Ok(config) = state.db.get_proxy_config_for_app(app_type).await {
            if config.enabled {
                apps_to_restore.push(app_type);
            }
        }
    }

    if apps_to_restore.is_empty() {
        log::debug!("启动时无需恢复代理状态");
        return;
    }

    log::info!("检测到上次代理状态需要恢复，应用列表: {apps_to_restore:?}");

    for app_type in apps_to_restore {
        match state.proxy_service.set_takeover_for_app(app_type, true).await {
            Ok(()) => {
                log::info!("✓ 已恢复 {app_type} 的代理接管状态");
            }
            Err(e) => {
                log::error!("✗ 恢复 {app_type} 的代理接管状态失败: {e}");
                if let Err(clear_err) = state.proxy_service.set_takeover_for_app(app_type, false).await {
                    log::error!("清除 {app_type} 代理状态失败: {clear_err}");
                }
            }
        }
    }
}

fn init_logging(app_config_dir: &std::path::Path) {
    use std::io::Write;

    let log_dir = app_config_dir.join("logs");
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("创建日志目录失败: {e}");
    }

    let log_file_path = log_dir.join("easycpa.log");

    // 默认 info 级别，用 RUST_LOG=debug 覆盖
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .unwrap_or_else(|e| {
            eprintln!("无法打开日志文件 {}: {e}", log_file_path.display());
            // 回退到 stdout
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
