//! Minimal in-process usage cache (write-through, no persistence)

use std::collections::HashMap;
use std::sync::RwLock;

use crate::provider::UsageResult;

#[derive(Default)]
pub struct UsageCache {
    script: RwLock<HashMap<(String, String), UsageResult>>,
}

impl UsageCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put_script(&self, app_type: &str, provider_id: &str, result: UsageResult) {
        if let Ok(mut w) = self.script.write() {
            w.insert((app_type.to_string(), provider_id.to_string()), result);
        }
    }

    pub fn with_script<R>(
        &self,
        app_type: &str,
        provider_id: &str,
        f: impl FnOnce(&UsageResult) -> R,
    ) -> Option<R> {
        self.script.read().ok().and_then(|r| {
            r.get(&(app_type.to_string(), provider_id.to_string()))
                .map(f)
        })
    }

    pub fn invalidate_script(&self, app_type: &str, provider_id: &str) {
        let key = (app_type.to_string(), provider_id.to_string());
        if let Ok(mut w) = self.script.write() {
            w.remove(&key);
        }
    }
}
