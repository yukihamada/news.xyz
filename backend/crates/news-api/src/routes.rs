use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use news_core::config::ConfigStore;
use news_core::dynamo::ArticleStore;
use news_core::grouping;
use news_core::models::{ArticlesResponse, Category, CategoryInfo};
use serde::Deserialize;

#[derive(Clone)]
pub struct AppState {
    pub article_store: ArticleStore,
    pub config_store: ConfigStore,
}

#[derive(Deserialize)]
pub struct ArticlesQuery {
    pub category: Option<String>,
    pub limit: Option<i32>,
    pub cursor: Option<String>,
}

/// GET /api/articles?category=&limit=&cursor=
pub async fn get_articles(
    State(state): State<AppState>,
    Query(params): Query<ArticlesQuery>,
) -> Response {
    let category = params
        .category
        .as_deref()
        .and_then(Category::from_str);
    let limit = params.limit.unwrap_or(30).min(100).max(1);

    let result = state
        .article_store
        .query_articles(category.as_ref(), limit, params.cursor.as_deref())
        .await;

    match result {
        Ok((mut articles, next_cursor)) => {
            // Apply grouping if feature is enabled
            if let Ok(flags) = state.config_store.get_feature_flags().await {
                if flags.grouping_enabled && articles.len() > 1 {
                    let titles: Vec<&str> =
                        articles.iter().map(|a| a.title.as_str()).collect();
                    let groups =
                        grouping::group_articles(&titles, flags.grouping_threshold);

                    for group in &groups {
                        if group.len() > 1 {
                            let group_id = uuid::Uuid::new_v4().to_string();
                            let count = group.len() as u32;
                            // First article in group is the representative
                            for (i, &idx) in group.iter().enumerate() {
                                articles[idx].group_id = Some(group_id.clone());
                                if i == 0 {
                                    articles[idx].group_count = Some(count);
                                }
                            }
                        }
                    }

                    // Keep only representative articles (first in each group) + ungrouped
                    let keep_indices: std::collections::HashSet<usize> = groups
                        .iter()
                        .flat_map(|g| {
                            if g.len() > 1 {
                                vec![g[0]] // only the representative
                            } else {
                                g.clone()
                            }
                        })
                        .collect();

                    let filtered: Vec<_> = articles
                        .into_iter()
                        .enumerate()
                        .filter(|(i, _)| keep_indices.contains(i))
                        .map(|(_, a)| a)
                        .collect();
                    articles = filtered;
                }
            }

            let body = ArticlesResponse {
                articles,
                next_cursor,
            };
            (
                StatusCode::OK,
                [
                    (header::CACHE_CONTROL, "public, max-age=120"),
                    (header::CONTENT_TYPE, "application/json; charset=utf-8"),
                ],
                Json(body),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to query articles");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal server error"})),
            )
                .into_response()
        }
    }
}

/// GET /api/categories
pub async fn get_categories() -> Response {
    (
        StatusCode::OK,
        [
            (header::CACHE_CONTROL, "public, max-age=3600"),
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
        ],
        Json(CategoryInfo::all()),
    )
        .into_response()
}

/// GET /health
pub async fn health() -> Response {
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}
