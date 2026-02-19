#[cfg(feature = "dynamo")]
use crate::error::{AppError, Result};
#[cfg(feature = "dynamo")]
use aws_sdk_dynamodb::types::AttributeValue;
#[cfg(feature = "dynamo")]
use aws_sdk_dynamodb::Client;
use serde::{Deserialize, Serialize};
#[cfg(feature = "dynamo")]
use std::collections::HashMap;
#[cfg(feature = "dynamo")]
use tracing::info;

/// A feed configuration stored in DynamoDB ConfigTable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicFeed {
    pub feed_id: String,
    pub url: String,
    pub source: String,
    pub category: String,
    pub enabled: bool,
    #[serde(default)]
    pub added_by: Option<String>,
}

/// Feature flags stored in DynamoDB ConfigTable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub grouping_enabled: bool,
    pub grouping_threshold: f64,
    pub ogp_enrichment_enabled: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            grouping_enabled: false,
            grouping_threshold: 0.3,
            ogp_enrichment_enabled: true,
        }
    }
}

/// Combined service configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub feeds: Vec<DynamicFeed>,
    pub features: FeatureFlags,
}

/// DynamoDB client for config operations.
#[cfg(feature = "dynamo")]
#[derive(Clone)]
pub struct ConfigStore {
    client: Client,
    table_name: String,
}

#[cfg(feature = "dynamo")]
const PK_CONFIG: &str = "CONFIG";

#[cfg(feature = "dynamo")]
impl ConfigStore {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    /// Get all enabled feeds from ConfigTable.
    pub async fn get_enabled_feeds(&self) -> Result<Vec<DynamicFeed>> {
        let output = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("pk = :pk AND begins_with(sk, :prefix)")
            .expression_attribute_values(":pk", AttributeValue::S(PK_CONFIG.into()))
            .expression_attribute_values(":prefix", AttributeValue::S("FEEDS#".into()))
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;

        let items = output.items.unwrap_or_default();
        let feeds: Vec<DynamicFeed> = items
            .iter()
            .filter_map(|item| item_to_feed(item))
            .filter(|f| f.enabled)
            .collect();

        Ok(feeds)
    }

    /// Get all feeds (including disabled) from ConfigTable.
    pub async fn get_all_feeds(&self) -> Result<Vec<DynamicFeed>> {
        let output = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("pk = :pk AND begins_with(sk, :prefix)")
            .expression_attribute_values(":pk", AttributeValue::S(PK_CONFIG.into()))
            .expression_attribute_values(":prefix", AttributeValue::S("FEEDS#".into()))
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;

        let items = output.items.unwrap_or_default();
        Ok(items.iter().filter_map(|item| item_to_feed(item)).collect())
    }

    /// Get feature flags from ConfigTable.
    pub async fn get_feature_flags(&self) -> Result<FeatureFlags> {
        let mut flags = FeatureFlags::default();

        let output = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("pk = :pk AND begins_with(sk, :prefix)")
            .expression_attribute_values(":pk", AttributeValue::S(PK_CONFIG.into()))
            .expression_attribute_values(":prefix", AttributeValue::S("FEATURE#".into()))
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;

        for item in output.items.unwrap_or_default() {
            let sk = match item.get("sk").and_then(|v| v.as_s().ok()) {
                Some(s) => s.as_str(),
                None => continue,
            };
            let enabled = item
                .get("enabled")
                .and_then(|v| v.as_bool().ok().copied())
                .unwrap_or(false);

            match sk {
                "FEATURE#grouping" => {
                    flags.grouping_enabled = enabled;
                    if let Some(v) = item
                        .get("similarity_threshold")
                        .and_then(|v| v.as_n().ok())
                        .and_then(|n| n.parse::<f64>().ok())
                    {
                        flags.grouping_threshold = v;
                    }
                }
                "FEATURE#ogp_enrichment" => {
                    flags.ogp_enrichment_enabled = enabled;
                }
                _ => {}
            }
        }

        Ok(flags)
    }

    /// Get full service config.
    pub async fn get_service_config(&self) -> Result<ServiceConfig> {
        let feeds = self.get_all_feeds().await?;
        let features = self.get_feature_flags().await?;
        Ok(ServiceConfig { feeds, features })
    }

