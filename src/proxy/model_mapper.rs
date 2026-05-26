//! 模型映射模块
//!
//! 从独立配置文件 `~/.easycpa/model-mapping.json` 读取映射规则。
//! 不读取 Provider 的 env 配置，不映射则原样透传。

use crate::config;
use crate::provider::Provider;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

/// 单条映射规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingRule {
    /// 匹配模式（子串，大小写不敏感）
    pub match_pattern: String,
    /// 替换后的模型名
    pub replace: String,
    /// 限定供应商名称或 ID（可选，大小写不敏感子串匹配）
    /// 不设置则对所有供应商生效
    #[serde(default)]
    pub provider: Option<String>,
}

/// 独立模型映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandaloneMapping {
    /// 映射规则列表（按顺序匹配，首条命中生效）
    #[serde(default)]
    pub mappings: Vec<MappingRule>,
    /// 默认模型（所有未匹配的模型都替换为此值，null = 不替换）
    #[serde(default)]
    pub default: Option<String>,
}

impl StandaloneMapping {
    /// 是否有任何映射规则
    pub fn has_mapping(&self) -> bool {
        !self.mappings.is_empty() || self.default.is_some()
    }

    /// 根据原始模型名查找映射
    ///
    /// `provider_name` 和 `provider_id` 用于匹配规则中的 `provider` 字段
    pub fn map_model(
        &self,
        original_model: &str,
        provider_name: &str,
        provider_id: &str,
    ) -> String {
        let model_lower = original_model.to_lowercase();

        // 按规则顺序匹配，首条命中
        for rule in &self.mappings {
            // 检查供应商限定
            if let Some(ref rule_provider) = rule.provider {
                let rp = rule_provider.to_lowercase();
                let name_match = provider_name.to_lowercase().contains(&rp);
                let id_match = provider_id.to_lowercase().contains(&rp);
                if !name_match && !id_match {
                    continue;
                }
            }

            if model_lower.contains(&rule.match_pattern.to_lowercase()) {
                return rule.replace.clone();
            }
        }

        // 默认模型
        if let Some(ref default) = self.default {
            return default.clone();
        }

        // 无映射，保持原样
        original_model.to_string()
    }
}

/// 全局缓存的映射配置（支持热重载）
/// RwLock 内存 (mapping, last_mtime)
static STANDALONE_MAPPING: OnceLock<RwLock<(Option<StandaloneMapping>, SystemTime)>> =
    OnceLock::new();

/// 初始化映射缓存（启动时调用一次）
pub fn init_model_mapping_cache() {
    let path = config::get_proxy_dir().join("model-mapping.json");
    let mapping = if path.exists() {
        parse_mapping_file(&path)
    } else {
        log::debug!(
            "[ModelMapper] 未找到映射配置文件: {}，模型将原样透传",
            path.display()
        );
        None
    };
    let mtime = if path.exists() {
        std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    } else {
        SystemTime::UNIX_EPOCH
    };
    let _ = STANDALONE_MAPPING.set(RwLock::new((mapping, mtime)));
}

/// 从文件解析映射配置
fn parse_mapping_file(path: &std::path::Path) -> Option<StandaloneMapping> {
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<StandaloneMapping>(&content) {
            Ok(mapping) => {
                log::info!(
                    "[ModelMapper] 已加载映射配置: {} 条规则, default={:?}",
                    mapping.mappings.len(),
                    mapping.default
                );
                Some(mapping)
            }
            Err(e) => {
                log::warn!("[ModelMapper] 映射配置解析失败: {e}");
                None
            }
        },
        Err(e) => {
            log::warn!("[ModelMapper] 读取映射配置失败: {e}");
            None
        }
    }
}

/// 检查 mtime 并在需要时重载映射配置
fn check_and_reload_mapping() {
    let Some(guard) = STANDALONE_MAPPING.get() else {
        return;
    };
    let path = config::get_proxy_dir().join("model-mapping.json");

    if !path.exists() {
        return;
    }

    let Ok(meta) = std::fs::metadata(&path) else {
        return;
    };
    let Ok(mtime) = meta.modified() else { return };

    {
        let r = guard.read().unwrap();
        if mtime <= r.1 {
            return;
        }
    }

    let mut w = guard.write().unwrap();
    if mtime <= w.1 {
        return;
    }
    let new_mapping = parse_mapping_file(&path);
    log::info!("[ModelMapper] 已重载映射配置");
    *w = (new_mapping, mtime);
}

/// 加载独立映射配置文件（带 mtime 检查）
fn load_standalone_mapping() -> Option<StandaloneMapping> {
    check_and_reload_mapping();
    STANDALONE_MAPPING
        .get()
        .and_then(|rw| rw.read().unwrap().0.clone())
}

