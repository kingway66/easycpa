//! Config management API handlers
//!
//! REST endpoints for managing model routes, Claude settings files, and Codex config

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use crate::config;
use crate::ModelRoute;

// === Model Routes API ===

#[derive(Serialize)]
pub struct ModelsResponse {
    models: Vec<ModelRoute>,
}

pub async fn list_models() -> Json<ModelsResponse> {
    Json(ModelsResponse {
        models: crate::get_model_routes(),
    })
}

pub async fn get_model(Path(name): Path<String>) -> Result<Json<ModelRoute>, StatusCode> {
    crate::find_model_route(&name)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn save_model(
    Path(name): Path<String>,
    Json(mut model): Json<ModelRoute>,
) -> Result<Json<Value>, (StatusCode, String)> {
    model.name = name;
    let config_path = config::get_config_json_path();
    let mut routes = crate::get_model_routes();

    // 互斥：如果保存为 enabled，禁用其他同名路由
    if model.enabled {
        for r in &mut routes {
            if r.name == model.name && r.enabled {
                r.enabled = false;
            }
        }
    }

    // 用 name + base_url + model 定位路由（支持同名多路由）
    let key = (&model.name, &model.base_url, &model.model);
    if let Some(existing) = routes.iter_mut().find(|r| (&r.name, &r.base_url, &r.model) == key) {
        *existing = model.clone();
    } else {
        routes.push(model.clone());
    }

    save_routes_to_config(&config_path, &routes)?;
    invalidate_route_cache();

    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct DeleteModelQuery {
    base_url: Option<String>,
    model: Option<String>,
}

pub async fn delete_model(
    Path(name): Path<String>,
    Query(query): Query<DeleteModelQuery>,
) -> Result<StatusCode, (StatusCode, String)> {
    let config_path = config::get_config_json_path();
    let mut routes = crate::get_model_routes();
    let before = routes.len();

    if let (Some(base_url), Some(model)) = (&query.base_url, &query.model) {
        routes.retain(|r| !(r.name == name && r.base_url == *base_url && r.model == *model));
    } else {
        routes.retain(|r| r.name != name);
    }

    if routes.len() == before {
        return Ok(StatusCode::NO_CONTENT);
    }

    save_routes_to_config(&config_path, &routes)?;
    invalidate_route_cache();

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct ConfigFile {
    listen: String,
    models: Vec<ModelRoute>,
}

fn save_routes_to_config(
    config_path: &std::path::Path,
    routes: &[ModelRoute],
) -> Result<(), (StatusCode, String)> {
    let listen = get_current_listen(config_path);
    // Sort: same-name routes grouped together, wildcard "*" always last
    let mut sorted_routes = routes.to_vec();
    sorted_routes.sort_by(|a, b| {
        let a_wild = a.name == "*";
        let b_wild = b.name == "*";
        match b_wild.cmp(&a_wild) {
            std::cmp::Ordering::Equal => a.name.cmp(&b.name),
            other => other,
        }
    });
    let config = ConfigFile {
        listen,
        models: sorted_routes,
    };

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    config::write_text_file(config_path, &json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

fn get_current_listen(config_path: &std::path::Path) -> String {
    std::fs::read_to_string(config_path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|v| v.get("listen")?.as_str().map(String::from))
        .unwrap_or_else(|| "127.0.0.1:15791".to_string())
}

fn invalidate_route_cache() {
    // Bump the mtime on config.json so the hot-reload check picks up changes
    let config_path = config::get_config_json_path();
    let now = std::time::SystemTime::now();
    let _ = filetime::set_file_mtime(&config_path, filetime::FileTime::from_system_time(now));
}

// === Claude Settings API ===

#[derive(Serialize, Deserialize, Clone)]
pub struct ClaudeSettingsFile {
    pub filename: String,
    pub env: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct ClaudeSettingsResponse {
    files: Vec<ClaudeSettingsFile>,
}

pub async fn list_claude_settings() -> Json<ClaudeSettingsResponse> {
    let dir = config::get_claude_config_dir();
    let mut files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with("settings.") && fname.ends_with(".json") && fname != "settings.json" {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    if let Ok(mut val) = serde_json::from_str::<Value>(&content) {
                        let env = val
                            .get_mut("env")
                            .map(|v| v.take())
                            .and_then(|v| serde_json::from_value::<HashMap<String, String>>(v).ok())
                            .unwrap_or_default();
                        files.push(ClaudeSettingsFile { filename: fname, env });
                    }
                }
            }
        }
    }

    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    Json(ClaudeSettingsResponse { files })
}

#[derive(Deserialize)]
pub struct SaveClaudeSettingsRequest {
    env: HashMap<String, String>,
}

pub async fn save_claude_settings(
    Path(filename): Path<String>,
    Json(body): Json<SaveClaudeSettingsRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if !filename.starts_with("settings.") || !filename.ends_with(".json") || filename == "settings.json" {
        return Err((StatusCode::BAD_REQUEST, "Invalid settings filename".to_string()));
    }

    let path = config::get_claude_config_dir().join(&filename);

    // Read existing file to preserve other fields, or start fresh
    let mut existing: Value = if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or(json!({}))
    } else {
        json!({})
    };

    let env_val = serde_json::to_value(&body.env)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    existing.as_object_mut().unwrap().insert("env".to_string(), env_val);

    config::write_json_file(&path, &existing)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({"ok": true})))
}