    /// Add or update a feed in ConfigTable.
    pub async fn put_feed(&self, feed: &DynamicFeed) -> Result<()> {
        let mut item: HashMap<String, AttributeValue> = HashMap::new();
        item.insert("pk".into(), AttributeValue::S(PK_CONFIG.into()));
        item.insert(
            "sk".into(),
            AttributeValue::S(format!("FEEDS#{}", feed.feed_id)),
        );
        item.insert("feed_id".into(), AttributeValue::S(feed.feed_id.clone()));
        item.insert("url".into(), AttributeValue::S(feed.url.clone()));
        item.insert("source".into(), AttributeValue::S(feed.source.clone()));
        item.insert(
            "category".into(),
            AttributeValue::S(feed.category.clone()),
        );
        item.insert("enabled".into(), AttributeValue::Bool(feed.enabled));
        if let Some(ref added_by) = feed.added_by {
            item.insert("added_by".into(), AttributeValue::S(added_by.clone()));
        }

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;

        info!(feed_id = %feed.feed_id, source = %feed.source, "Feed saved to config");
        Ok(())
    }

    /// Delete a feed from ConfigTable.
    pub async fn delete_feed(&self, feed_id: &str) -> Result<()> {
        self.client
            .delete_item()
            .table_name(&self.table_name)
            .key("pk", AttributeValue::S(PK_CONFIG.into()))
            .key("sk", AttributeValue::S(format!("FEEDS#{}", feed_id)))
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;

        info!(feed_id = %feed_id, "Feed deleted from config");
        Ok(())
    }

    /// Set a feature flag.
    pub async fn set_feature_flag(
        &self,
        feature: &str,
        enabled: bool,
        extra: Option<HashMap<String, AttributeValue>>,
    ) -> Result<()> {
        let mut item: HashMap<String, AttributeValue> = HashMap::new();
        item.insert("pk".into(), AttributeValue::S(PK_CONFIG.into()));
        item.insert(
            "sk".into(),
            AttributeValue::S(format!("FEATURE#{}", feature)),
        );
        item.insert("enabled".into(), AttributeValue::Bool(enabled));

        if let Some(extra_attrs) = extra {
            item.extend(extra_attrs);
        }

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;

        info!(feature = %feature, enabled, "Feature flag updated");
        Ok(())
    }
}

#[cfg(feature = "dynamo")]
fn item_to_feed(item: &HashMap<String, AttributeValue>) -> Option<DynamicFeed> {
    let feed_id = item.get("feed_id")?.as_s().ok()?.clone();
    let url = item.get("url")?.as_s().ok()?.clone();
    let source = item.get("source")?.as_s().ok()?.clone();
    let category = item.get("category")?.as_s().ok()?.clone();
    let enabled = item
        .get("enabled")
        .and_then(|v| v.as_bool().ok().copied())
        .unwrap_or(true);
    let added_by = item
        .get("added_by")
        .and_then(|v| v.as_s().ok().cloned());

    Some(DynamicFeed {
        feed_id,
        url,
        source,
        category,
        enabled,
        added_by,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_feature_flags() {
        let flags = FeatureFlags::default();
        assert!(!flags.grouping_enabled);
        assert!(flags.ogp_enrichment_enabled);
        assert!((flags.grouping_threshold - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn dynamic_feed_serialization() {
        let feed = DynamicFeed {
            feed_id: "test-1".into(),
            url: "https://example.com/feed".into(),
            source: "Example".into(),
            category: "tech".into(),
            enabled: true,
            added_by: Some("admin".into()),
        };
        let json = serde_json::to_string(&feed).unwrap();
        let parsed: DynamicFeed = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.feed_id, "test-1");
        assert_eq!(parsed.source, "Example");
        assert!(parsed.enabled);
    }

    #[test]
    fn service_config_serialization() {
        let config = ServiceConfig {
            feeds: vec![DynamicFeed {
                feed_id: "f1".into(),
                url: "https://example.com/rss".into(),
                source: "Test".into(),
                category: "general".into(),
                enabled: true,
                added_by: None,
            }],
            features: FeatureFlags::default(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"grouping_enabled\":false"));
        assert!(json.contains("\"feed_id\":\"f1\""));
    }
}
