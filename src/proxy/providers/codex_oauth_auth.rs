//! Codex OAuth — reads tokens from ~/.codex/auth.json
//!
//! Matches the official openai/codex CLI auth.json structure (TokenData + IdTokenInfo).
//! No login / refresh / logout — relies on `codex` CLI to authenticate.
//! Impersonates the official `codex_cli_rs` client in upstream headers to avoid bans.

use base64::Engine;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

/// Codex OAuth error
#[derive(Debug, thiserror::Error)]
pub enum CodexOAuthError {
    #[error("auth.json not found at {0}")]
    AuthFileNotFound(String),

    #[error("auth.json parse error: {0}")]
    ParseError(String),

    #[error("no tokens in auth.json")]
    NoTokens,

    #[error("IO error: {0}")]
    IoError(String),
}

impl From<std::io::Error> for CodexOAuthError {
    fn from(err: std::io::Error) -> Self {
        CodexOAuthError::IoError(err.to_string())
    }
}

// ── auth.json structure (matches codex-rs/login/src/token_data.rs) ──

/// Flat subset of useful claims from the id_token JWT.
/// Matches `IdTokenInfo` in codex-rs/login/src/token_data.rs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdTokenInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// ChatGPT subscription plan: free / plus / pro / team / enterprise / edu
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chatgpt_plan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chatgpt_user_id: Option<String>,
    /// Workspace / organization id
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chatgpt_account_id: Option<String>,
    #[serde(default)]
    pub chatgpt_account_is_fedramp: bool,
    #[serde(default)]
    pub raw_jwt: String,
}

/// Token data matching `TokenData` in codex-rs/login/src/token_data.rs.
/// The `id_token` field deserializes from either a raw JWT string (old format)
/// or a pre-parsed `IdTokenInfo` object (new format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexTokenData {
    #[serde(deserialize_with = "deserialize_id_token")]
    pub id_token: IdTokenInfo,
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

/// Top-level auth.json — matches `AuthDotJson` in codex-rs/login/src/auth/storage.rs
#[derive(Debug, Clone, Deserialize)]
pub struct CodexAuthJson {
    #[serde(default)]
    pub auth_mode: Option<String>,
    #[serde(rename = "OPENAI_API_KEY", default)]
    pub openai_api_key: Option<String>,
    pub tokens: Option<CodexTokenData>,
    #[serde(default)]
    pub last_refresh: Option<String>,
    #[serde(default)]
    pub agent_identity: Option<String>,
}

/// Legacy auth.json format where tokens are a flat struct
#[derive(Debug, Clone, Deserialize)]
struct LegacyTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub account_id: String,
    #[serde(default)]
    pub id_token: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedCodexCredentials {
    modified_at: SystemTime,
    credentials: CodexCredentials,
}

static CODEX_AUTH_CACHE: Lazy<Mutex<Option<CachedCodexCredentials>>> =
    Lazy::new(|| Mutex::new(None));

/// Credentials returned to the forwarder
#[derive(Debug, Clone)]
pub struct CodexCredentials {
    pub access_token: String,
    pub account_id: String,
    pub email: Option<String>,
    pub plan_type: Option<String>,
}

// ── JWT parsing (no crypto verification, same as official code) ──

