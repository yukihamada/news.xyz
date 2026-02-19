use crate::claude;
use aws_sdk_dynamodb::types::AttributeValue;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use news_core::changes::{AdminAction, ChangeRequest, ChangeStatus, ChangeStore};
use news_core::config::{ConfigStore, DynamicFeed};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{info, warn};

#[derive(Clone)]
pub struct AdminState {
    pub config_store: ConfigStore,
    pub change_store: ChangeStore,
    pub http_client: reqwest::Client,
    pub api_key: String,
}

#[derive(Deserialize)]
pub struct CommandRequest {
    pub command: String,
}

/// POST /api/admin/command — Accept a natural language command.
pub async fn handle_command(
    State(state): State<AdminState>,
    Json(body): Json<CommandRequest>,
) -> Response {
    let command = body.command.trim();
    if command.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Empty command"})),
        )
            .into_response();
    }

    // Get current service config for context
    let current_config = match state.config_store.get_service_config().await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Failed to load service config");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to load config"})),
            )
                .into_response();
        }
    };

    // Send to Claude API for interpretation
    let interpretation = match claude::interpret_command(
        &state.http_client,
        &state.api_key,
        command,
        &current_config,
    )
    .await
    {
        Ok(i) => i,
        Err(e) => {
            warn!(error = %e, "Claude API interpretation failed");
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "type": "error",
                    "message": format!("コマンドの解釈に失敗しました: {}", e)
                })),
            )
                .into_response();
        }
    };

    // Low confidence: return explanation only
    if interpretation.confidence < 0.7 || interpretation.actions.is_empty() {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "type": "info",
                "message": interpretation.interpretation,
                "confidence": interpretation.confidence
            })),
        )
            .into_response();
    }

    // Create a change request
    let change_id = uuid::Uuid::new_v4().to_string();
    let change = ChangeRequest {
        change_id: change_id.clone(),
        status: ChangeStatus::Preview,
        command_text: command.to_string(),
        interpretation: interpretation.interpretation.clone(),
        actions: interpretation.actions,
        preview_config: Some(current_config),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    if let Err(e) = state.change_store.create_change(&change).await {
        warn!(error = %e, "Failed to save change request");
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "type": "preview",
            "change_id": change_id,
            "interpretation": interpretation.interpretation,
            "confidence": interpretation.confidence,
            "actions": change.actions
        })),
    )
        .into_response()
}

/// GET /api/admin/changes — List recent change requests.
pub async fn list_changes(State(state): State<AdminState>) -> Response {
    match state.change_store.list_changes(20).await {
        Ok(changes) => (StatusCode::OK, Json(serde_json::json!({"changes": changes}))).into_response(),
        Err(e) => {
            warn!(error = %e, "Failed to list changes");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to list changes"})),
            )
                .into_response()
        }
    }
}

/// POST /api/admin/changes/{id}/apply — Apply a change request.
pub async fn apply_change(
    State(state): State<AdminState>,
    Path(change_id): Path<String>,
) -> Response {
    let change = match state.change_store.get_change(&change_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Change not found"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    if change.status != ChangeStatus::Preview {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Change is not in preview status"})),
        )
            .into_response();
    }

    // Apply each action
    let mut applied = 0;
    let mut errors = Vec::new();

    for action in &change.actions {
        match apply_action(&state.config_store, action).await {
            Ok(()) => applied += 1,
            Err(e) => errors.push(format!("{:?}: {}", action, e)),
        }
    }

    // Update change status
    let _ = state
        .change_store
        .update_status(&change_id, ChangeStatus::Applied)
        .await;

    info!(change_id = %change_id, applied, errors = errors.len(), "Change applied");

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "applied",
            "applied": applied,
            "errors": errors
        })),
    )
        .into_response()
}

/// POST /api/admin/changes/{id}/reject — Reject a change request.
pub async fn reject_change(
    State(state): State<AdminState>,
    Path(change_id): Path<String>,
) -> Response {
    match state
        .change_store
        .update_status(&change_id, ChangeStatus::Rejected)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "rejected"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Apply a single admin action to the config store.
async fn apply_action(config_store: &ConfigStore, action: &AdminAction) -> Result<(), String> {
    match action {
        AdminAction::AddFeed {
            url,
            source,
            category,
        } => {
            let feed_id = format!(
                "feed-{}",
                uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("x")
            );
            let feed = DynamicFeed {
                feed_id,
                url: url.clone(),
                source: source.clone(),
                category: category.clone(),
                enabled: true,
                added_by: Some("admin-chat".into()),
            };
            config_store
                .put_feed(&feed)
                .await
                .map_err(|e| e.to_string())
        }
        AdminAction::RemoveFeed { feed_id } => {
            config_store
                .delete_feed(feed_id)
                .await
                .map_err(|e| e.to_string())
        }
        AdminAction::EnableFeed { feed_id } => {
            // Read current feed, update enabled
            update_feed_enabled(config_store, feed_id, true).await
        }
        AdminAction::DisableFeed { feed_id } => {
            update_feed_enabled(config_store, feed_id, false).await
        }
        AdminAction::ToggleFeature { feature, enabled } => {
            config_store
                .set_feature_flag(feature, *enabled, None)
                .await
                .map_err(|e| e.to_string())
        }
        AdminAction::SetGroupingThreshold { threshold } => {
            let mut extra = HashMap::new();
            extra.insert(
                "similarity_threshold".into(),
                AttributeValue::N(threshold.to_string()),
            );
            config_store
                .set_feature_flag("grouping", true, Some(extra))
                .await
                .map_err(|e| e.to_string())
        }
        AdminAction::AddCategory { .. }
        | AdminAction::RemoveCategory { .. }
        | AdminAction::RenameCategory { .. }
        | AdminAction::ReorderCategories { .. } => {
            Err("Category management is not supported in DynamoDB admin. Use the server API instead.".into())
        }
    }
}

async fn update_feed_enabled(
    config_store: &ConfigStore,
    feed_id: &str,
    enabled: bool,
) -> Result<(), String> {
    let feeds = config_store
        .get_all_feeds()
        .await
        .map_err(|e| e.to_string())?;
    let feed = feeds
        .into_iter()
        .find(|f| f.feed_id == feed_id)
        .ok_or_else(|| format!("Feed not found: {}", feed_id))?;

    let updated = DynamicFeed { enabled, ..feed };
    config_store
        .put_feed(&updated)
        .await
        .map_err(|e| e.to_string())
}
