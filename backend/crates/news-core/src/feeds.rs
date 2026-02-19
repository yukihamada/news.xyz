use crate::dedup::article_id_from_url;
use crate::error::{AppError, Result};
use crate::models::{Article, Category};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::{info, warn};

/// Feed configuration loaded from feeds.toml.
#[derive(Debug, Deserialize, Clone)]
pub struct FeedConfig {
    pub url: String,
    pub source: String,
    pub category: String,
}

#[derive(Debug, Deserialize)]
pub struct FeedsConfig {
    pub feeds: Vec<FeedConfig>,
}

impl FeedsConfig {
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        toml::from_str(toml_str).map_err(|e| AppError::ConfigError(e.to_string()))
    }
}

/// Fetch and parse a single RSS/Atom feed into articles.
pub async fn fetch_feed(client: &reqwest::Client, feed: &FeedConfig) -> Result<Vec<Article>> {
    let category = Category::from_str(&feed.category)
        .ok_or_else(|| AppError::ConfigError(format!("Unknown category: {}", feed.category)))?;

    info!(url = %feed.url, source = %feed.source, "Fetching feed");

    let response = client.get(&feed.url).send().await?;
    let bytes = response.bytes().await?;

    let parsed =
        feed_rs::parser::parse(&bytes[..]).map_err(|e| AppError::ParseError(e.to_string()))?;

    let now = Utc::now();
    let mut articles = Vec::new();

    for entry in parsed.entries {
        let Some(link) = entry.links.first().map(|l| l.href.clone()) else {
            continue;
        };

        let title = entry
            .title
            .map(|t| t.content)
            .unwrap_or_else(|| "(no title)".into());

        let published_at: DateTime<Utc> = entry
            .published
            .or(entry.updated)
            .unwrap_or(now);

        let description = entry
            .summary
            .map(|s| s.content)
            .or_else(|| {
                entry
                    .content
                    .and_then(|c| c.body)
            });

        // Try to extract image from media content or content
        let image_url = entry
            .media
            .first()
            .and_then(|m| m.content.first())
            .and_then(|c| c.url.as_ref())
            .map(|u| u.to_string());

        let id = article_id_from_url(&link);

        articles.push(Article {
            id,
            category: category.clone(),
            title,
            url: link,
            description,
            image_url,
            source: feed.source.clone(),
            published_at,
            fetched_at: now,
            group_id: None,
            group_count: None,
        });
    }

    info!(
        url = %feed.url,
        count = articles.len(),
        "Parsed feed"
    );

    Ok(articles)
}

/// Fetch all configured feeds concurrently.
pub async fn fetch_all_feeds(client: &reqwest::Client, config: &FeedsConfig) -> Vec<Article> {
    let futures: Vec<_> = config
        .feeds
        .iter()
        .map(|feed| fetch_feed(client, feed))
        .collect();

    let results = futures::future::join_all(futures).await;
    let mut all_articles = Vec::new();

    for result in results {
        match result {
            Ok(articles) => all_articles.extend(articles),
            Err(e) => warn!(error = %e, "Failed to fetch feed, skipping"),
        }
    }

    all_articles
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TOML: &str = r#"
[[feeds]]
url = "https://hnrss.org/frontpage"
source = "Hacker News"
category = "tech"

[[feeds]]
url = "https://www3.nhk.or.jp/rss/news/cat0.xml"
source = "NHK"
category = "general"
"#;

    #[test]
    fn parse_feeds_config() {
        let config = FeedsConfig::from_toml(SAMPLE_TOML).unwrap();
        assert_eq!(config.feeds.len(), 2);
        assert_eq!(config.feeds[0].source, "Hacker News");
        assert_eq!(config.feeds[1].category, "general");
    }

    #[test]
    fn invalid_toml_returns_error() {
        let result = FeedsConfig::from_toml("not valid toml {{{}}}");
        assert!(result.is_err());
    }
}
