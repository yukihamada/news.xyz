use crate::config::ServiceConfig;
#[cfg(feature = "dynamo")]
use crate::error::{AppError, Result};
#[cfg(feature = "dynamo")]
use aws_sdk_dynamodb::types::AttributeValue;
#[cfg(feature = "dynamo")]
use aws_sdk_dynamodb::Client;
#[cfg(feature = "dynamo")]
use chrono::Utc;
use serde::{Deserialize, Serialize};
#[cfg(feature = "dynamo")]
use std::collections::HashMap;
#[cfg(feature = "dynamo")]
use tracing::info;

/// Status of a change request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChangeStatus {
    Pending,
    Preview,
    Applied,
    Rejected,
}

impl ChangeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Preview => "preview",
            Self::Applied => "applied",
            Self::Rejected => "rejected",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "preview" => Some(Self::Preview),
            "applied" => Some(Self::Applied),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

/// Actions that can be performed on the service configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdminAction {
    AddFeed {
        url: String,
        source: String,
        category: String,
    },
    RemoveFeed {
        feed_id: String,
    },
    EnableFeed {
        feed_id: String,
    },
    DisableFeed {
        feed_id: String,
    },
    ToggleFeature {
        feature: String,
        enabled: bool,
    },
    SetGroupingThreshold {
        threshold: f64,
    },
    AddCategory {
        id: String,
        label_ja: String,
    },
    RemoveCategory {
        id: String,
    },
    RenameCategory {
        id: String,
        label_ja: String,
    },
    ReorderCategories {
        order: Vec<String>,
    },
}

/// A change request from the admin chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRequest {
    pub change_id: String,
    pub status: ChangeStatus,
    pub command_text: String,
    pub interpretation: String,
    pub actions: Vec<AdminAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_config: Option<ServiceConfig>,
    pub created_at: String,
}

/// DynamoDB client for change request operations.
#[cfg(feature = "dynamo")]
#[derive(Clone)]
pub struct ChangeStore {
    client: Client,
    table_name: String,
}

#[cfg(feature = "dynamo")]
impl ChangeStore {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    /// Create a new change request.
    pub async fn create_change(&self, change: &ChangeRequest) -> Result<()> {
        let actions_json =
            serde_json::to_string(&change.actions).map_err(AppError::SerdeError)?;

        let mut item: HashMap<String, AttributeValue> = HashMap::new();
        item.insert(
            "pk".into(),
            AttributeValue::S(format!("CHANGE#{}", change.change_id)),
        );
        item.insert("sk".into(), AttributeValue::S("META".into()));
        item.insert(
            "change_id".into(),
            AttributeValue::S(change.change_id.clone()),
        );
        item.insert(
            "status".into(),
            AttributeValue::S(change.status.as_str().into()),
        );
        item.insert(
            "command_text".into(),
            AttributeValue::S(change.command_text.clone()),
        );
        item.insert(
            "interpretation".into(),
            AttributeValue::S(change.interpretation.clone()),
        );
        item.insert("actions".into(), AttributeValue::S(actions_json));
        item.insert(
            "created_at".into(),
            AttributeValue::S(change.created_at.clone()),
        );

        if let Some(ref preview) = change.preview_config {
            let preview_json =
                serde_json::to_string(preview).map_err(AppError::SerdeError)?;
            item.insert("preview_config".into(), AttributeValue::S(preview_json));
        }

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;

        info!(change_id = %change.change_id, "Change request created");
        Ok(())
    }

    /// Get a change request by ID.
    pub async fn get_change(&self, change_id: &str) -> Result<Option<ChangeRequest>> {
        let output = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("pk", AttributeValue::S(format!("CHANGE#{}", change_id)))
            .key("sk", AttributeValue::S("META".into()))
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;

        let item = match output.item {
            Some(i) => i,
            None => return Ok(None),
        };

        Ok(item_to_change(&item))
    }

