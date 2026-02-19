use crate::claude;
use crate::routes::AppState;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use news_core::config::DynamicFeed;
use news_core::models::Category;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

// --- JSON-RPC types ---

#[derive(Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

fn success(id: Value, result: Value) -> JsonRpcResponse {
    JsonRpcResponse { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
}

fn error(id: Value, code: i32, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: None,
        error: Some(JsonRpcError { code, message: message.into(), data: None }),
    }
}

// --- MCP endpoint ---

pub async fn handle_mcp(
    State(state): State<Arc<AppState>>,
    Json(req): Json<JsonRpcRequest>,
) -> Response {
    let id = req.id.clone().unwrap_or(Value::Null);

    info!(method = %req.method, "MCP request");

    let response = match req.method.as_str() {
        "initialize" => handle_initialize(id),
        "tools/list" => handle_tools_list(id),
        "tools/call" => handle_tools_call(id, &req.params, &state).await,
        "resources/list" => handle_resources_list(id),
        "resources/read" => handle_resources_read(id, &req.params, &state).await,
        "ping" => success(id, json!({})),
        _ => error(id, -32601, &format!("Method not found: {}", req.method)),
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        Json(response),
    ).into_response()
}

// --- initialize ---

fn handle_initialize(id: Value) -> JsonRpcResponse {
    success(id, json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {},
            "resources": {}
        },
        "serverInfo": {
            "name": "hypernews",
            "version": "1.0.0"
        }
    }))
}

// --- tools/list ---

fn handle_tools_list(id: Value) -> JsonRpcResponse {
    success(id, json!({
        "tools": [
            {
                "name": "list_articles",
                "description": "Get latest news articles, optionally filtered by category",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "category": { "type": "string", "description": "Category filter: general, tech, business, entertainment, sports, science" },
                        "limit": { "type": "integer", "description": "Number of articles (1-100, default 20)" },
                        "cursor": { "type": "string", "description": "Pagination cursor from previous response" }
                    }
                }
            },
            {
                "name": "search_articles",
                "description": "Search articles by keyword in title or description",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search keyword" },
                        "limit": { "type": "integer", "description": "Max results (default 20)" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "list_feeds",
                "description": "List all registered RSS feeds",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "add_feed",
                "description": "Add a new RSS feed",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "RSS feed URL" },
                        "source": { "type": "string", "description": "Source name (e.g. Reuters)" },
                        "category": { "type": "string", "description": "Category: general, tech, business, entertainment, sports, science" }
                    },
                    "required": ["url", "source", "category"]
                }
            },
            {
                "name": "remove_feed",
                "description": "Remove an RSS feed by its feed_id",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "feed_id": { "type": "string", "description": "Feed ID to remove" }
                    },
                    "required": ["feed_id"]
                }
            },
            {
                "name": "toggle_feed",
                "description": "Enable or disable an RSS feed",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "feed_id": { "type": "string", "description": "Feed ID to toggle" },
                        "enabled": { "type": "boolean", "description": "true to enable, false to disable" }
                    },
                    "required": ["feed_id", "enabled"]
                }
            },
            {
                "name": "list_categories",
                "description": "List all news categories",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "ask_question",
                "description": "Ask an AI question about a news article",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "Article title" },
                        "question": { "type": "string", "description": "Question to ask about the article" },
                        "description": { "type": "string", "description": "Article description/summary" }
                    },
                    "required": ["title", "question"]
                }
            },
            {
                "name": "summarize_news",
                "description": "Generate an AI news summary (1-10 minutes)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "minutes": { "type": "integer", "description": "Summary length in minutes (1-10, default 3)" }
                    }
                }
            },
            {
                "name": "get_settings",
                "description": "Get current server settings (features, feed count)",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "update_settings",
                "description": "Update a feature setting (e.g. grouping, ogp_enrichment)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "feature": { "type": "string", "description": "Feature name: grouping, ogp_enrichment" },
                        "enabled": { "type": "boolean", "description": "Enable or disable" }
                    },
                    "required": ["feature", "enabled"]
                }
            }
        ]
    }))
}

// --- tools/call ---

