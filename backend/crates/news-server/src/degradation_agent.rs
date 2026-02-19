use crate::routes::AppState;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

/// Degradation agent that runs periodically to:
/// 1. Degrade images for old, unpopular articles (1 hour+ old with low popularity)
/// 2. Delete 80% of articles older than 1 day (keeping top 20% by popularity)
pub async fn run(state: Arc<AppState>) {
    info!("Degradation agent starting");

    // Run every hour
    let mut tick = interval(Duration::from_secs(3600));

    loop {
        tick.tick().await;

        if let Err(e) = run_cycle(&state).await {
            warn!(error = %e, "Degradation cycle failed");
        }
    }
}

/// Run one degradation cycle.
async fn run_cycle(state: &Arc<AppState>) -> Result<(), String> {
    info!("Starting degradation cycle");

    // Step 1: Degrade images for old unpopular articles (older than 1 hour)
    match state.db.degrade_old_unpopular_images(1) {
        Ok(degraded) => {
            if degraded > 0 {
                info!(degraded, "Degraded images for old unpopular articles");
            }
        }
        Err(e) => warn!(error = %e, "Failed to degrade images"),
    }

    // Step 2: Delete bottom 80% of articles older than 1 day
    match state.db.cleanup_old_articles_bottom_80(1) {
        Ok(deleted) => {
            if deleted > 0 {
                info!(deleted, "Deleted bottom 80% of articles older than 1 day");
            }
        }
        Err(e) => warn!(error = %e, "Failed to cleanup old articles"),
    }

    info!("Degradation cycle completed");
    Ok(())
}