/// 对请求体应用模型映射
///
/// 返回 (映射后的请求体, 原始模型名, 映射后模型名)
///
/// 逻辑：
/// - 如果 `~/.easycpa/model-mapping.json` 存在 → 使用它
/// - 如果不存在 → 模型原样透传
/// - **不读取 Provider 的 env 配置**
pub fn apply_model_mapping(
    mut body: Value,
    provider: &Provider,
) -> (Value, Option<String>, Option<String>) {
    let original_model = body.get("model").and_then(|m| m.as_str()).map(String::from);

    let Some(ref original) = original_model else {
        return (body, original_model, None);
    };

    let mapping = load_standalone_mapping();

    let Some(mapping) = mapping else {
        // 没有配置文件，原样透传
        return (body, original_model, None);
    };

    if !mapping.has_mapping() {
        return (body, original_model, None);
    }

    let mapped = mapping.map_model(original, &provider.name, &provider.id);

    if mapped != *original {
        log::debug!(
            "[ModelMapper] 模型映射: {original} → {mapped} (provider: {})",
            provider.name
        );
        body["model"] = serde_json::json!(mapped);
        return (body, Some(original.clone()), Some(mapped));
    }

    (body, original_model, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_provider() -> Provider {
        Provider {
            id: "test-id".to_string(),
            name: "TestProvider".to_string(),
            settings_config: json!({}),
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            meta: None,
            icon: None,
            icon_color: None,
            in_failover_queue: false,
        }
    }

    #[test]
    fn test_substring_match() {
        let mapping = StandaloneMapping {
            mappings: vec![MappingRule {
                match_pattern: "minimax".to_string(),
                replace: "MiniMax-M2.5-Free".to_string(),
                provider: None,
            }],
            default: None,
        };
        assert_eq!(
            mapping.map_model("MiniMax-Chat", "p", "id"),
            "MiniMax-M2.5-Free"
        );
        assert_eq!(
            mapping.map_model("some-minimax-model", "p", "id"),
            "MiniMax-M2.5-Free"
        );
        assert_eq!(mapping.map_model("gpt-4o", "p", "id"), "gpt-4o");
    }

    #[test]
    fn test_default_model() {
        let mapping = StandaloneMapping {
            mappings: vec![],
            default: Some("default-model".to_string()),
        };
        assert_eq!(mapping.map_model("anything", "p", "id"), "default-model");
    }

    #[test]
    fn test_first_match_wins() {
        let mapping = StandaloneMapping {
            mappings: vec![
                MappingRule {
                    match_pattern: "mini".to_string(),
                    replace: "first".to_string(),
                    provider: None,
                },
                MappingRule {
                    match_pattern: "minimax".to_string(),
                    replace: "second".to_string(),
                    provider: None,
                },
            ],
            default: None,
        };
        assert_eq!(mapping.map_model("minimax-m2.5", "p", "id"), "first");
    }

    #[test]
    fn test_provider_filter_match() {
        let mapping = StandaloneMapping {
            mappings: vec![MappingRule {
                match_pattern: "minimax".to_string(),
                replace: "replaced".to_string(),
                provider: Some("opencode".to_string()),
            }],
            default: None,
        };
        // Provider name matches
        assert_eq!(
            mapping.map_model("minimax-m2.5", "opencode", "id1"),
            "replaced"
        );
        // Provider name doesn't match
        assert_eq!(
            mapping.map_model("minimax-m2.5", "other-provider", "id1"),
            "minimax-m2.5"
        );
        // Provider ID matches
        assert_eq!(
            mapping.map_model("minimax-m2.5", "p", "opencode-id"),
            "replaced"
        );
    }

    #[test]
    fn test_provider_filter_no_provider_field() {
        // provider 不设置时，对所有供应商生效
        let mapping = StandaloneMapping {
            mappings: vec![MappingRule {
                match_pattern: "minimax".to_string(),
                replace: "replaced".to_string(),
                provider: None,
            }],
            default: None,
        };
        assert_eq!(
            mapping.map_model("minimax-m2.5", "any-provider", "any-id"),
            "replaced"
        );
    }

    #[test]
    fn test_no_rules_no_default() {
        let mapping = StandaloneMapping {
            mappings: vec![],
            default: None,
        };
        assert_eq!(mapping.map_model("minimax-m2.5", "p", "id"), "minimax-m2.5");
    }

    #[test]
    fn test_no_config_file_passes_through() {
        let provider = create_provider();
        let body = json!({"model": "minimax-m2.5-free"});
        let (result, original, mapped) = apply_model_mapping(body, &provider);
        assert_eq!(result["model"], "minimax-m2.5-free");
        assert_eq!(original, Some("minimax-m2.5-free".to_string()));
        if mapped.is_some() {
            assert_eq!(mapped, Some("minimax-m2.5-free".to_string()));
        }
    }
}