async fn handle_tools_call(id: Value, params: &Value, state: &AppState) -> JsonRpcResponse {
    let tool_name = params["name"].as_str().unwrap_or("");
    let args = &params["arguments"];

    match tool_name {
        "list_articles" => tool_list_articles(id, args, state),
        "search_articles" => tool_search_articles(id, args, state),
        "list_feeds" => tool_list_feeds(id, state),
        "add_feed" => tool_add_feed(id, args, state),
        "remove_feed" => tool_remove_feed(id, args, state),
        "toggle_feed" => tool_toggle_feed(id, args, state),
        "list_categories" => tool_list_categories(id, state),
        "ask_question" => tool_ask_question(id, args, state).await,
        "summarize_news" => tool_summarize_news(id, args, state).await,
        "get_settings" => tool_get_settings(id, state),
        "update_settings" => tool_update_settings(id, args, state),
        _ => error(id, -32602, &format!("Unknown tool: {}", tool_name)),
    }
}

fn tool_list_articles(id: Value, args: &Value, state: &AppState) -> JsonRpcResponse {
    let category = args["category"].as_str().and_then(Category::from_str);
    let limit = args["limit"].as_i64().unwrap_or(20).min(100).max(1);
    let cursor = args["cursor"].as_str();

    match state.db.query_articles(category.as_ref(), limit, cursor) {
        Ok((articles, next_cursor)) => {
            let items: Vec<Value> = articles.iter().map(|a| json!({
                "id": a.id,
                "title": a.title,
                "source": a.source,
                "category": a.category.as_str(),
                "url": a.url,
                "description": a.description,
                "published_at": a.published_at.to_rfc3339(),
            })).collect();
            success(id, json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({
                    "articles": items,
                    "count": items.len(),
                    "next_cursor": next_cursor,
                })).unwrap_or_default() }]
            }))
        }
        Err(e) => error(id, -32000, &format!("Failed to query articles: {}", e)),
    }
}

fn tool_search_articles(id: Value, args: &Value, state: &AppState) -> JsonRpcResponse {
    let query = args["query"].as_str().unwrap_or("");
    let limit = args["limit"].as_i64().unwrap_or(20).min(100).max(1);

    if query.is_empty() {
        return error(id, -32602, "query is required");
    }

    // Fetch recent articles and filter by keyword
    match state.db.query_articles(None, 200, None) {
        Ok((articles, _)) => {
            let query_lower = query.to_lowercase();
            let matched: Vec<Value> = articles.iter()
                .filter(|a| {
                    a.title.to_lowercase().contains(&query_lower) ||
                    a.description.as_deref().unwrap_or("").to_lowercase().contains(&query_lower)
                })
                .take(limit as usize)
                .map(|a| json!({
                    "id": a.id,
                    "title": a.title,
                    "source": a.source,
                    "category": a.category.as_str(),
                    "url": a.url,
                    "description": a.description,
                    "published_at": a.published_at.to_rfc3339(),
                }))
                .collect();
            success(id, json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({
                    "query": query,
                    "results": matched,
                    "count": matched.len(),
                })).unwrap_or_default() }]
            }))
        }
        Err(e) => error(id, -32000, &format!("Search failed: {}", e)),
    }
}

fn tool_list_feeds(id: Value, state: &AppState) -> JsonRpcResponse {
    match state.db.get_all_feeds() {
        Ok(feeds) => {
            let items: Vec<Value> = feeds.iter().map(|f| json!({
                "feed_id": f.feed_id,
                "url": f.url,
                "source": f.source,
                "category": f.category,
                "enabled": f.enabled,
            })).collect();
            success(id, json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({
                    "feeds": items,
                    "count": items.len(),
                })).unwrap_or_default() }]
            }))
        }
        Err(e) => error(id, -32000, &format!("Failed to list feeds: {}", e)),
    }
}

