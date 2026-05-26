//! EasyCPA — CPA-compatible proxy for Claude Code & Codex

pub mod config;
pub mod daemon;
pub mod database;
pub mod error;
pub mod provider;
pub mod provider_defaults;
pub mod proxy;
pub mod services;
pub mod store;

use error::AppError;
use proxy::types::{AppProxyConfig, GlobalProxyConfig, OptimizerConfig, RectifierConfig};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::SystemTime;
use store::AppState;

// === 模型路由配置 ===

fn default_listen() -> String {
    "127.0.0.1:15791".to_string()
}

fn default_api_format() -> String {
    "openai_chat".to_string()
}

fn default_false() -> bool {
    false
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum RectifierSetting {
    Bool(bool),
    Detailed(RectifierConfig),
}

impl Default for RectifierSetting {
    fn default() -> Self {
        Self::Detailed(RectifierConfig::default())
    }
}

impl RectifierSetting {
    fn resolve(&self) -> RectifierConfig {
        match self {
            Self::Bool(enabled) => RectifierConfig {
                enabled: *enabled,
                ..RectifierConfig::default()
            },
            Self::Detailed(config) => config.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum OptimizerSetting {
    Bool(bool),
    Detailed(OptimizerConfig),
}

impl Default for OptimizerSetting {
    fn default() -> Self {
        Self::Detailed(OptimizerConfig::default())
    }
}

impl OptimizerSetting {
    fn resolve(&self) -> OptimizerConfig {
        match self {
            Self::Bool(enabled) => OptimizerConfig {
                enabled: *enabled,
                ..OptimizerConfig::default()
            },
            Self::Detailed(config) => config.clone(),
        }
    }
}

/// 模型路由条目
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelRoute {
    #[serde(default)]
    pub name: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rectifier: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optimizer: Option<bool>,
    #[serde(default = "default_false")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SimpleConfig {
    #[serde(default)]
    models: Vec<ModelRoute>,
    #[serde(default = "default_listen")]
    listen: String,
    #[serde(default)]
    rectifier: RectifierSetting,
    #[serde(default)]
    optimizer: OptimizerSetting,
}

#[derive(Debug, Clone)]
struct ModelRouteState {
    routes: Vec<ModelRoute>,
    rectifier: RectifierConfig,
    optimizer: OptimizerConfig,
    source_path: PathBuf,
    modified_at: Option<SystemTime>,
}

/// 全局模型路由表（启动时加载，后台轮询热重载）
static MODEL_ROUTES: OnceLock<RwLock<ModelRouteState>> = OnceLock::new();
static MODEL_ROUTE_WATCHER_STARTED: OnceLock<()> = OnceLock::new();

fn normalize_model_routes(routes: &mut [ModelRoute]) {
    for route in routes {
        if route.name.is_empty() {
            route.name = route.model.clone();
        }
    }
}

fn validate_model_routes(routes: &[ModelRoute]) -> Result<(), String> {
    let mut enabled_by_name: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    for route in routes.iter().filter(|route| route.enabled) {
        *enabled_by_name.entry(route.name.as_str()).or_default() += 1;
    }

    let conflicts: Vec<&str> = enabled_by_name
        .into_iter()
        .filter_map(|(name, count)| (count > 1).then_some(name))
        .collect();

    if conflicts.is_empty() {
        return Ok(());
    }

    Err(format!(
        "同名模型路由只能启用一条，冲突名称: {}",
        conflicts.join(", ")
    ))
}

fn model_route_file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

fn update_model_routes_state(
    config: SimpleConfig,
    source_path: PathBuf,
    modified_at: Option<SystemTime>,
) {
    let new_state = ModelRouteState {
        routes: config.models,
        rectifier: config.rectifier.resolve(),
        optimizer: config.optimizer.resolve(),
        source_path,
        modified_at,
    };

    if let Some(lock) = MODEL_ROUTES.get() {
        *lock.write().unwrap() = new_state;
    } else {
        let _ = MODEL_ROUTES.set(RwLock::new(new_state));
    }
}

fn reload_config_from_file(path: &Path) -> Result<SimpleConfig, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("读取配置文件失败: {e}"))?;
    let mut config: SimpleConfig =
        serde_json::from_str(&content).map_err(|e| format!("解析配置文件失败: {e}"))?;
    normalize_model_routes(&mut config.models);
    validate_model_routes(&config.models)?;
    Ok(config)
}

pub fn reload_model_routes_now() -> Result<usize, String> {
    let Some(lock) = MODEL_ROUTES.get() else {
        return Err("MODEL_ROUTES 尚未初始化".to_string());
    };

    let source_path = lock.read().unwrap().source_path.clone();
    let config = reload_config_from_file(&source_path)?;
    let count = config.models.len();
    update_model_routes_state(
        config,
        source_path.clone(),
        model_route_file_mtime(&source_path),
    );
    log::info!("[Config] 已重载 config.json: {count} 条模型路由");
    Ok(count)
}

fn start_model_route_watcher() {
    if MODEL_ROUTE_WATCHER_STARTED.set(()).is_err() {
        return;
    }

    tokio::spawn(async {
        let poll_interval = std::time::Duration::from_secs(2);
        loop {
            tokio::time::sleep(poll_interval).await;

            let Some(lock) = MODEL_ROUTES.get() else {
                continue;
            };

            let (source_path, previous_mtime) = {
                let state = lock.read().unwrap();
                (state.source_path.clone(), state.modified_at)
            };

            let current_mtime = model_route_file_mtime(&source_path);
            if current_mtime.is_none() || current_mtime <= previous_mtime {
                continue;
            }

            match reload_config_from_file(&source_path) {
                Ok(config) => {
                    let count = config.models.len();
                    update_model_routes_state(config, source_path.clone(), current_mtime);
                    log::info!("[Config] 检测到配置变更，已重载 config.json: {count} 条模型路由");
                }
                Err(e) => {
                    log::warn!("[Config] 后台重载 config.json 失败: {e}");
                }
            }
        }
    });
}

pub fn find_model_route(name: &str) -> Option<ModelRoute> {
    let guard = MODEL_ROUTES.get()?.read().unwrap();
    if let Some(r) = guard.routes.iter().find(|r| r.name == name && r.enabled) {
        return Some(r.clone());
    }
    guard
        .routes
        .iter()
        .find(|r| r.name == "*" && r.enabled)
        .cloned()
}

pub fn get_model_routes() -> Vec<ModelRoute> {
    MODEL_ROUTES
        .get()
        .map(|rw| rw.read().unwrap().routes.clone())
        .unwrap_or_default()
}

pub fn get_rectifier_config() -> RectifierConfig {
    MODEL_ROUTES
        .get()
        .map(|rw| rw.read().unwrap().rectifier.clone())
        .unwrap_or_default()
}

pub fn get_optimizer_config() -> OptimizerConfig {
    MODEL_ROUTES
        .get()
        .map(|rw| rw.read().unwrap().optimizer.clone())
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

    let config_path = config::get_active_config_path().ok_or_else(|| {
        AppError::Config(
            "未找到配置文件 (可执行文件目录/config.json 或 ~/.easycpa/config.json)".to_string(),
        )
    })?;

    log::info!("检测到配置文件，从 {} 加载", config_path.display());
    init_app_from_json(&config_path).await
}

async fn init_app_from_json(json_path: &Path) -> Result<AppState, AppError> {
    let content = std::fs::read_to_string(json_path)
        .map_err(|e| AppError::Config(format!("读取配置文件失败: {e}")))?;

    let mut config: SimpleConfig = serde_json::from_str(&content)
        .map_err(|e| AppError::Config(format!("解析配置文件失败: {e}")))?;

    normalize_model_routes(&mut config.models);
    validate_model_routes(&config.models).map_err(AppError::Config)?;

    let (listen_addr, listen_port) = parse_listen(&config.listen);

    log::info!("已加载 {} 条模型路由", config.models.len());
    for r in &config.models {
        log::info!(
            "  [{}] {} → {} ({})",
            r.name,
            r.model,
            r.base_url,
            r.api_format
        );
    }

    update_model_routes_state(
        config,
        json_path.to_path_buf(),
        model_route_file_mtime(json_path),
    );
    start_model_route_watcher();

    let db = Arc::new(
        database::Database::memory()
            .map_err(|e| AppError::Message(format!("创建内存数据库失败: {e}")))?,
    );

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
    ("127.0.0.1".to_string(), 15791)
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

#[cfg(test)]
mod tests {
    use super::{validate_model_routes, OptimizerSetting, RectifierSetting, SimpleConfig};

    #[test]
    fn model_routes_default_to_disabled() {
        let config: SimpleConfig = serde_json::from_str(
            r#"{
                "models": [
                    {
                        "name": "demo-model",
                        "model": "upstream-a",
                        "base_url": "http://127.0.0.1:19001",
                        "api_key": "sk-test"
                    }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(config.models.len(), 1);
        assert!(!config.models[0].enabled);
    }

    #[test]
    fn duplicate_enabled_routes_with_same_name_are_rejected() {
        let config: SimpleConfig = serde_json::from_str(
            r#"{
                "models": [
                    {
                        "name": "demo-model",
                        "model": "upstream-a",
                        "base_url": "http://127.0.0.1:19001",
                        "api_key": "sk-test-a",
                        "enabled": true
                    },
                    {
                        "name": "demo-model",
                        "model": "upstream-b",
                        "base_url": "http://127.0.0.1:19002",
                        "api_key": "sk-test-b",
                        "enabled": true
                    }
                ]
            }"#,
        )
        .unwrap();

        let err = validate_model_routes(&config.models).unwrap_err();
        assert!(err.contains("demo-model"));
    }

    #[test]
    fn config_defaults_rectifier_and_optimizer() {
        let config: SimpleConfig = serde_json::from_str(
            r#"{
                "models": [],
                "listen": "127.0.0.1:15791"
            }"#,
        )
        .unwrap();

        match config.rectifier {
            RectifierSetting::Detailed(rectifier) => {
                assert!(!rectifier.enabled);
                assert!(rectifier.request_thinking_signature);
                assert!(rectifier.request_thinking_budget);
            }
            RectifierSetting::Bool(_) => panic!("expected detailed default rectifier"),
        }
        match config.optimizer {
            OptimizerSetting::Detailed(optimizer) => {
                assert!(!optimizer.enabled);
                assert!(!optimizer.thinking_optimizer);
                assert!(optimizer.cache_injection);
            }
            OptimizerSetting::Bool(_) => panic!("expected detailed default optimizer"),
        }
    }

    #[test]
    fn config_accepts_boolean_rectifier() {
        let config: SimpleConfig = serde_json::from_str(
            r#"{
                "listen": "127.0.0.1:15791",
                "rectifier": true,
                "models": [
                    {
                        "name": "demo-model",
                        "model": "demo-model",
                        "base_url": "http://127.0.0.1:19001",
                        "api_key": "sk-test",
                        "enabled": true,
                        "rectifier": false,
                        "optimizer": false
                    }
                ]
            }"#,
        )
        .unwrap();

        match config.rectifier {
            RectifierSetting::Bool(enabled) => assert!(enabled),
            RectifierSetting::Detailed(_) => panic!("expected boolean rectifier"),
        }
        assert_eq!(config.models[0].rectifier, Some(false));
        assert_eq!(config.models[0].optimizer, Some(false));
    }

    #[test]
    fn config_accepts_detailed_rectifier_for_backward_compatibility() {
        let config: SimpleConfig = serde_json::from_str(
            r#"{
                "rectifier": {
                    "enabled": true,
                    "requestThinkingSignature": false,
                    "requestThinkingBudget": true
                },
                "models": []
            }"#,
        )
        .unwrap();

        match config.rectifier {
            RectifierSetting::Detailed(rectifier) => {
                assert!(rectifier.enabled);
                assert!(!rectifier.request_thinking_signature);
                assert!(rectifier.request_thinking_budget);
            }
            RectifierSetting::Bool(_) => panic!("expected detailed rectifier"),
        }
    }

    #[test]
    fn config_accepts_boolean_optimizer() {
        let config: SimpleConfig = serde_json::from_str(
            r#"{
                "optimizer": true,
                "models": []
            }"#,
        )
        .unwrap();

        match config.optimizer {
            OptimizerSetting::Bool(enabled) => assert!(enabled),
            OptimizerSetting::Detailed(_) => panic!("expected boolean optimizer"),
        }
    }

    #[test]
    fn config_accepts_detailed_optimizer_for_backward_compatibility() {
        let config: SimpleConfig = serde_json::from_str(
            r#"{
                "optimizer": {
                    "enabled": true,
                    "thinkingOptimizer": true,
                    "cacheInjection": false,
                    "cacheTtl": "5m"
                },
                "models": []
            }"#,
        )
        .unwrap();

        match config.optimizer {
            OptimizerSetting::Detailed(optimizer) => {
                assert!(optimizer.enabled);
                assert!(optimizer.thinking_optimizer);
                assert!(!optimizer.cache_injection);
                assert_eq!(optimizer.cache_ttl, "5m");
            }
            OptimizerSetting::Bool(_) => panic!("expected detailed optimizer"),
        }
    }
}
