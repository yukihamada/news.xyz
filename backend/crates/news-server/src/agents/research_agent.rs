use crate::routes::AppState;
use news_core::models::Article;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use super::retry_with_backoff;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResearchEnrichmentData {
    pub summary: String,
    pub background: String,
    pub key_points: Vec<String>,
    pub related_articles: Vec<RelatedArticle>,
    pub visualization: Option<serde_json::Value>,
    pub provider: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelatedArticle {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContentBlock {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BraveSearchResponse {
    web: Option<BraveWebResults>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResults {
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    description: String,
}

/// Perform background research on an article using Claude API.
///
/// # Arguments
/// * `state` - Application state with API keys
/// * `article` - Article to research
///
/// # Returns
/// Result with enrichment data or error message
pub async fn research_article(
    state: &Arc<AppState>,
    article: &Article,
) -> Result<ResearchEnrichmentData, String> {
    if state.api_key.is_empty() {
        return Err("Claude API key not configured".to_string());
    }

    info!(
        article_id = %article.id,
        title = %article.title,
        "Researching article background with Claude"
    );

    // Check cache first
    let cache_key = format!("research:{}", article.id);
    if let Ok(Some(cached)) = state.db.get_cache(&cache_key) {
        if let Ok(data) = serde_json::from_str::<ResearchEnrichmentData>(&cached) {
            info!(
                article_id = %article.id,
                "Using cached research results"
            );
            return Ok(data);
        }
    }

    // Perform Claude research, web search, and visualization in parallel
    let (claude_result, web_search_result, viz_result) = tokio::join!(
        retry_with_backoff(
            || call_claude_research(&state.http_client, &state.api_key, article),
            3,
            1000,
        ),
        search_related_articles(&state.http_client, article),
        generate_visualization(&state.http_client, &state.api_key, article),
    );

    let mut result = claude_result?;

    // Add web search results if available
    if let Ok(articles) = web_search_result {
        result.related_articles = articles;
    }

    // Add visualization if available
    if let Ok(Some(viz)) = viz_result {
        result.visualization = Some(viz);
    }

    // Cache for 24 hours (86400 seconds)
    if let Ok(json) = serde_json::to_string(&result) {
        state.db.set_cache(&cache_key, "claude_research", &json, 86400).ok();
    }

    Ok(result)
}

/// Call Claude API to perform background research.
async fn call_claude_research(
    client: &reqwest::Client,
    api_key: &str,
    article: &Article,
) -> Result<ResearchEnrichmentData, String> {
    let description = article
        .description
        .as_ref()
        .map(|d| d.as_str())
        .unwrap_or("");

    let prompt = format!(
        r#"あなたは専門的なニュースアナリストです。以下のニュース記事について、背景情報と分析を提供してください。

## 記事情報
タイトル: {}
説明: {}
ソース: {}

## 出力形式（必ずこのJSON形式で出力してください。マークダウンやコードブロック不要）

{{"summary":"記事の要点を1-2文で簡潔に","background":"この記事の歴史的背景や関連する文脈を2-3文で説明","key_points":["重要なポイント1","重要なポイント2","重要なポイント3"]}}

ルール:
- summaryは150文字以内
- backgroundは200-300文字
- key_pointsは3-5個、各50文字以内
- 客観的で中立的な分析
- 専門用語は避け、一般読者向けに"#,
        article.title, description, article.source
    );

    let request = ClaudeRequest {
        model: "claude-haiku-4-5-20251001".to_string(), // Use Haiku for cost efficiency
        max_tokens: 1000,
        messages: vec![ClaudeMessage {
            role: "user".to_string(),
            content: prompt,
        }],
    };

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Claude API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Claude API error {}: {}", status, error_text));
    }

    let claude_response: ClaudeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Claude response: {}", e))?;

    let text = claude_response
        .content
        .first()
        .and_then(|block| block.text.as_ref())
        .ok_or_else(|| "No text in Claude response".to_string())?;

    // Parse JSON response
    let parsed: serde_json::Value = serde_json::from_str(text.trim())
        .map_err(|e| format!("Failed to parse research JSON: {} - Response: {}", e, text))?;

    Ok(ResearchEnrichmentData {
        summary: parsed["summary"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        background: parsed["background"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        key_points: parsed["key_points"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        related_articles: Vec::new(), // Will be filled by the caller
        visualization: None, // Will be filled by the caller
        provider: "claude-haiku".to_string(),
    })
}

/// Generate a Vega-Lite visualization spec if article contains numerical data.
async fn generate_visualization(
    client: &reqwest::Client,
    api_key: &str,
    article: &Article,
) -> Result<Option<serde_json::Value>, String> {
    if api_key.is_empty() {
        return Ok(None);
    }

    let description = article
        .description
        .as_ref()
        .map(|d| d.as_str())
        .unwrap_or("");

    // Check if article likely contains numerical data
    let has_numbers = description.chars().any(|c| c.is_numeric());
    if !has_numbers {
        return Ok(None);
    }

    let prompt = format!(
        r#"あなたはデータ可視化の専門家です。以下のニュース記事にデータや数値が含まれている場合、Vega-Lite JSON仕様を生成してください。

## 記事情報
タイトル: {}
説明: {}

## 指示
- 記事に数値データ、統計、比較、推移などが含まれている場合のみVega-Lite仕様を生成
- 数値データがない場合は null を返す
- グラフタイプ: 棒グラフ、折れ線グラフ、円グラフなど適切なものを選択
- 日本語ラベルを使用
- シンプルで読みやすいデザイン

## 出力形式（JSONのみ、マークダウン不要）
数値データがある場合:
{{"$schema":"https://vega.github.io/schema/vega-lite/v5.json","description":"...","data":{{"values":[...]}},"mark":"bar","encoding":{{...}}}}

数値データがない場合:
null"#,
        article.title, description
    );

    let request = ClaudeRequest {
        model: "claude-haiku-4-5-20251001".to_string(),
        max_tokens: 1500,
        messages: vec![ClaudeMessage {
            role: "user".to_string(),
            content: prompt,
        }],
    };

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Claude visualization request failed: {}", e))?;

    if !response.status().is_success() {
        return Ok(None); // Don't fail the whole enrichment if viz fails
    }

    let claude_response: ClaudeResponse = match response.json().await {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };

    let text = match claude_response
        .content
        .first()
        .and_then(|block| block.text.as_ref())
    {
        Some(t) => t,
        None => return Ok(None),
    };

    let trimmed = text.trim();
    if trimmed == "null" || trimmed.is_empty() {
        return Ok(None);
    }

    // Try to parse as Vega-Lite JSON
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(viz) => Ok(Some(viz)),
        Err(_) => Ok(None), // Invalid JSON, skip visualization
    }
}

/// Search for related articles using Brave Search API.
async fn search_related_articles(
    client: &reqwest::Client,
    article: &Article,
) -> Result<Vec<RelatedArticle>, String> {
    let brave_api_key = std::env::var("BRAVE_SEARCH_API_KEY")
        .unwrap_or_default();

    if brave_api_key.is_empty() {
        info!("Brave Search API key not configured, skipping related articles");
        return Ok(Vec::new());
    }

    // Create search query from article title
    let query = article.title.chars().take(100).collect::<String>();

    let response = match client
        .get("https://api.search.brave.com/res/v1/web/search")
        .header("Accept", "application/json")
        .header("X-Subscription-Token", brave_api_key)
        .query(&[
            ("q", query.as_str()),
            ("count", "5"),
            ("text_decorations", "false"),
            ("search_lang", "ja"),
        ])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            info!(error = %e, "Brave Search request failed, skipping");
            return Ok(Vec::new());
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Brave Search API error {}: {}", status, error_text));
    }

    let search_response: BraveSearchResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Brave Search response: {}", e))?;

    let related = search_response
        .web
        .map(|web| {
            web.results
                .into_iter()
                .take(5)
                .map(|result| {
                    // Extract domain from URL for source
                    let source = result
                        .url
                        .split('/')
                        .nth(2)
                        .unwrap_or("unknown")
                        .to_string();

                    RelatedArticle {
                        title: result.title,
                        url: result.url,
                        snippet: result.description,
                        source,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(related)
}

/// Process research enrichment for an article.
pub async fn run(state: &Arc<AppState>, article: &Article) -> Result<(), String> {
    let enrichment_id = Uuid::new_v4().to_string();

    // Create pending enrichment record
    state
        .db
        .create_enrichment(
            &enrichment_id,
            &article.id,
            "research",
            "background_info",
            "{}",
        )
        .map_err(|e| format!("Failed to create enrichment: {}", e))?;

    // Perform research
    match research_article(state, article).await {
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
                "Research enrichment completed"
            );

            Ok(())
        }
        Err(e) => {
            warn!(
                article_id = %article.id,
                enrichment_id = %enrichment_id,
                error = %e,
                "Research enrichment failed"
            );

            state
                .db
                .update_enrichment(&enrichment_id, "failed", None, Some(&e))
                .ok();

            Err(e)
        }
    }
}
