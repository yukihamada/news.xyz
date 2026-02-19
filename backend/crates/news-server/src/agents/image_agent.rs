use crate::routes::AppState;
use news_core::models::Article;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use super::retry_with_backoff;

#[derive(Debug, Serialize)]
struct DalleRequest {
    model: String,
    prompt: String,
    n: u32,
    size: String,
    quality: String,
}

#[derive(Debug, Deserialize)]
struct DalleResponse {
    data: Vec<DalleImageData>,
}

#[derive(Debug, Deserialize)]
struct DalleImageData {
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageEnrichmentData {
    pub image_url: String,
    pub prompt: String,
    pub provider: String,
}

#[derive(Debug, Serialize)]
struct ReplicateRequest {
    version: String,
    input: ReplicateInput,
}

#[derive(Debug, Serialize)]
struct ReplicateInput {
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_outputs: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ReplicateResponse {
    id: String,
    status: String,
    output: Option<Vec<String>>,
    error: Option<String>,
}

const FLUX_SCHNELL_VERSION: &str = "f2e15a9d0e1c4e8c91f3d63f5f3b3e3f2e15a9d0e1c4e8c91f3d63f5f3b3e3f"; // Flux Schnell

/// Generate an AI image for an article using DALL-E 3 or Flux (fallback).
///
/// # Arguments
/// * `state` - Application state with API keys
/// * `article` - Article to generate image for
///
/// # Returns
/// Result with enrichment data or error message
pub async fn generate_image(
    state: &Arc<AppState>,
    article: &Article,
) -> Result<ImageEnrichmentData, String> {
    // Create a prompt from the article title and description
    let prompt = create_image_prompt(article);

    // Try DALL-E 3 first if API key is configured
    if !state.openai_api_key.is_empty() {
        info!(
            article_id = %article.id,
            prompt = %prompt,
            "Generating image with DALL-E 3"
        );

        match retry_with_backoff(
            || generate_dalle_image(&state.http_client, &state.openai_api_key, &prompt),
            2,
            1000,
        )
        .await
        {
            Ok(image_url) => {
                return Ok(ImageEnrichmentData {
                    image_url,
                    prompt,
                    provider: "dalle-3".to_string(),
                });
            }
            Err(e) => {
                warn!(
                    article_id = %article.id,
                    error = %e,
                    "DALL-E 3 failed, trying Flux fallback"
                );
            }
        }
    }

    // Fallback to Flux via Replicate
    let replicate_token = std::env::var("REPLICATE_API_TOKEN")
        .unwrap_or_default();

    if replicate_token.is_empty() {
        return Err("Both OpenAI and Replicate API keys are not configured".to_string());
    }

    info!(
        article_id = %article.id,
        prompt = %prompt,
        "Generating image with Flux (Replicate)"
    );

    let result = retry_with_backoff(
        || generate_flux_image(&state.http_client, &replicate_token, &prompt),
        3,
        2000,
    )
    .await?;

    Ok(ImageEnrichmentData {
        image_url: result,
        prompt,
        provider: "flux-schnell".to_string(),
    })
}

/// Create an effective image generation prompt from article data.
fn create_image_prompt(article: &Article) -> String {
    let base_description = article
        .description
        .as_ref()
        .map(|d| {
            if d.len() > 200 {
                &d[..200]
            } else {
                d.as_str()
            }
        })
        .unwrap_or("");

    // Create a concise, descriptive prompt for DALL-E
    format!(
        "Create a professional, modern illustration for a news article titled '{}'. {}. Style: clean, minimal, editorial illustration.",
        article.title.chars().take(100).collect::<String>(),
        base_description
    )
}

/// Call DALL-E 3 API to generate an image.
async fn generate_dalle_image(
    client: &reqwest::Client,
    api_key: &str,
    prompt: &str,
) -> Result<String, String> {
    let request = DalleRequest {
        model: "dall-e-3".to_string(),
        prompt: prompt.to_string(),
        n: 1,
        size: "1024x1024".to_string(),
        quality: "standard".to_string(),
    };

    let response = client
        .post("https://api.openai.com/v1/images/generations")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("DALL-E request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("DALL-E API error {}: {}", status, error_text));
    }

    let dalle_response: DalleResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse DALL-E response: {}", e))?;

    dalle_response
        .data
        .first()
        .map(|img| img.url.clone())
        .ok_or_else(|| "No image URL in DALL-E response".to_string())
}

/// Call Flux API via Replicate to generate an image.
async fn generate_flux_image(
    client: &reqwest::Client,
    api_token: &str,
    prompt: &str,
) -> Result<String, String> {
    // Create prediction
    let request = ReplicateRequest {
        version: "black-forest-labs/flux-schnell".to_string(), // Using model name instead
        input: ReplicateInput {
            prompt: prompt.to_string(),
            width: Some(1024),
            height: Some(1024),
            num_outputs: Some(1),
        },
    };

    let create_response = client
        .post("https://api.replicate.com/v1/predictions")
        .header("Authorization", format!("Token {}", api_token))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Replicate request failed: {}", e))?;

    if !create_response.status().is_success() {
        let status = create_response.status();
        let error_text = create_response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Replicate API error {}: {}", status, error_text));
    }

    let mut prediction: ReplicateResponse = create_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Replicate response: {}", e))?;

    // Poll for completion (max 60 seconds)
    for _ in 0..30 {
        if prediction.status == "succeeded" {
            return prediction
                .output
                .and_then(|output| output.first().cloned())
                .ok_or_else(|| "No image URL in Replicate response".to_string());
        }

        if prediction.status == "failed" || prediction.status == "canceled" {
            return Err(format!(
                "Replicate prediction failed: {}",
                prediction.error.unwrap_or_else(|| "Unknown error".to_string())
            ));
        }

        // Wait 2 seconds before polling again
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Get prediction status
        let get_response = client
            .get(&format!(
                "https://api.replicate.com/v1/predictions/{}",
                prediction.id
            ))
            .header("Authorization", format!("Token {}", api_token))
            .send()
            .await
            .map_err(|e| format!("Replicate polling failed: {}", e))?;

        if !get_response.status().is_success() {
            return Err(format!("Replicate polling error: {}", get_response.status()));
        }

        prediction = get_response
            .json()
            .await
            .map_err(|e| format!("Failed to parse poll response: {}", e))?;
    }

    Err("Replicate prediction timed out after 60 seconds".to_string())
}

/// Process image enrichment for an article.
pub async fn run(state: &Arc<AppState>, article: &Article) -> Result<(), String> {
    let enrichment_id = Uuid::new_v4().to_string();

    // Create pending enrichment record
    state
        .db
        .create_enrichment(
            &enrichment_id,
            &article.id,
            "image",
            "ai_image",
            "{}",
        )
        .map_err(|e| format!("Failed to create enrichment: {}", e))?;

    // Generate image
    match generate_image(state, article).await {
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
                "Image enrichment completed"
            );

            Ok(())
        }
        Err(e) => {
            warn!(
                article_id = %article.id,
                enrichment_id = %enrichment_id,
                error = %e,
                "Image enrichment failed"
            );

            state
                .db
                .update_enrichment(&enrichment_id, "failed", None, Some(&e))
                .ok();

            Err(e)
        }
    }
}
