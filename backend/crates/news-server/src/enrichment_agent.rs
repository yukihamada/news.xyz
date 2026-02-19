use crate::agents::{image_agent, research_agent, video_agent};
use crate::routes::AppState;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

/// Main enrichment agent that runs in a loop.
///
/// This agent:
/// 1. Runs every 10 minutes
/// 2. Identifies popular articles (top 10-20% by popularity_score)
/// 3. Marks them for enrichment
/// 4. Spawns parallel tasks to enrich articles
pub async fn run(state: Arc<AppState>) {
    info!("Enrichment agent starting");

    let mut tick = interval(Duration::from_secs(600)); // 10 minutes

    loop {
        tick.tick().await;

        if let Err(e) = run_cycle(&state).await {
            warn!(error = %e, "Enrichment cycle failed");
        }
    }
}

/// Run one enrichment cycle.
async fn run_cycle(state: &Arc<AppState>) -> Result<(), String> {
    info!("Starting enrichment cycle");

    // Step 1: Mark popular articles for enrichment
    mark_popular_articles_for_enrichment(state).await?;

    // Step 2: Get pending enrichment articles
    let pending_articles = state
        .db
        .get_pending_enrichment_articles(20)
        .map_err(|e| format!("Failed to get pending articles: {}", e))?;

    if pending_articles.is_empty() {
        info!("No articles pending enrichment");
        return Ok(());
    }

    info!(count = pending_articles.len(), "Found articles to enrich");

    // Step 3: Process articles with concurrency limit (max 3 at a time)
    let semaphore = Arc::new(Semaphore::new(3));
    let mut tasks = Vec::new();

    for article in pending_articles {
        let state = Arc::clone(state);
        let permit = Arc::clone(&semaphore);

        let task = tokio::spawn(async move {
            let _permit = permit.acquire().await.unwrap();
            process_article(&state, &article).await
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        if let Err(e) = task.await {
            warn!(error = %e, "Enrichment task failed");
        }
    }

    info!("Enrichment cycle completed");
    Ok(())
}

/// Mark popular articles (top 10-20%) for enrichment.
async fn mark_popular_articles_for_enrichment(state: &Arc<AppState>) -> Result<(), String> {
    // Get articles in the 80-90th percentile (top 10-20%)
    let popular_articles = state
        .db
        .get_popular_articles(80.0, 90.0, 100)
        .map_err(|e| format!("Failed to get popular articles: {}", e))?;

    let mut marked = 0;

    for article in &popular_articles {
        // Check if article is already enriched or pending
        let enrichments = state.db.get_enrichments(&article.id).ok();
        let has_enrichments = enrichments.map(|e| !e.is_empty()).unwrap_or(false);

        if !has_enrichments {
            state
                .db
                .update_enrichment_status(&article.id, "pending")
                .ok();
            marked += 1;
        }
    }

    if marked > 0 {
        info!(
            marked,
            total = popular_articles.len(),
            "Marked popular articles for enrichment"
        );
    }

    Ok(())
}

/// Process a single article for enrichment.
async fn process_article(state: &Arc<AppState>, article: &news_core::models::Article) {
    info!(article_id = %article.id, title = %article.title, "Processing article");

    // Update status to processing
    if let Err(e) = state.db.update_enrichment_status(&article.id, "processing") {
        warn!(article_id = %article.id, error = %e, "Failed to update status");
        return;
    }

    // Run all three agents in parallel
    let (image_result, video_result, research_result) = tokio::join!(
        image_agent::run(state, article),
        video_agent::run(state, article),
        research_agent::run(state, article),
    );

    // Log results
    let mut success_count = 0;
    let total_count = 3;

    if image_result.is_ok() {
        success_count += 1;
    } else if let Err(e) = &image_result {
        warn!(article_id = %article.id, error = %e, "Image agent failed");
    }

    if video_result.is_ok() {
        success_count += 1;
    } else if let Err(e) = &video_result {
        warn!(article_id = %article.id, error = %e, "Video agent failed");
    }

    if research_result.is_ok() {
        success_count += 1;
    } else if let Err(e) = &research_result {
        warn!(article_id = %article.id, error = %e, "Research agent failed");
    }

    // Update final status (partial success is ok)
    let final_status = if success_count > 0 {
        "completed"
    } else {
        "failed"
    };

    if let Err(e) = state.db.update_enrichment_status(&article.id, final_status) {
        warn!(
            article_id = %article.id,
            error = %e,
            "Failed to update final status"
        );
    }

    info!(
        article_id = %article.id,
        status = final_status,
        success_count = success_count,
        total_count = total_count,
        "Article processing completed"
    );
}
