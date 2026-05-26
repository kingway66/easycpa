//! Proxy service — start/stop the HTTP proxy server

use crate::database::Database;
use crate::proxy::server::ProxyServer;
use crate::proxy::switch_lock::SwitchLockManager;
use crate::proxy::types::*;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct ProxyService {
    db: Arc<Database>,
    server: Arc<RwLock<Option<ProxyServer>>>,
    switch_locks: SwitchLockManager,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HotSwitchOutcome {
    pub logical_target_changed: bool,
}

impl ProxyService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            server: Arc::new(RwLock::new(None)),
            switch_locks: SwitchLockManager::new(),
        }
    }

    /// Start the proxy server
    pub async fn start(&self) -> Result<ProxyServerInfo, String> {
        let mut global_config = self
            .db
            .get_global_proxy_config()
            .await
            .map_err(|e| format!("获取全局代理配置失败: {e}"))?;

        if !global_config.proxy_enabled {
            global_config.proxy_enabled = true;
            self.db
                .update_global_proxy_config(global_config.clone())
                .await
                .map_err(|e| format!("更新代理总开关失败: {e}"))?;
        }

        let config = self
            .db
            .get_proxy_config()
            .await
            .map_err(|e| format!("获取代理配置失败: {e}"))?;

        if let Some(server) = self.server.read().await.as_ref() {
            let status = server.get_status().await;
            return Ok(ProxyServerInfo {
                address: status.address,
                port: status.port,
                started_at: chrono::Utc::now().to_rfc3339(),
            });
        }

        let server = ProxyServer::new(config.clone(), self.db.clone());
        server.set_proxy_service(self.clone()).await;
        let info = server
            .start()
            .await
            .map_err(|e| format!("启动代理服务器失败: {e}"))?;

        *self.server.write().await = Some(server);
        log::info!("代理服务器已启动: {}:{}", info.address, info.port);
        Ok(info)
    }

    /// Stop the proxy server
    pub async fn stop(&self) -> Result<(), String> {
        if let Some(server) = self.server.write().await.take() {
            server
                .stop()
                .await
                .map_err(|e| format!("停止代理服务器失败: {e}"))?;

            let mut global_config = self
                .db
                .get_global_proxy_config()
                .await
                .map_err(|e| format!("获取全局代理配置失败: {e}"))?;

            if global_config.proxy_enabled {
                global_config.proxy_enabled = false;
                if let Err(e) = self.db.update_global_proxy_config(global_config).await {
                    log::warn!("更新代理总开关失败: {e}");
                }
            }

            log::info!("代理服务器已停止");
            Ok(())
        } else {
            Err("代理服务器未运行".to_string())
        }
    }

    /// Check if the proxy server is running
    pub async fn is_running(&self) -> bool {
        self.server.read().await.is_some()
    }

    /// Hot-switch the active provider for failover
    ///
    /// In EasyCPA this just updates the database record — no live config takeover.
    pub async fn hot_switch_provider(
        &self,
        app_type: &str,
        provider_id: &str,
    ) -> Result<HotSwitchOutcome, String> {
        let _guard = self.switch_locks.lock_for_app(app_type).await;

        let prev = self
            .db
            .get_current_provider(app_type)
            .ok()
            .flatten();

        self.db
            .set_current_provider(app_type, provider_id)
            .map_err(|e| format!("更新当前供应商失败: {e}"))?;

        Ok(HotSwitchOutcome {
            logical_target_changed: prev.as_deref() != Some(provider_id),
        })
    }

    /// Get current proxy status
    pub async fn get_status(&self) -> Result<ProxyStatus, String> {
        let running = self.is_running().await;
        if running {
            let status = self.server.read().await;
            match status.as_ref() {
                Some(s) => Ok(s.get_status().await),
                None => Ok(ProxyStatus::default()),
            }
        } else {
            Ok(ProxyStatus::default())
        }
    }

    /// Get proxy config
    pub async fn get_config(&self) -> Result<ProxyConfig, String> {
        self.db
            .get_proxy_config()
            .await
            .map_err(|e| format!("获取代理配置失败: {e}"))
    }

    /// Reload runtime config on the live server (called on SIGHUP)
    pub async fn reload_runtime_config(&self, config: &ProxyConfig) {
        if let Some(server) = self.server.read().await.as_ref() {
            server.apply_runtime_config(config).await;
        }
    }

    /// Update proxy config
    pub async fn update_config(&self, config: &ProxyConfig) -> Result<(), String> {
        self.db
            .update_proxy_config(config.clone())
            .await
            .map_err(|e| format!("更新代理配置失败: {e}"))
    }
}