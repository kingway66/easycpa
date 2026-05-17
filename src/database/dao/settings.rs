//! Settings DAO — key-value store for app configuration

use crate::database::{lock_conn, Database};
use crate::error::AppError;
use crate::proxy::types::{CopilotOptimizerConfig, OptimizerConfig, RectifierConfig};
use rusqlite::params;

impl Database {
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn
            .prepare("SELECT value FROM settings WHERE key = ?1")
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut rows = stmt
            .query(params![key])
            .map_err(|e| AppError::Database(e.to_string()))?;
        if let Some(row) = rows.next().map_err(|e| AppError::Database(e.to_string()))? {
            let value: String = row.get(0).map_err(|e| AppError::Database(e.to_string()))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    pub fn get_bool_flag(&self, key: &str) -> Result<bool, AppError> {
        Ok(matches!(self.get_setting(key)?.as_deref(), Some("true") | Some("1")))
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_rectifier_config(&self) -> Result<RectifierConfig, AppError> {
        match self.get_setting("rectifier_config")? {
            Some(json) => serde_json::from_str(&json)
                .map_err(|e| AppError::Database(format!("解析整流器配置失败: {e}"))),
            None => Ok(RectifierConfig::default()),
        }
    }

    pub fn get_optimizer_config(&self) -> Result<OptimizerConfig, AppError> {
        match self.get_setting("optimizer_config")? {
            Some(json) => serde_json::from_str(&json)
                .map_err(|e| AppError::Database(format!("解析优化器配置失败: {e}"))),
            None => Ok(OptimizerConfig::default()),
        }
    }

    pub fn get_copilot_optimizer_config(&self) -> Result<CopilotOptimizerConfig, AppError> {
        match self.get_setting("copilot_optimizer_config")? {
            Some(json) => serde_json::from_str(&json)
                .map_err(|e| AppError::Database(format!("解析 Copilot 优化器配置失败: {e}"))),
            None => Ok(CopilotOptimizerConfig::default()),
        }
    }

    pub fn get_gateway_token(&self) -> Result<Option<String>, AppError> {
        self.get_setting("gateway_token")
    }

    pub fn get_or_create_gateway_token(&self) -> Result<String, AppError> {
        if let Some(token) = self.get_setting("gateway_token")? {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
        let token = format!("easycpa-{}", uuid::Uuid::new_v4().simple());
        self.set_setting("gateway_token", &token)?;
        Ok(token)
    }
}