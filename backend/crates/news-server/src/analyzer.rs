/**
 * AI Article Analyzer - Background task
 *
 * Runs every 10 minutes to analyze articles using ChatWeb.ai
 * Processes articles in parallel for efficiency.
 */

use crate::chatweb::ChatWebClient;
use crate::routes::AppState;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

const ANALYSIS_INTERVAL: Duration = Duration::from_secs(10 * 60); // 10 minutes
const MAX_CONCURRENT_ANALYSES: usize = 10; // Analyze up to 10 articles at once
const BATCH_SIZE: i64 = 50; // Analyze 50 articles per cycle

/// Run the AI analyzer background task
pub async fn run(state: Arc<AppState>) {
    info!("AI Analyzer: Starting background task (interval: 10 minutes)");

    let chatweb_client = ChatWebClient::new();

    loop {
        // Wait for the next interval
        tokio::time::sleep(ANALYSIS_INTERVAL).await;

        // Get analysis statistics
        match state.db.get_analysis_stats() {
            Ok((total, analyzed)) => {
                let remaining = total - analyzed;
                info!(
                    "AI Analyzer: Stats - Total: {}, Analyzed: {}, Remaining: {}",
                    total, analyzed, remaining
                );

                if remaining == 0 {
                    info!("AI Analyzer: No articles to analyze, skipping cycle");
                    continue;
                }
            }
            Err(e) => {
                error!("AI Analyzer: Failed to get stats: {}", e);
                continue;
            }
        }

        // Get articles that need analysis
        let articles = match state.db.get_articles_for_analysis(BATCH_SIZE) {
            Ok(articles) => articles,
            Err(e) => {
                error!("AI Analyzer: Failed to fetch articles: {}", e);
                continue;
            }
        };

        if articles.is_empty() {
            info!("AI Analyzer: No articles found for analysis");
            continue;
        }

        info!(
            "AI Analyzer: Processing {} articles in parallel (max concurrency: {})",
            articles.len(),
            MAX_CONCURRENT_ANALYSES
        );

        // Prepare article data for parallel analysis (owned strings to avoid lifetime issues)
        let article_data: Vec<_> = articles
            .iter()
            .map(|a| {
                (
                    a.title.clone(),
                    a.description.clone().unwrap_or_default(),
                    a.url.clone(),
                )
            })
            .collect();

        // Analyze articles in parallel
        let start = std::time::Instant::now();
        let results = chatweb_client
            .analyze_articles_parallel(article_data, MAX_CONCURRENT_ANALYSES)
            .await;

        let elapsed = start.elapsed();
        info!(
            "AI Analyzer: Completed {} analyses in {:.2}s",
            results.len(),
            elapsed.as_secs_f64()
        );

        // Update database with results
        let mut success_count = 0;
        let mut error_count = 0;

        for (article, result) in articles.iter().zip(results.iter()) {
            match result {
                Ok(analysis) => {
                    match state.db.update_article_analysis(
                        &article.id,
                        &analysis.summary,
                        &analysis.keywords,
                        &analysis.sentiment,
                        analysis.importance_score,
                        &analysis.category,
                    ) {
                        Ok(_) => {
                            success_count += 1;
                            info!(
                                "AI Analyzer: Analyzed article '{}' - sentiment: {}, importance: {:.2}",
                                article.title.chars().take(50).collect::<String>(),
                                analysis.sentiment,
                                analysis.importance_score
                            );
                        }
                        Err(e) => {
                            error_count += 1;
                            error!(
                                "AI Analyzer: Failed to save analysis for '{}': {}",
                                article.title, e
                            );
                        }
                    }
                }
                Err(e) => {
                    error_count += 1;
                    warn!(
                        "AI Analyzer: Analysis failed for '{}': {}",
                        article.title, e
                    );
                }
            }
        }

        info!(
            "AI Analyzer: Cycle complete - Success: {}, Errors: {}, Rate: {:.1}%",
            success_count,
            error_count,
            (success_count as f64 / (success_count + error_count) as f64) * 100.0
        );
    }
}
