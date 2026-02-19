use hmac::{Hmac, Mac};
use sha2::Sha256;
use tracing::{info, warn};

type HmacSha256 = Hmac<Sha256>;

pub struct CheckoutResult {
    pub session_url: String,
}

pub async fn create_checkout_session(
    client: &reqwest::Client,
    secret_key: &str,
    price_id: &str,
    success_url: &str,
    cancel_url: &str,
    client_reference_id: &str,
) -> Result<CheckoutResult, String> {
    let params = [
        ("mode", "subscription"),
        ("payment_method_types[]", "card"),
        ("line_items[0][price]", price_id),
        ("line_items[0][quantity]", "1"),
        ("success_url", success_url),
        ("cancel_url", cancel_url),
        ("client_reference_id", client_reference_id),
    ];

    let resp = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(secret_key, None::<&str>)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Stripe request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(status = %status, body = %body, "Stripe checkout session error");
        return Err(format!("Stripe error: {status}"));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Stripe JSON parse error: {e}"))?;

    let url = json["url"]
        .as_str()
        .ok_or_else(|| "No URL in Stripe response".to_string())?
        .to_string();

    info!("Stripe checkout session created");
    Ok(CheckoutResult { session_url: url })
}

pub async fn create_billing_portal_session(
    client: &reqwest::Client,
    secret_key: &str,
    customer_id: &str,
    return_url: &str,
) -> Result<String, String> {
    let params = [
        ("customer", customer_id),
        ("return_url", return_url),
    ];

    let resp = client
        .post("https://api.stripe.com/v1/billing_portal/sessions")
        .basic_auth(secret_key, None::<&str>)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Stripe portal request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(status = %status, body = %body, "Stripe billing portal error");
        return Err(format!("Stripe error: {status}"));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Stripe JSON parse error: {e}"))?;

    let url = json["url"]
        .as_str()
        .ok_or_else(|| "No URL in Stripe portal response".to_string())?
        .to_string();

    info!("Stripe billing portal session created");
    Ok(url)
}

pub fn verify_webhook_signature(
    payload: &[u8],
    sig_header: &str,
    webhook_secret: &str,
) -> Result<(), String> {
    // Parse Stripe-Signature header: t=TIMESTAMP,v1=SIGNATURE
    let mut timestamp = "";
    let mut signature = "";

    for part in sig_header.split(',') {
        let kv: Vec<&str> = part.splitn(2, '=').collect();
        if kv.len() != 2 {
            continue;
        }
        match kv[0] {
            "t" => timestamp = kv[1],
            "v1" => signature = kv[1],
            _ => {}
        }
    }

    if timestamp.is_empty() || signature.is_empty() {
        return Err("Invalid Stripe-Signature header".into());
    }

    // Verify timestamp is recent (within 5 minutes)
    if let Ok(ts) = timestamp.parse::<i64>() {
        let now = chrono::Utc::now().timestamp();
        if (now - ts).abs() > 300 {
            return Err("Webhook timestamp too old".into());
        }
    }

    // Compute expected signature: HMAC-SHA256(webhook_secret, "TIMESTAMP.PAYLOAD")
    let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));

    let mut mac =
        HmacSha256::new_from_slice(webhook_secret.as_bytes()).map_err(|e| format!("HMAC error: {e}"))?;
    mac.update(signed_payload.as_bytes());

    let expected = hex::encode(mac.finalize().into_bytes());

    if !constant_time_eq(expected.as_bytes(), signature.as_bytes()) {
        return Err("Invalid webhook signature".into());
    }

    Ok(())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}
