mod agents;
mod analyzer;
mod chatweb;
mod claude;
mod db;
mod degradation_agent;
mod enrichment_agent;
mod fetcher;
mod mcp;
mod routes;
mod stripe;
mod tts_cache;

use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{delete, get, post, put};
use axum::Router;
use db::Db;
use news_core::config::DynamicFeed;
use news_core::feeds::FeedsConfig;
use routes::AppState;
use std::sync::Arc;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::services::ServeDir;
use tracing::info;

const FEEDS_TOML: &str = include_str!("../../../feeds.toml");

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "/data/news.db".into());
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "/app/public".into());
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    let elevenlabs_api_key = std::env::var("ELEVENLABS_API_KEY").unwrap_or_default();
    let openai_api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    let cartesia_api_key = std::env::var("CARTESIA_API_KEY").unwrap_or_default();
    let fish_audio_api_key = std::env::var("FISH_AUDIO_API_KEY").unwrap_or_default();
    let aimlapi_key = std::env::var("AIMLAPI_KEY").unwrap_or_default();
    let venice_api_key = std::env::var("VENICE_API_KEY").unwrap_or_default();
    let runpod_api_key = std::env::var("RUNPOD_API_KEY").unwrap_or_default();
    let cosyvoice_endpoint_id = std::env::var("COSYVOICE_ENDPOINT_ID").unwrap_or_default();
    let qwen_tts_endpoint_id = std::env::var("QWEN_TTS_ENDPOINT_ID").unwrap_or_default();
    let qwen_omni_endpoint_id = std::env::var("QWEN_OMNI_ENDPOINT_ID").unwrap_or_default();
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY").unwrap_or_default();
    let stripe_webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET").unwrap_or_default();
    let stripe_price_id = std::env::var("STRIPE_PRICE_ID").unwrap_or_default();
    let admin_secret = std::env::var("ADMIN_SECRET").unwrap_or_default();
    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "https://news.xyz".into());
    let google_client_id = std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default();
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let db = Arc::new(Db::open(&db_path).expect("Failed to open SQLite database"));

    // Seed feeds from feeds.toml if DB is empty
    if db.feed_count().unwrap_or(0) == 0 {
        if let Ok(config) = FeedsConfig::from_toml(FEEDS_TOML) {
            for (i, feed) in config.feeds.iter().enumerate() {
                let dynamic = DynamicFeed {
                    feed_id: format!("seed-{}", i),
                    url: feed.url.clone(),
                    source: feed.source.clone(),
                    category: feed.category.clone(),
                    enabled: true,
                    added_by: Some("seed".into()),
                };
                let _ = db.put_feed(&dynamic);
            }
            info!(count = config.feeds.len(), "Seeded feeds from feeds.toml");
        }
    }

    // Seed default categories if table is empty
    if db.category_count().unwrap_or(0) == 0 {
        let _ = db.seed_default_categories();
    }
    // Ensure all categories are visible (fix for hidden categories)
    let _ = db.ensure_all_categories_visible();

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("NewsAggregator/1.0")
        .build()
        .expect("Failed to build HTTP client");

    let runpod_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(200))
        .build()
        .expect("Failed to build RunPod HTTP client");

    // Spawn background fetcher
    let fetcher_db = Arc::clone(&db);
    let fetcher_client = http_client.clone();
    tokio::spawn(async move {
        fetcher::run(fetcher_db, fetcher_client).await;
    });

    // NOTE: TTS pre-cache task is spawned after state construction (see below)

    let state = Arc::new(AppState {
        db,
        http_client,
        api_key,
        elevenlabs_api_key,
        openai_api_key,
        cartesia_api_key,
        fish_audio_api_key,
        aimlapi_key,
        venice_api_key,
        runpod_api_key,
        runpod_client,
        cosyvoice_endpoint_id,
        qwen_tts_endpoint_id,
        qwen_omni_endpoint_id,
        stripe_secret_key,
        stripe_webhook_secret,
        stripe_price_id,
        admin_secret,
        base_url,
        google_client_id,
    });

    // Spawn TTS pre-cache background task
    tokio::spawn(tts_cache::run(Arc::clone(&state)));

    // Spawn enrichment agent background task
    tokio::spawn(enrichment_agent::run(Arc::clone(&state)));

    // Spawn degradation agent background task
    tokio::spawn(degradation_agent::run(Arc::clone(&state)));

    // Spawn AI analyzer background task (ChatWeb.ai)
    tokio::spawn(analyzer::run(Arc::clone(&state)));

    let api_routes = Router::new()
        .route("/article/:id", get(routes::serve_article_html))
        .route("/api/articles", get(routes::get_articles))
        .route("/api/articles/:id", get(routes::get_article_by_id))
        .route("/api/articles/:id/view", post(routes::handle_article_view))
        .route("/api/articles/:id/click", post(routes::handle_article_click))
        .route("/api/articles/:id/enrichments", get(routes::handle_get_enrichments))
        .route("/api/categories", get(routes::get_categories))
        .route("/api/search", get(routes::handle_search))
        .route("/api/image-proxy", get(routes::handle_image_proxy))
        .route("/health", get(routes::health))
        .route("/api/articles/summarize", post(routes::handle_summarize))
        .route("/api/articles/questions", post(routes::handle_article_questions))
        .route("/api/articles/ask", post(routes::handle_article_ask))
        .route("/api/articles/classify", post(routes::handle_article_classify))
        .route("/api/articles/action-plan", post(routes::handle_action_plan))
        .route("/api/tts/to-reading", post(routes::handle_to_reading))
        .route("/api/tts/voices", get(routes::handle_tts_voices))
        .route("/api/tts", post(routes::handle_tts))
        .route("/api/tts/clone", post(routes::handle_tts_clone))
        .route("/api/podcast/generate", post(routes::handle_podcast_generate))
        .route("/api/murmur/generate", post(routes::handle_murmur_generate))
        .route("/api/feed", get(routes::get_feed))
        .route("/api/admin/feeds", get(routes::list_feeds))
        .route("/api/admin/feeds", post(routes::add_feed))
        .route("/api/admin/feeds/:feed_id", delete(routes::delete_feed))
        .route("/api/admin/feeds/:feed_id", put(routes::update_feed))
        .route("/api/admin/categories", post(routes::handle_categories_manage))
        .route("/api/admin/command", post(routes::handle_command))
        .route("/api/admin/features", post(routes::handle_toggle_feature))
        .route("/api/admin/changes", get(routes::list_changes))
        .route(
            "/api/admin/changes/:id/apply",
            post(routes::apply_change),
        )
        .route(
            "/api/admin/changes/:id/reject",
            post(routes::reject_change),
        )
        // Subscription routes
        .route("/api/subscribe", post(routes::handle_subscribe))
        .route("/api/stripe/webhook", post(routes::handle_stripe_webhook))
        .route("/api/subscription/status", get(routes::handle_subscription_status))
        .route("/api/subscription/portal", post(routes::handle_billing_portal))
        .route("/api/usage", get(routes::handle_usage))
        // Auth routes
        .route("/api/auth/google", post(routes::handle_google_auth))
        .route("/api/auth/konami", post(routes::handle_konami))
        .route("/api/config", get(routes::handle_config))
        // Telemetry (vitals + errors from frontend beacon)
        .route("/api/telemetry", post(routes::handle_telemetry))
        // MCP server endpoint
        .route("/mcp", post(mcp::handle_mcp))
        // SEO: server-side rendered index.html with per-domain OGP meta tags
        .route("/", get(routes::serve_index_html))
        .route("/index.html", get(routes::serve_index_html))
        // SEO: sitemap and robots.txt
        .route("/robots.txt", get(routes::serve_robots_txt))
        .route("/sitemap.xml", get(routes::serve_sitemap_xml))
        .with_state(state);

    // CORS: restrict to known origins (same-origin requests + specific domains)
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            "https://news.xyz".parse::<HeaderValue>().unwrap(),
            "https://news-xyz.fly.dev".parse::<HeaderValue>().unwrap(),
            "http://localhost:8080".parse::<HeaderValue>().unwrap(),
        ]))
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderName::from_static("x-device-id"),
            axum::http::HeaderName::from_static("x-admin-secret"),
        ]);

    let app = api_routes
        .fallback_service(ServeDir::new(&static_dir).append_index_html_on_directories(true))
        .layer(middleware::from_fn(set_cache_headers))
        .layer(ConcurrencyLimitLayer::new(256))
        .layer(CompressionLayer::new())
        .layer(cors)
        // Security headers
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ));

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind");

    info!(port, "Server starting");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

/// Set Cache-Control headers based on URL patterns
async fn set_cache_headers(req: Request, next: Next) -> impl IntoResponse {
    let path = req.uri().path().to_owned();
    let query = req.uri().query().unwrap_or_default().to_owned();
    let mut res = next.run(req).await;
    let cache_value = if query.contains("v=") {
        Some("public, max-age=31536000, immutable")
    } else if path == "/sw.js" {
        Some("no-cache")
    } else if path.ends_with(".html") || path == "/" {
        Some("no-cache")
    } else if path.starts_with("/icons/") {
        Some("public, max-age=604800")
    } else if path.ends_with(".json") || path == "/robots.txt" || path == "/sitemap.xml" {
        Some("public, max-age=3600")
    } else {
        None
    };
    if let Some(val) = cache_value {
        res.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static(val),
        );
    }
    res
}



async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");
    info!("Shutdown signal received");
}