    /// Update the status of a change request.
    pub async fn update_status(
        &self,
        change_id: &str,
        status: ChangeStatus,
    ) -> Result<()> {
        self.client
            .update_item()
            .table_name(&self.table_name)
            .key("pk", AttributeValue::S(format!("CHANGE#{}", change_id)))
            .key("sk", AttributeValue::S("META".into()))
            .update_expression("SET #s = :s")
            .expression_attribute_names("#s", "status")
            .expression_attribute_values(":s", AttributeValue::S(status.as_str().into()))
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;

        info!(change_id = %change_id, status = %status.as_str(), "Change status updated");
        Ok(())
    }

    /// List recent change requests (most recent first, up to limit).
    pub async fn list_changes(&self, limit: i32) -> Result<Vec<ChangeRequest>> {
        // Scan for all CHANGE# items — for small scale this is fine
        let output = self
            .client
            .scan()
            .table_name(&self.table_name)
            .filter_expression("begins_with(pk, :prefix) AND sk = :meta")
            .expression_attribute_values(":prefix", AttributeValue::S("CHANGE#".into()))
            .expression_attribute_values(":meta", AttributeValue::S("META".into()))
            .limit(limit)
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;

        let items = output.items.unwrap_or_default();
        let mut changes: Vec<ChangeRequest> =
            items.iter().filter_map(item_to_change).collect();

        // Sort by created_at descending
        changes.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(changes)
    }
}

#[cfg(feature = "dynamo")]
fn item_to_change(item: &HashMap<String, AttributeValue>) -> Option<ChangeRequest> {
    let change_id = item.get("change_id")?.as_s().ok()?.clone();
    let status_str = item.get("status")?.as_s().ok()?;
    let status = ChangeStatus::from_str(status_str)?;
    let command_text = item.get("command_text")?.as_s().ok()?.clone();
    let interpretation = item
        .get("interpretation")
        .and_then(|v| v.as_s().ok().cloned())
        .unwrap_or_default();
    let actions_json = item.get("actions")?.as_s().ok()?;
    let actions: Vec<AdminAction> = serde_json::from_str(actions_json).ok()?;
    let created_at = item
        .get("created_at")
        .and_then(|v| v.as_s().ok().cloned())
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let preview_config = item
        .get("preview_config")
        .and_then(|v| v.as_s().ok())
        .and_then(|s| serde_json::from_str(s).ok());

    Some(ChangeRequest {
        change_id,
        status,
        command_text,
        interpretation,
        actions,
        preview_config,
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_status_roundtrip() {
        for status in [
            ChangeStatus::Pending,
            ChangeStatus::Preview,
            ChangeStatus::Applied,
            ChangeStatus::Rejected,
        ] {
            let s = status.as_str();
            let parsed = ChangeStatus::from_str(s).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn admin_action_serialization() {
        let action = AdminAction::AddFeed {
            url: "https://example.com/feed".into(),
            source: "Example".into(),
            category: "tech".into(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"type\":\"add_feed\""));
        let parsed: AdminAction = serde_json::from_str(&json).unwrap();
        match parsed {
            AdminAction::AddFeed { url, source, category } => {
                assert_eq!(url, "https://example.com/feed");
                assert_eq!(source, "Example");
                assert_eq!(category, "tech");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn change_request_serialization() {
        let change = ChangeRequest {
            change_id: "test-123".into(),
            status: ChangeStatus::Preview,
            command_text: "NHK以外を増やして".into(),
            interpretation: "Adding non-NHK feeds".into(),
            actions: vec![AdminAction::AddFeed {
                url: "https://rss.itmedia.co.jp/rss/2.0/itmedia_all.xml".into(),
                source: "ITmedia".into(),
                category: "tech".into(),
            }],
            preview_config: None,
            created_at: "2025-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&change).unwrap();
        let parsed: ChangeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.change_id, "test-123");
        assert_eq!(parsed.status, ChangeStatus::Preview);
        assert_eq!(parsed.actions.len(), 1);
    }
}
