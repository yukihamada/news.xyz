use crate::claude;
use crate::routes::{cache_key, tts_generate, AppState};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

const DEFAULT_VOICE: &str = "qwen-tts:Japanese";
const ARTICLES_PER_CATEGORY: i64 = 5;
const INTER_REQUEST_DELAY: Duration = Duration::from_secs(2);
const AUDIO_TTL: i64 = 86400; // 24h
const CYCLE_INTERVAL: Duration = Duration::from_secs(900); // 15 min
const INITIAL_DELAY: Duration = Duration::from_secs(60); // 1 min warmup
const TTS_TIMEOUT: Duration = Duration::from_secs(180); // 3 min (RunPod cold start can be slow)

pub async fn run(state: Arc<AppState>) {
    // Short warmup delay, then run first cycle quickly
    tokio::time::sleep(INITIAL_DELAY).await;

    loop {
        // Send a warmup request to wake RunPod GPU before the main cycle
        warmup_runpod(&state).await;

        if let Err(e) = run_cycle(&state).await {
            warn!(error = %e, "TTS pre-generation cycle failed");
        }
        tokio::time::sleep(CYCLE_INTERVAL).await;
    }
}

/// Send a tiny TTS request to wake RunPod GPU, then wait for it to complete or timeout.
async fn warmup_runpod(state: &AppState) {
    if state.runpod_api_key.is_empty() || state.qwen_tts_endpoint_id.is_empty() {
        return;
    }
    info!("TTS pre-cache: sending warmup request to RunPod");
    match tokio::time::timeout(
        TTS_TIMEOUT,
        tts_generate(state, DEFAULT_VOICE, "ウォームアップ"),
    )
    .await
    {
        Ok(Ok(_)) => info!("TTS pre-cache: RunPod warmup succeeded"),
        Ok(Err(e)) => warn!(error = %e, "TTS pre-cache: RunPod warmup failed"),
        Err(_) => warn!("TTS pre-cache: RunPod warmup timed out ({}s)", TTS_TIMEOUT.as_secs()),
    }
}

async fn run_cycle(state: &AppState) -> Result<(), String> {
    // Check that RunPod TTS is configured
    if state.runpod_api_key.is_empty() || state.qwen_tts_endpoint_id.is_empty() {
        info!("TTS pre-cache skipped: RunPod TTS not configured");
        return Ok(());
    }

    let articles = state.db.top_articles_per_category(ARTICLES_PER_CATEGORY)?;
    if articles.is_empty() {
        info!("TTS pre-cache skipped: no articles found");
        return Ok(());
    }

    let mut generated = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;

    for article in &articles {
        let desc = article.description.as_deref().unwrap_or("");
        let raw_text = format!("{}。{}", article.title.trim(), desc.trim());
        // Truncate to 5000 chars (same limit as handle_tts)
        let raw_text = if raw_text.len() > 5000 {
            &raw_text[..5000]
        } else {
            &raw_text
        };

        // Check audio cache
        let audio_ckey = cache_key("tts_audio", &format!("{}|{}", DEFAULT_VOICE, raw_text));
        if let Ok(Some(_)) = state.db.get_cache(&audio_ckey) {
            skipped += 1;
            continue;
        }

        // Get or create reading conversion (qwen-tts engine for pre-cache)
        let reading_ckey = cache_key("to_reading", &format!("qwen-tts|{}", raw_text));
        let text = if let Ok(Some(cached_reading)) = state.db.get_cache(&reading_ckey) {
            cached_reading
        } else if !state.api_key.is_empty() {
            match claude::convert_to_reading(&state.http_client, &state.api_key, raw_text, "qwen-tts").await {
                Ok(reading) => {
                    let _ = state.db.set_cache(&reading_ckey, "to_reading", &reading, AUDIO_TTL);
                    reading
                }
                Err(e) => {
                    warn!(article_id = %article.id, error = %e, "TTS pre-cache: reading conversion failed, using raw text");
                    raw_text.to_string()
                }
            }
        } else {
            raw_text.to_string()
        };

        // Generate TTS audio with extended timeout for cold start
        match tokio::time::timeout(
            TTS_TIMEOUT,
            tts_generate(state, DEFAULT_VOICE, &text),
        )
        .await
        {
            Ok(Ok(bytes)) => {
                let b64 = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &bytes,
                );
                let _ = state.db.set_cache(&audio_ckey, "tts_audio", &b64, AUDIO_TTL);
                generated += 1;
                info!(article_id = %article.id, "TTS pre-cache: generated audio");
            }
            Ok(Err(e)) => {
                warn!(article_id = %article.id, error = %e, "TTS pre-cache: generation failed");
                failed += 1;
            }
            Err(_) => {
                warn!(article_id = %article.id, "TTS pre-cache: generation timed out ({}s)", TTS_TIMEOUT.as_secs());
                failed += 1;
            }
        }

        // Delay between requests to avoid overloading RunPod
        tokio::time::sleep(INTER_REQUEST_DELAY).await;
    }

    info!(
        generated,
        skipped,
        failed,
        total = articles.len(),
        "TTS pre-generation cycle complete"
    );
    Ok(())
}
