pub mod image_agent;
pub mod research_agent;
pub mod video_agent;

use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

/// Retry an operation with exponential backoff.
///
/// # Arguments
/// * `operation` - Async function to retry
/// * `max_retries` - Maximum number of retry attempts (default: 3)
/// * `initial_delay_ms` - Initial delay in milliseconds (default: 1000)
///
/// # Returns
/// Result of the operation or the last error encountered
pub async fn retry_with_backoff<F, Fut, T, E>(
    mut operation: F,
    max_retries: u32,
    initial_delay_ms: u64,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempts = 0;
    let mut delay = Duration::from_millis(initial_delay_ms);

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempts += 1;
                if attempts > max_retries {
                    warn!(
                        error = %e,
                        attempts,
                        "Operation failed after {} retries",
                        max_retries
                    );
                    return Err(e);
                }
                warn!(
                    error = %e,
                    attempt = attempts,
                    delay_ms = delay.as_millis(),
                    "Operation failed, retrying..."
                );
                sleep(delay).await;
                delay *= 2; // Exponential backoff
            }
        }
    }
}

/// Check rate limit for a specific feature and device.
///
/// # Arguments
/// * `db` - Database connection
/// * `device_id` - Device identifier
/// * `feature` - Feature name (e.g., "enrichment", "dalle")
/// * `limit` - Daily limit for this feature
///
/// # Returns
/// Ok(current_usage) if under limit, Err(message) if over limit
pub async fn check_rate_limit(
    db: &crate::db::Db,
    device_id: &str,
    feature: &str,
    limit: i64,
) -> Result<i64, String> {
    let usage = db
        .get_usage(device_id, feature)
        .map_err(|e| format!("Failed to get usage: {}", e))?;

    if usage >= limit {
        return Err(format!(
            "Daily limit exceeded for {}: {}/{}",
            feature, usage, limit
        ));
    }

    Ok(usage)
}

/// Increment usage counter for a feature.
pub async fn increment_usage(
    db: &crate::db::Db,
    device_id: &str,
    feature: &str,
) -> Result<i64, String> {
    db.increment_usage(device_id, feature)
}