fn tool_add_feed(id: Value, args: &Value, state: &AppState) -> JsonRpcResponse {
    let url = args["url"].as_str().unwrap_or("");
    let source = args["source"].as_str().unwrap_or("");
    let category = args["category"].as_str().unwrap_or("general");

    if url.is_empty() || source.is_empty() {
        return error(id, -32602, "url and source are required");
    }

    let feed_id = format!("feed-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("x"));
    let feed = DynamicFeed {
        feed_id: feed_id.clone(),
        url: url.to_string(),
        source: source.to_string(),
        category: category.to_string(),
        enabled: true,
        added_by: Some("mcp".into()),
    };

    match state.db.put_feed(&feed) {
        Ok(()) => {
            info!(feed_id = %feed_id, source, "Feed added via MCP");
            success(id, json!({
                "content": [{ "type": "text", "text": format!("Feed added: {} ({}) [{}]", source, feed_id, category) }]
            }))
        }
        Err(e) => error(id, -32000, &format!("Failed to add feed: {}", e)),
    }
}

fn tool_remove_feed(id: Value, args: &Value, state: &AppState) -> JsonRpcResponse {
    let feed_id = args["feed_id"].as_str().unwrap_or("");
    if feed_id.is_empty() {
        return error(id, -32602, "feed_id is required");
    }

    match state.db.delete_feed(feed_id) {
        Ok(()) => {
            info!(feed_id, "Feed removed via MCP");
            success(id, json!({
                "content": [{ "type": "text", "text": format!("Feed removed: {}", feed_id) }]
            }))
        }
        Err(e) => error(id, -32000, &format!("Failed to remove feed: {}", e)),
    }
}

fn tool_toggle_feed(id: Value, args: &Value, state: &AppState) -> JsonRpcResponse {
    let feed_id = args["feed_id"].as_str().unwrap_or("");
    let enabled = args["enabled"].as_bool().unwrap_or(true);

    if feed_id.is_empty() {
        return error(id, -32602, "feed_id is required");
    }

    let feeds = match state.db.get_all_feeds() {
        Ok(f) => f,
        Err(e) => return error(id, -32000, &format!("Failed to get feeds: {}", e)),
    };
    let feed = match feeds.into_iter().find(|f| f.feed_id == feed_id) {
        Some(f) => f,
        None => return error(id, -32602, &format!("Feed not found: {}", feed_id)),
    };
    let updated = DynamicFeed { enabled, ..feed };

    match state.db.put_feed(&updated) {
        Ok(()) => {
            let label = if enabled { "enabled" } else { "disabled" };
            success(id, json!({
                "content": [{ "type": "text", "text": format!("Feed {} {}", feed_id, label) }]
            }))
        }
        Err(e) => error(id, -32000, &format!("Failed to toggle feed: {}", e)),
    }
}

fn tool_list_categories(id: Value, state: &AppState) -> JsonRpcResponse {
    match state.db.get_categories() {
        Ok(cats) => {
            let items: Vec<Value> = cats.iter()
                .filter(|(_, _, _, _, vis)| *vis)
                .map(|(cid, ja, en, order, _)| json!({
                    "id": cid,
                    "label_ja": ja,
                    "label_en": en,
                    "sort_order": order,
                }))
                .collect();
            success(id, json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({
                    "categories": items,
                })).unwrap_or_default() }]
            }))
        }
        Err(_) => {
            let defaults: Vec<Value> = Category::all().iter().map(|c| json!({
                "id": c.as_str(),
            })).collect();
            success(id, json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({
                    "categories": defaults,
                })).unwrap_or_default() }]
            }))
        }
    }
}

async fn tool_ask_question(id: Value, args: &Value, state: &AppState) -> JsonRpcResponse {
    let title = args["title"].as_str().unwrap_or("");
    let question = args["question"].as_str().unwrap_or("");
    let description = args["description"].as_str().unwrap_or("");

    if title.is_empty() || question.is_empty() {
        return error(id, -32602, "title and question are required");
    }

    if state.api_key.is_empty() {
        return error(id, -32000, "Anthropic API key not configured");
    }

    match claude::answer_question(
        &state.http_client,
        &state.api_key,
        title,
        description,
        "",
        question,
        "",
        None,
    ).await {
        Ok(answer) => success(id, json!({
            "content": [{ "type": "text", "text": answer }]
        })),
        Err(e) => error(id, -32000, &format!("AI answer failed: {}", e)),
    }
}

async fn tool_summarize_news(id: Value, args: &Value, state: &AppState) -> JsonRpcResponse {
    let minutes = args["minutes"].as_u64().unwrap_or(3).min(10).max(1) as usize;
    let target_chars = minutes * 300;

    if state.api_key.is_empty() {
        return error(id, -32000, "Anthropic API key not configured");
    }

    let articles = match state.db.query_articles(None, 30, None) {
        Ok((arts, _)) => arts,
        Err(e) => return error(id, -32000, &format!("Failed to query articles: {}", e)),
    };

    if articles.is_empty() {
        return success(id, json!({
            "content": [{ "type": "text", "text": "No articles available to summarize." }]
        }));
    }

    let pairs: Vec<(String, String)> = articles.iter()
        .map(|a| (a.title.clone(), a.source.clone()))
        .collect();

    match claude::summarize_articles(&state.http_client, &state.api_key, &pairs, target_chars).await {
        Ok(summary) => success(id, json!({
            "content": [{ "type": "text", "text": summary }]
        })),
        Err(e) => error(id, -32000, &format!("Summarization failed: {}", e)),
    }
}

