use lambda_runtime::{service_fn, Error, LambdaEvent};
use news_core::config::{ConfigStore, FeatureFlags};
use news_core::dynamo::ArticleStore;
use news_core::feeds::{fetch_all_feeds, FeedConfig, FeedsConfig};
use news_core::ogp;
use serde_json::Value;
use tracing::info;

/// Embedded feeds configuration (compiled into binary) — used as fallback.
const FEEDS_TOML: &str = include_str!("../../../feeds.toml");

/// Load feeds from DynamoDB ConfigTable, falling back to feeds.toml.
async fn load_feeds(config_store: &ConfigStore) -> Vec<FeedConfig> {
    match config_store.get_enabled_feeds().await {
        Ok(feeds) if !feeds.is_empty() => {
            info!(count = feeds.len(), "Loaded feeds from ConfigTable");
            feeds
                .into_iter()
                .map(|f| FeedConfig {
                    url: f.url,
                    source: f.source,
                    category: f.category,
                })
                .collect()
        }
        Ok(_) => {
            info!("ConfigTable empty, falling back to feeds.toml");
            fallback_feeds()
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to read ConfigTable, falling back to feeds.toml");
            fallback_feeds()
        }
    }
}

fn fallback_feeds() -> Vec<FeedConfig> {
    FeedsConfig::from_toml(FEEDS_TOML)
        .map(|c| c.feeds)
        .unwrap_or_default()
}

/// Load feature flags, returning defaults on failure.
async fn load_feature_flags(config_store: &ConfigStore) -> FeatureFlags {
    config_store
        .get_feature_flags()
        .await
        .unwrap_or_default()
}

async fn handler(_event: LambdaEvent<Value>) -> Result<Value, Error> {
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("NewsAggregator/1.0")
        .build()?;

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamo_client = aws_sdk_dynamodb::Client::new(&aws_config);

    let table_name =
        std::env::var("ARTICLES_TABLE").unwrap_or_else(|_| "NewsArticles".to_string());
    let config_table =
        std::env::var("CONFIG_TABLE").unwrap_or_else(|_| "NewsConfig".to_string());

    let store = ArticleStore::new(dynamo_client.clone(), table_name);
    let config_store = ConfigStore::new(dynamo_client, config_table);

    // Load dynamic config
    let feeds = load_feeds(&config_store).await;
    let features = load_feature_flags(&config_store).await;

    let feeds_config = FeedsConfig { feeds };
    let articles = fetch_all_feeds(&http_client, &feeds_config).await;
    info!(total_articles = articles.len(), "Fetched all feeds");

    let inserted = store.put_articles(&articles).await?;

    // OGP enrichment: if enabled, enrich articles without image_url (max 20 per cycle)
    let mut ogp_count = 0;
    if features.ogp_enrichment_enabled {
        let no_image: Vec<_> = articles
            .iter()
            .filter(|a| a.image_url.is_none())
            .take(20)
            .collect();

        for article in &no_image {
            if let Some(img_url) = ogp::fetch_og_image(&http_client, &article.url).await {
                let sk = format!("{}#{}", article.published_at.to_rfc3339(), article.id);
                if store
                    .update_image_url(article.category.as_str(), &sk, &img_url)
                    .await
                    .is_ok()
                {
                    ogp_count += 1;
                }
            }
        }
        info!(ogp_enriched = ogp_count, "OGP enrichment complete");
    }

    info!(inserted, "Fetch cycle complete");

    Ok(serde_json::json!({
        "statusCode": 200,
        "body": format!("Fetched {} articles, inserted {}, OGP enriched {}", articles.len(), inserted, ogp_count)
    }))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    lambda_runtime::run(service_fn(handler)).await
}
