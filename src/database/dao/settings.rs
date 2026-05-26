//! Settings DAO — key-value store for app configuration

use crate::database::{lock_conn, Database};
use crate::error::AppError;
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