fn tool_get_settings(id: Value, state: &AppState) -> JsonRpcResponse {
    match state.db.get_service_config() {
        Ok(config) => success(id, json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&config).unwrap_or_default() }]
        })),
        Err(e) => error(id, -32000, &format!("Failed to get settings: {}", e)),
    }
}

fn tool_update_settings(id: Value, args: &Value, state: &AppState) -> JsonRpcResponse {
    let feature = args["feature"].as_str().unwrap_or("");
    let enabled = args["enabled"].as_bool().unwrap_or(true);

    if feature.is_empty() {
        return error(id, -32602, "feature is required");
    }

    match state.db.set_feature_flag(feature, enabled, None) {
        Ok(()) => {
            let label = if enabled { "enabled" } else { "disabled" };
            info!(feature, enabled, "Feature toggled via MCP");
            success(id, json!({
                "content": [{ "type": "text", "text": format!("Feature '{}' {}", feature, label) }]
            }))
        }
        Err(e) => error(id, -32000, &format!("Failed to update settings: {}", e)),
    }
}

// --- resources/list ---

fn handle_resources_list(id: Value) -> JsonRpcResponse {
    success(id, json!({
        "resources": [
            {
                "uri": "news://articles",
                "name": "Latest Articles",
                "description": "Most recent news articles across all categories",
                "mimeType": "application/json"
            },
            {
                "uri": "news://feeds",
                "name": "Registered Feeds",
                "description": "All registered RSS/Atom feeds",
                "mimeType": "application/json"
            },
            {
                "uri": "news://categories",
                "name": "Categories",
                "description": "News categories",
                "mimeType": "application/json"
            },
            {
                "uri": "news://settings",
                "name": "Settings",
                "description": "Current server settings and feature flags",
                "mimeType": "application/json"
            }
        ]
    }))
}

// --- resources/read ---

async fn handle_resources_read(id: Value, params: &Value, state: &AppState) -> JsonRpcResponse {
    let uri = params["uri"].as_str().unwrap_or("");

    match uri {
        "news://articles" => {
            match state.db.query_articles(None, 30, None) {
                Ok((articles, _)) => {
                    let items: Vec<Value> = articles.iter().map(|a| json!({
                        "id": a.id,
                        "title": a.title,
                        "source": a.source,
                        "category": a.category.as_str(),
                        "url": a.url,
                        "published_at": a.published_at.to_rfc3339(),
                    })).collect();
                    success(id, json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": serde_json::to_string_pretty(&items).unwrap_or_default()
                        }]
                    }))
                }
                Err(e) => error(id, -32000, &format!("Failed to read articles: {}", e)),
            }
        }
        "news://feeds" => {
            match state.db.get_all_feeds() {
                Ok(feeds) => {
                    let items: Vec<Value> = feeds.iter().map(|f| json!({
                        "feed_id": f.feed_id,
                        "url": f.url,
                        "source": f.source,
                        "category": f.category,
                        "enabled": f.enabled,
                    })).collect();
                    success(id, json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": serde_json::to_string_pretty(&items).unwrap_or_default()
                        }]
                    }))
                }
                Err(e) => error(id, -32000, &format!("Failed to read feeds: {}", e)),
            }
        }
        "news://categories" => {
            match state.db.get_categories() {
                Ok(cats) => {
                    let items: Vec<Value> = cats.iter()
                        .filter(|(_, _, _, _, vis)| *vis)
                        .map(|(cid, ja, en, order, _)| json!({
                            "id": cid, "label_ja": ja, "label_en": en, "sort_order": order,
                        }))
                        .collect();
                    success(id, json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": serde_json::to_string_pretty(&items).unwrap_or_default()
                        }]
                    }))
                }
                Err(e) => error(id, -32000, &format!("Failed to read categories: {}", e)),
            }
        }
        "news://settings" => {
            match state.db.get_service_config() {
                Ok(config) => {
                    success(id, json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": serde_json::to_string_pretty(&config).unwrap_or_default()
                        }]
                    }))
                }
                Err(e) => error(id, -32000, &format!("Failed to read settings: {}", e)),
            }
        }
        _ => error(id, -32602, &format!("Unknown resource URI: {}", uri)),
    }
}
