mod claude;
mod handler;

use axum::routing::{get, post};
use axum::Router;
use news_core::changes::ChangeStore;
use news_core::config::ConfigStore;
use handler::AdminState;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), lambda_http::Error> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamo_client = aws_sdk_dynamodb::Client::new(&aws_config);

    let config_table =
        std::env::var("CONFIG_TABLE").unwrap_or_else(|_| "NewsConfig".to_string());

    let config_store = ConfigStore::new(dynamo_client.clone(), config_table.clone());
    let change_store = ChangeStore::new(dynamo_client, config_table);

    // Load Anthropic API key from Secrets Manager
    let secret_id =
        std::env::var("ANTHROPIC_SECRET_ID").unwrap_or_else(|_| "hypernews/anthropic-api-key".to_string());
    let sm_client = aws_sdk_secretsmanager::Client::new(&aws_config);
    let api_key = sm_client
        .get_secret_value()
        .secret_id(&secret_id)
        .send()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to load Anthropic API key");
            lambda_http::Error::from(format!("Secret not found: {}", e))
        })?
        .secret_string()
        .unwrap_or_default()
        .to_string();

    info!("Anthropic API key loaded from Secrets Manager");

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .map_err(|e| lambda_http::Error::from(e.to_string()))?;

    let state = AdminState {
        config_store,
        change_store,
        http_client,
        api_key,
    };

    let app = Router::new()
        .route("/api/admin/command", post(handler::handle_command))
        .route("/api/admin/changes", get(handler::list_changes))
        .route(
            "/api/admin/changes/:id/apply",
            post(handler::apply_change),
        )
        .route(
            "/api/admin/changes/:id/reject",
            post(handler::reject_change),
        )
        .with_state(state);

    lambda_http::run(app).await
}
