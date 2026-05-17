//! Provider Adapters Module

mod adapter;
mod auth;
mod claude;
mod codex;
pub mod codex_oauth_auth;
pub mod models;
pub mod streaming;
pub mod streaming_chat_to_responses;
pub mod streaming_responses;
pub mod transform;
pub mod transform_responses;
pub mod transform_responses_chat;

use serde::{Deserialize, Serialize};

// 公开导出
pub use adapter::ProviderAdapter;
pub use auth::{AuthInfo, AuthStrategy};
pub use claude::{
    claude_api_format_needs_transform, get_claude_api_format,
    transform_claude_request_for_api_format, ClaudeAdapter,
};
pub use codex::CodexAdapter;

/// Get adapter by app type string
pub fn get_adapter(app_type: &str) -> &'static dyn ProviderAdapter {
    match app_type {
        "codex" => &CodexAdapter,
        _ => &ClaudeAdapter,
    }
}

/// Provider type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderType {
    Claude,
    ClaudeAuth,
    Codex,
    OpenRouter,
    GitHubCopilot,
    CodexOAuth,
}

impl ProviderType {
    pub fn needs_transform(&self) -> bool {
        matches!(self, ProviderType::GitHubCopilot | ProviderType::CodexOAuth)
    }

    pub fn from_app_type_and_config(app_type: &str, provider: &crate::provider::Provider) -> Self {
        let env = provider.settings_config.get("env").and_then(|v| v.as_object());
        let has_oauth = env
            .map(|e| e.contains_key("OPENAI_API_KEY") || e.contains_key("CHATGPT_API_KEY"))
            .unwrap_or(false);

        match app_type {
            "claude" if has_oauth => ProviderType::ClaudeAuth,
            "claude" => ProviderType::Claude,
            "codex" if has_oauth => ProviderType::CodexOAuth,
            "codex" => ProviderType::Codex,
            _ => ProviderType::Claude,
        }
    }
}

pub fn get_adapter_for_provider_type(provider_type: &ProviderType) -> &'static dyn ProviderAdapter {
    match provider_type {
        ProviderType::Claude | ProviderType::ClaudeAuth | ProviderType::OpenRouter | ProviderType::GitHubCopilot => &ClaudeAdapter,
        ProviderType::Codex | ProviderType::CodexOAuth => &CodexAdapter,
    }
}