/// Parse the id_token JWT into IdTokenInfo.
/// Does NOT verify the signature — extracts claims only (same as official codex-rs).
fn parse_id_token(jwt: &str) -> IdTokenInfo {
    let raw_jwt = jwt.to_string();
    let mut parts = jwt.split('.');
    let (_header_b64, payload_b64, _sig_b64) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => {
            return IdTokenInfo {
                raw_jwt,
                ..Default::default()
            };
        }
    };

    let payload_bytes = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload_b64) {
        Ok(b) => b,
        Err(_) => {
            return IdTokenInfo {
                raw_jwt,
                ..Default::default()
            };
        }
    };

    let claims: serde_json::Value = match serde_json::from_slice(&payload_bytes) {
        Ok(v) => v,
        Err(_) => {
            return IdTokenInfo {
                raw_jwt,
                ..Default::default()
            };
        }
    };

    // Extract email from top-level or profile sub-claim (same logic as official)
    let email = claims
        .get("email")
        .and_then(|v| v.as_str())
        .or_else(|| {
            claims
                .get("https://api.openai.com/profile")
                .and_then(|v| v.get("email"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string());

    // Extract auth sub-claims from https://api.openai.com/auth
    let auth = claims.get("https://api.openai.com/auth");

    let chatgpt_plan_type = auth
        .and_then(|v| v.get("chatgpt_plan_type"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let chatgpt_user_id = auth
        .and_then(|v| v.get("chatgpt_user_id"))
        .or_else(|| auth.and_then(|v| v.get("user_id")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let chatgpt_account_id = auth
        .and_then(|v| v.get("chatgpt_account_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let chatgpt_account_is_fedramp = auth
        .and_then(|v| v.get("chatgpt_account_is_fedramp"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    IdTokenInfo {
        email,
        chatgpt_plan_type,
        chatgpt_user_id,
        chatgpt_account_id,
        chatgpt_account_is_fedramp,
        raw_jwt,
    }
}

// ── Custom deserializer: id_token can be a raw JWT string or a parsed object ──

fn deserialize_id_token<'de, D>(deserializer: D) -> Result<IdTokenInfo, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct IdTokenVisitor;

    impl<'de> de::Visitor<'de> for IdTokenVisitor {
        type Value = IdTokenInfo;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a JWT string or an IdTokenInfo object")
        }

        fn visit_str<E: de::Error>(self, s: &str) -> Result<Self::Value, E> {
            Ok(parse_id_token(s))
        }

        fn visit_map<M: de::MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
            // Already-parsed IdTokenInfo object
            IdTokenInfo::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_any(IdTokenVisitor)
}

// ── Public API ──

fn auth_file_path() -> Result<std::path::PathBuf, CodexOAuthError> {
    let home = dirs::home_dir().ok_or_else(|| {
        CodexOAuthError::IoError("cannot determine home directory".to_string())
    })?;
    Ok(home.join(".codex").join("auth.json"))
}

fn parse_codex_auth_content(content: &str) -> Result<CodexCredentials, CodexOAuthError> {
    if let Ok(auth) = serde_json::from_str::<CodexAuthJson>(content) {
        if let Some(tokens) = auth.tokens {
            let account_id = tokens
                .account_id
                .or_else(|| tokens.id_token.chatgpt_account_id.clone())
                .unwrap_or_default();

            return Ok(CodexCredentials {
                access_token: tokens.access_token,
                account_id,
                email: tokens.id_token.email,
                plan_type: tokens.id_token.chatgpt_plan_type,
            });
        }
    }

    let legacy: LegacyTokens =
        serde_json::from_str(content).map_err(|e| CodexOAuthError::ParseError(e.to_string()))?;

    let id_info = legacy
        .id_token
        .as_deref()
        .map(parse_id_token)
        .unwrap_or_default();

    Ok(CodexCredentials {
        access_token: legacy.access_token,
        account_id: id_info
            .chatgpt_account_id
            .unwrap_or(legacy.account_id),
        email: id_info.email,
        plan_type: id_info.chatgpt_plan_type,
    })
}

fn modified_time(path: &Path) -> Result<SystemTime, CodexOAuthError> {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .map_err(|e| CodexOAuthError::IoError(e.to_string()))
}

/// Read ~/.codex/auth.json and return credentials.
/// Supports both new format (with parsed id_token) and legacy format (flat tokens).
pub fn read_codex_auth() -> Result<CodexCredentials, CodexOAuthError> {
    let path = auth_file_path()?;

    if !path.exists() {
        return Err(CodexOAuthError::AuthFileNotFound(
            path.to_string_lossy().to_string(),
        ));
    }

    let modified_at = modified_time(&path)?;
    if let Some(cached) = CODEX_AUTH_CACHE
        .lock()
        .map_err(|e| CodexOAuthError::IoError(format!("cache lock poisoned: {e}")))?
        .as_ref()
        .filter(|cached| cached.modified_at == modified_at)
        .cloned()
    {
        return Ok(cached.credentials);
    }

    let content =
        std::fs::read_to_string(&path).map_err(|e| CodexOAuthError::IoError(e.to_string()))?;
    let credentials = parse_codex_auth_content(&content)?;

    *CODEX_AUTH_CACHE
        .lock()
        .map_err(|e| CodexOAuthError::IoError(format!("cache lock poisoned: {e}")))? = Some(
        CachedCodexCredentials {
            modified_at,
            credentials: credentials.clone(),
        },
    );

    Ok(credentials)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_jwt_claims() {
        // A minimal JWT with known claims
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"alg":"none","typ":"JWT"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            br#"{"email":"test@example.com","https://api.openai.com/auth":{"chatgpt_plan_type":"plus","chatgpt_user_id":"u-123","chatgpt_account_id":"org-456"}}"#,
        );
        let sig = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"sig");
        let jwt = format!("{header}.{payload}.{sig}");

        let info = parse_id_token(&jwt);
        assert_eq!(info.email.as_deref(), Some("test@example.com"));
        assert_eq!(info.chatgpt_plan_type.as_deref(), Some("plus"));
        assert_eq!(info.chatgpt_user_id.as_deref(), Some("u-123"));
        assert_eq!(info.chatgpt_account_id.as_deref(), Some("org-456"));
    }

    #[test]
    fn test_parse_invalid_jwt() {
        let info = parse_id_token("not.a.jwt");
        assert!(info.email.is_none());
        assert_eq!(info.raw_jwt, "not.a.jwt");
    }

    #[test]
    fn test_legacy_format() {
        let json = serde_json::json!({
            "tokens": {
                "access_token": "eyJtest",
                "refresh_token": "rt_test",
                "account_id": "acc-123"
            }
        });
        let legacy: LegacyTokens = serde_json::from_value(json).unwrap();
        assert_eq!(legacy.access_token, "eyJtest");
        assert_eq!(legacy.account_id, "acc-123");
    }

    #[test]
    fn test_new_format_with_raw_jwt() {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"alg":"none","typ":"JWT"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"org-789"}}"#,
        );
        let sig = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"sig");
        let jwt = format!("{header}.{payload}.{sig}");

        let json = serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": jwt,
                "access_token": "at_test",
                "refresh_token": "rt_test"
            }
        });
        let auth: CodexAuthJson = serde_json::from_value(json).unwrap();
        let tokens = auth.tokens.unwrap();
        assert_eq!(tokens.access_token, "at_test");
        assert_eq!(
            tokens.id_token.chatgpt_account_id.as_deref(),
            Some("org-789")
        );
    }
}
