use crate::routes::AppState;
use news_core::models::Article;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use super::retry_with_backoff;

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoSearchResult {
    pub video_id: String,
    pub title: String,
    pub description: String,
    pub thumbnail_url: String,
    pub channel_title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoEnrichmentData {
    pub videos: Vec<VideoSearchResult>,
    pub search_query: String,
    pub provider: String,
}

#[derive(Debug, Deserialize)]
struct YouTubeSearchResponse {
    items: Vec<YouTubeSearchItem>,
}

#[derive(Debug, Deserialize)]
struct YouTubeSearchItem {
    id: YouTubeVideoId,
    snippet: YouTubeSnippet,
}

#[derive(Debug, Deserialize)]
struct YouTubeVideoId {
    #[serde(rename = "videoId")]
    video_id: String,
}

#[derive(Debug, Deserialize)]
struct YouTubeSnippet {
    title: String,
    description: String,
    #[serde(rename = "channelTitle")]
    channel_title: String,
    thumbnails: YouTubeThumbnails,
}

#[derive(Debug, Deserialize)]
struct YouTubeThumbnails {
    high: Option<YouTubeThumbnail>,
    medium: Option<YouTubeThumbnail>,
    default: Option<YouTubeThumbnail>,
}

#[derive(Debug, Deserialize)]
struct YouTubeThumbnail {
    url: String,
}

/// Search YouTube for videos related to an article.
///
/// # Arguments
/// * `state` - Application state with API keys
/// * `article` - Article to search videos for
///
/// # Returns
/// Result with enrichment data or error message
pub async fn search_videos(
    state: &Arc<AppState>,
    article: &Article,
) -> Result<VideoEnrichmentData, String> {
    let youtube_api_key = std::env::var("YOUTUBE_API_KEY")
        .map_err(|_| "YOUTUBE_API_KEY environment variable not set".to_string())?;

    if youtube_api_key.is_empty() {
        return Err("YouTube API key is empty".to_string());
    }

    // Create search query from article title
    let search_query = create_search_query(article);

    info!(
        article_id = %article.id,
        query = %search_query,
        "Searching YouTube for related videos"
    );

    // Check cache first
    let cache_key = format!("youtube_search:{}", search_query);
    if let Ok(Some(cached)) = state.db.get_cache(&cache_key) {
        if let Ok(data) = serde_json::from_str::<VideoEnrichmentData>(&cached) {
            info!(
                article_id = %article.id,
                "Using cached YouTube search results"
            );
            return Ok(data);
        }
    }

    // Retry with exponential backoff
    let videos = retry_with_backoff(
        || search_youtube(&state.http_client, &youtube_api_key, &search_query),
        3,
        1000,
    )
    .await?;

    let enrichment_data = VideoEnrichmentData {
        videos,
        search_query: search_query.clone(),
        provider: "youtube".to_string(),
    };

    // Cache for 24 hours (86400 seconds)
    if let Ok(json) = serde_json::to_string(&enrichment_data) {
        state.db.set_cache(&cache_key, "youtube_search", &json, 86400).ok();
    }

    Ok(enrichment_data)
}

/// Create an effective search query from article data.
fn create_search_query(article: &Article) -> String {
    // Use title, limit to 100 characters for better YouTube search results
    article.title.chars().take(100).collect::<String>()
}

/// Call YouTube Data API v3 to search for videos.
async fn search_youtube(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Result<Vec<VideoSearchResult>, String> {
    let url = "https://www.googleapis.com/youtube/v3/search";

    let response = client
        .get(url)
        .query(&[
            ("part", "snippet"),
            ("q", query),
            ("type", "video"),
            ("maxResults", "3"),
            ("key", api_key),
            ("videoEmbeddable", "true"),
            ("videoSyndicated", "true"),
        ])
        .send()
        .await
        .map_err(|e| format!("YouTube API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("YouTube API error {}: {}", status, error_text));
    }

    let youtube_response: YouTubeSearchResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse YouTube response: {}", e))?;

    let videos = youtube_response
        .items
        .into_iter()
        .map(|item| VideoSearchResult {
            video_id: item.id.video_id,
            title: item.snippet.title,
            description: item.snippet.description,
            thumbnail_url: item
                .snippet
                .thumbnails
                .high
                .or(item.snippet.thumbnails.medium)
                .or(item.snippet.thumbnails.default)
                .map(|t| t.url)
                .unwrap_or_default(),
            channel_title: item.snippet.channel_title,
        })
        .collect();

    Ok(videos)
}

/// Process video enrichment for an article.
pub async fn run(state: &Arc<AppState>, article: &Article) -> Result<(), String> {
    let enrichment_id = Uuid::new_v4().to_string();

    // Create pending enrichment record
    state
        .db
        .create_enrichment(
            &enrichment_id,
            &article.id,
            "video",
            "youtube_videos",
            "{}",
        )
        .map_err(|e| format!("Failed to create enrichment: {}", e))?;

    // Search videos
    match search_videos(state, article).await {
        Ok(data) => {
            let data_json = serde_json::to_string(&data)
                .map_err(|e| format!("Failed to serialize enrichment: {}", e))?;

            state
                .db
                .update_enrichment(&enrichment_id, "completed", Some(&data_json), None)
                .map_err(|e| format!("Failed to update enrichment: {}", e))?;

            info!(
                article_id = %article.id,
                enrichment_id = %enrichment_id,
                video_count = data.videos.len(),
                "Video enrichment completed"
            );

            Ok(())
        }
        Err(e) => {
            warn!(
                article_id = %article.id,
                enrichment_id = %enrichment_id,
                error = %e,
                "Video enrichment failed"
            );

            state
                .db
                .update_enrichment(&enrichment_id, "failed", None, Some(&e))
                .ok();

            Err(e)
        }
    }
}
