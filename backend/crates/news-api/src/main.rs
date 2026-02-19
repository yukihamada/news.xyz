mod routes;

use axum::routing::get;
use axum::Router;
use news_core::config::ConfigStore;
use news_core::dynamo::ArticleStore;
use routes::AppState;

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

    let table_name =
        std::env::var("ARTICLES_TABLE").unwrap_or_else(|_| "NewsArticles".to_string());
    let config_table =
        std::env::var("CONFIG_TABLE").unwrap_or_else(|_| "NewsConfig".to_string());

    let state = AppState {
        article_store: ArticleStore::new(dynamo_client.clone(), table_name),
        config_store: ConfigStore::new(dynamo_client, config_table),
    };

    let app = Router::new()
        .route("/api/articles", get(routes::get_articles))
        .route("/api/categories", get(routes::get_categories))
        .route("/health", get(routes::health))
        .with_state(state);

    lambda_http::run(app).await
}