pub async fn delete_claude_settings(
    Path(filename): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    if !filename.starts_with("settings.") || !filename.ends_with(".json") || filename == "settings.json" {
        return Err((StatusCode::BAD_REQUEST, "Invalid settings filename".to_string()));
    }

    let path = config::get_claude_config_dir().join(&filename);
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

// === Codex Config API ===
//
// The Codex config.toml uses table-of-tables for model_providers and profiles:
//   [model_providers.rightcode]
//   name = "rightcode"
//   base_url = "..."
//
// We use toml_edit to preserve the full file structure (projects, plugins, etc.)
// while only reading/writing the model_providers and profiles sections.

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CodexModelProvider {
    pub name: String,
    pub base_url: String,
    pub wire_api: String,
    #[serde(default)]
    pub requires_openai_auth: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CodexProfile {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_reasoning_effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_auth_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_context_window: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_auto_compact_token_limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approvals_reviewer: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct CodexConfigFile {
    pub model_providers: Vec<CodexModelProvider>,
    pub profiles: Vec<CodexProfile>,
}

fn get_codex_config_path() -> PathBuf {
    config::get_home_dir().join(".codex").join("config.toml")
}

/// Read model_providers and profiles from config.toml using toml_edit
fn read_codex_config() -> Result<CodexConfigFile, (StatusCode, String)> {
    let path = get_codex_config_path();
    if !path.exists() {
        return Ok(CodexConfigFile::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let doc = content.parse::<toml_edit::DocumentMut>()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Parse codex config: {e}")))?;

    let mut result = CodexConfigFile::default();

    // Extract model_providers from [model_providers.<name>] sections
    if let Some(providers) = doc.get("model_providers").and_then(|v| v.as_table_like()) {
        for (key, val) in providers.iter() {
            if let Some(table) = val.as_table_like() {
                let provider = CodexModelProvider {
                    name: table.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(key)
                        .to_string(),
                    base_url: table.get("base_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    wire_api: table.get("wire_api")
                        .and_then(|v| v.as_str())
                        .unwrap_or("responses")
                        .to_string(),
                    requires_openai_auth: table.get("requires_openai_auth")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                };
                result.model_providers.push(provider);
            }
        }
    }

    // Extract profiles from [profiles.<name>] sections
    if let Some(profiles) = doc.get("profiles").and_then(|v| v.as_table_like()) {
        for (key, val) in profiles.iter() {
            if let Some(table) = val.as_table_like() {
                let profile = CodexProfile {
                    name: table.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(key)
                        .to_string(),
                    model_provider: table.get("model_provider").and_then(|v| v.as_str()).map(String::from),
                    model: table.get("model").and_then(|v| v.as_str()).map(String::from),
                    model_reasoning_effort: table.get("model_reasoning_effort").and_then(|v| v.as_str()).map(String::from),
                    preferred_auth_method: table.get("preferred_auth_method").and_then(|v| v.as_str()).map(String::from),
                    model_context_window: table.get("model_context_window").and_then(|v| v.as_integer()),
                    model_auto_compact_token_limit: table.get("model_auto_compact_token_limit").and_then(|v| v.as_integer()),
                    approvals_reviewer: table.get("approvals_reviewer").and_then(|v| v.as_str()).map(String::from),
                };
                result.profiles.push(profile);
            }
        }
    }

    Ok(result)
}

/// Write model_providers and profiles back into config.toml using toml_edit,
/// preserving all other sections (projects, plugins, analytics, etc.)
fn write_codex_config_section(
    providers: &[CodexModelProvider],
    profiles: &[CodexProfile],
) -> Result<(), (StatusCode, String)> {
    let path = get_codex_config_path();
    let content = if path.exists() {
        std::fs::read_to_string(&path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        String::new()
    };

    let mut doc = content.parse::<toml_edit::DocumentMut>()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Parse codex config: {e}")))?;

    // Rebuild model_providers section
    let mut new_providers = toml_edit::Table::new();
    for p in providers {
        let mut table = toml_edit::Table::new();
        table.insert("name", toml_edit::value(&p.name));
        table.insert("base_url", toml_edit::value(&p.base_url));
        table.insert("wire_api", toml_edit::value(&p.wire_api));
        table.insert("requires_openai_auth", toml_edit::value(p.requires_openai_auth));
        new_providers.insert(&p.name, toml_edit::Item::Table(table));
    }
    doc.insert("model_providers", toml_edit::Item::Table(new_providers));

    // Rebuild profiles section
    let mut new_profiles = toml_edit::Table::new();
    for p in profiles {
        let mut table = toml_edit::Table::new();
        if let Some(v) = &p.model_provider { table.insert("model_provider", toml_edit::value(v)); }
        if let Some(v) = &p.model { table.insert("model", toml_edit::value(v)); }
        if let Some(v) = &p.model_reasoning_effort { table.insert("model_reasoning_effort", toml_edit::value(v)); }
        if let Some(v) = &p.preferred_auth_method { table.insert("preferred_auth_method", toml_edit::value(v)); }
        if let Some(v) = p.model_context_window { table.insert("model_context_window", toml_edit::value(v)); }
        if let Some(v) = p.model_auto_compact_token_limit { table.insert("model_auto_compact_token_limit", toml_edit::value(v)); }
        if let Some(v) = &p.approvals_reviewer { table.insert("approvals_reviewer", toml_edit::value(v)); }
        new_profiles.insert(&p.name, toml_edit::Item::Table(table));
    }
    doc.insert("profiles", toml_edit::Item::Table(new_profiles));

    config::write_text_file(&path, &doc.to_string())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn get_codex_config() -> Result<Json<CodexConfigFile>, (StatusCode, String)> {
    let config = read_codex_config()?;
    Ok(Json(config))
}

pub async fn save_codex_provider(
    Json(provider): Json<CodexModelProvider>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let mut config = read_codex_config()?;

    if let Some(existing) = config.model_providers.iter_mut().find(|p| p.name == provider.name) {
        *existing = provider;
    } else {
        config.model_providers.push(provider);
    }

    write_codex_config_section(&config.model_providers, &config.profiles)?;
    Ok(Json(json!({"ok": true})))
}

pub async fn delete_codex_provider(
    Path(name): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut config = read_codex_config()?;
    let before = config.model_providers.len();
    config.model_providers.retain(|p| p.name != name);
    if config.model_providers.len() == before {
        return Ok(StatusCode::NO_CONTENT);
    }
    write_codex_config_section(&config.model_providers, &config.profiles)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn save_codex_profile(
    Json(profile): Json<CodexProfile>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let mut config = read_codex_config()?;

    if let Some(existing) = config.profiles.iter_mut().find(|p| p.name == profile.name) {
        *existing = profile;
    } else {
        config.profiles.push(profile);
    }

    write_codex_config_section(&config.model_providers, &config.profiles)?;
    Ok(Json(json!({"ok": true})))
}

pub async fn delete_codex_profile(
    Path(name): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut config = read_codex_config()?;
    let before = config.profiles.len();
    config.profiles.retain(|p| p.name != name);
    if config.profiles.len() == before {
        return Ok(StatusCode::NO_CONTENT);
    }
    write_codex_config_section(&config.model_providers, &config.profiles)?;
    Ok(StatusCode::NO_CONTENT)
}

// === Server Lifecycle API ===

pub async fn server_reload() -> Json<Value> {
    let pid = std::process::id();
    unsafe {
        libc::kill(pid as i32, libc::SIGHUP);
    }
    Json(json!({"ok": true}))
}

pub async fn server_restart() -> Json<Value> {
    let exe = std::env::current_exe().unwrap();
    let log_path = crate::daemon::log_path();

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path);

    match log_file {
        Ok(lf) => {
            let lf_err = lf.try_clone().unwrap_or_else(|_| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                    .unwrap()
            });

            // Detach stdin so child isn't tied to current process
            let child = Command::new(&exe)
                .args(["start", "--foreground", "--no-open"])
                .stdin(std::process::Stdio::null())
                .stdout(lf)
                .stderr(lf_err)
                .spawn();

            match child {
                Ok(_) => {
                    // Remove PID file so new process can write its own
                    crate::daemon::remove_pid();
                    // Exit immediately — the child will retry binding until port is free
                    tokio::spawn(async {
                        std::process::exit(0);
                    });
                    Json(json!({"ok": true}))
                }
                Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
            }
        }
        Err(e) => Json(json!({"ok": false, "error": format!("cannot open log: {e}")})),
    }
}
