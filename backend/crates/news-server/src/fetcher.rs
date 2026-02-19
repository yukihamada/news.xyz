use crate::db::Db;
use chrono::{Duration, Utc};
use news_core::feeds::{fetch_all_feeds, FeedConfig, FeedsConfig};
use news_core::ogp;
use std::sync::Arc;
use tracing::{info, warn};

const FEEDS_TOML: &str = include_str!("../../../feeds.toml");

fn fallback_feeds() -> Vec<FeedConfig> {
    FeedsConfig::from_toml(FEEDS_TOML)
        .map(|c| c.feeds)
        .unwrap_or_default()
}

fn load_feeds(db: &Db) -> Vec<FeedConfig> {
    match db.get_enabled_feeds() {
        Ok(feeds) if !feeds.is_empty() => {
            info!(count = feeds.len(), "Loaded feeds from DB");
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
            info!("DB feeds empty, using fallback feeds.toml");
            fallback_feeds()
        }
        Err(e) => {
            warn!(error = %e, "Failed to read feeds from DB, using fallback");
            fallback_feeds()
        }
    }
}

pub async fn run(db: Arc<Db>, http_client: reqwest::Client) {
    let mut fetch_interval = tokio::time::interval(std::time::Duration::from_secs(600));
    let mut cleanup_interval = tokio::time::interval(std::time::Duration::from_secs(86400));

    // Tick once immediately for cleanup (first tick is instant)
    cleanup_interval.tick().await;

    loop {
        tokio::select! {
            _ = fetch_interval.tick() => {
                fetch_cycle(&db, &http_client).await;
            }
            _ = cleanup_interval.tick() => {
                let cutoff = Utc::now() - Duration::days(7);
                match db.delete_old_articles(&cutoff) {
                    Ok(n) => info!(deleted = n, "Old articles cleaned up"),
                    Err(e) => warn!(error = %e, "Failed to clean old articles"),
                }
                match db.cleanup_old_usage(7) {
                    Ok(n) if n > 0 => info!(deleted = n, "Old usage records cleaned up"),
                    Err(e) => warn!(error = %e, "Failed to clean old usage"),
                    _ => {}
                }
                match db.cleanup_expired_cache() {
                    Ok(n) if n > 0 => info!(deleted = n, "Expired cache entries cleaned up"),
                    Err(e) => warn!(error = %e, "Failed to clean expired cache"),
                    _ => {}
                }
            }
        }
    }
}

async fn fetch_cycle(db: &Db, http_client: &reqwest::Client) {
    let feeds = load_feeds(db);

    let feeds_config = FeedsConfig { feeds };
    let articles = fetch_all_feeds(http_client, &feeds_config).await;
    info!(total_articles = articles.len(), "Fetched all feeds");

    match db.insert_articles(&articles) {
        Ok(inserted) => info!(inserted, "Articles stored"),
        Err(e) => warn!(error = %e, "Failed to store articles"),
    }

    // OGP enrichment — always run to ensure articles have images
    let no_image = match db.articles_without_image(50) {
        Ok(a) => a,
        Err(_) => return,
    };
    if !no_image.is_empty() {
        let mut ogp_count = 0;
        for article in &no_image {
            if let Some(img_url) = ogp::fetch_og_image(http_client, &article.url).await {
                if db.update_image_url(&article.id, &img_url).is_ok() {
                    ogp_count += 1;
                }
            }
        }
        if ogp_count > 0 {
            info!(ogp_enriched = ogp_count, total_checked = no_image.len(), "OGP enrichment complete");
        }
    }
}
