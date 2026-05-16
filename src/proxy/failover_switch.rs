//! 故障转移切换模块
//!
//! 处理故障转移成功后的供应商切换逻辑，包括：
//! - 去重控制（避免多个请求同时触发）
//! - 供应商热切换

use crate::database::Database;
use crate::error::AppError;
use crate::services::ProxyService;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 故障转移切换管理器
///
/// 负责处理故障转移成功后的供应商切换，确保当前使用的供应商是最优的。
#[derive(Clone)]
pub struct FailoverSwitchManager {
    /// 正在处理中的切换（key = "app_type:provider_id"）
    pending_switches: Arc<RwLock<HashSet<String>>>,
    db: Arc<Database>,
    proxy_service: Arc<RwLock<Option<ProxyService>>>,
}

impl FailoverSwitchManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            pending_switches: Arc::new(RwLock::new(HashSet::new())),
            db,
            proxy_service: Arc::new(RwLock::new(None)),
        }
    }

    /// 设置 ProxyService 引用（在 ProxyService 创建后调用）
    pub async fn set_proxy_service(&self, service: ProxyService) {
        *self.proxy_service.write().await = Some(service);
    }

    /// 尝试执行故障转移切换
    ///
    /// 如果相同的切换已在进行中，则跳过；否则执行切换逻辑。
    ///
    /// # Returns
    /// - `Ok(true)` - 切换成功执行
    /// - `Ok(false)` - 切换已在进行中，跳过
    /// - `Err(e)` - 切换过程中发生错误
    pub async fn try_switch(
        &self,
        app_type: &str,
        provider_id: &str,
        provider_name: &str,
    ) -> Result<bool, AppError> {
        let switch_key = format!("{app_type}:{provider_id}");

        // 去重检查：如果相同切换已在进行中，跳过
        {
            let mut pending = self.pending_switches.write().await;
            if pending.contains(&switch_key) {
                log::debug!("[Failover] 切换已在进行中，跳过: {app_type} -> {provider_id}");
                return Ok(false);
            }
            pending.insert(switch_key.clone());
        }

        // 执行切换（确保最后清理 pending 标记）
        let result = self.do_switch(app_type, provider_id, provider_name).await;

        // 清理 pending 标记
        {
            let mut pending = self.pending_switches.write().await;
            pending.remove(&switch_key);
        }

        result
    }

    async fn do_switch(
        &self,
        app_type: &str,
        provider_id: &str,
        provider_name: &str,
    ) -> Result<bool, AppError> {
        // 检查该应用是否已被代理接管（enabled=true）
        // 只有被接管的应用才允许执行故障转移切换
        let app_enabled = match self.db.get_proxy_config_for_app(app_type).await {
            Ok(config) => config.enabled,
            Err(e) => {
                log::warn!("[FO-002] 无法读取 {app_type} 配置: {e}，跳过切换");
                return Ok(false);
            }
        };

        if !app_enabled {
            log::debug!("[Failover] {app_type} 未启用代理，跳过切换");
            return Ok(false);
        }

        log::info!("[FO-001] 切换: {app_type} → {provider_name}");

        let proxy_service = self.proxy_service.read().await;
        if let Some(service) = proxy_service.as_ref() {
            let switched = service
                .hot_switch_provider(app_type, provider_id)
                .await
                .map_err(AppError::Message)?
                .logical_target_changed;

            if !switched {
                return Ok(false);
            }
        } else {
            log::warn!("[Failover] ProxyService 未初始化，跳过切换");
            return Ok(false);
        }

        Ok(true)
    }
}
