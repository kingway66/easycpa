//! Minimal provider service stub
//!
//! EasyCPA routes via config.json ModelRoute — no database provider CRUD needed.
//! This stub keeps the type exports so downstream references compile.

use crate::error::AppError;
use crate::store::AppState;

/// Provider business logic service (stub — routing is via config.json)
pub struct ProviderService;

/// Result of a provider switch operation (stub)
#[derive(Debug, serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SwitchResult {
    pub warnings: Vec<String>,
}

/// Sort order update entry (stub)
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ProviderSortUpdate {
    pub id: String,
    pub sort_order: i32,
}

impl ProviderService {
    pub fn switch(_state: &AppState, _id: &str) -> Result<SwitchResult, AppError> {
        Ok(SwitchResult::default())
    }
}