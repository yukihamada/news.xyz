use url::Url;
use uuid::Uuid;

/// Namespace UUID for generating deterministic article IDs from URLs.
const URL_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
    0xc8,
]);

/// Tracking query parameters to strip before normalization.
const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "ref",
    "fbclid",
    "gclid",
    "mc_cid",
    "mc_eid",
];

/// Normalize a URL by removing tracking parameters and fragments,
/// then generate a deterministic UUID v5 from the normalized URL.
pub fn article_id_from_url(raw_url: &str) -> String {
    let normalized = normalize_url(raw_url);
    Uuid::new_v5(&URL_NAMESPACE, normalized.as_bytes()).to_string()
}

/// Strip tracking params, fragments, and normalize scheme/host to lowercase.
fn normalize_url(raw: &str) -> String {
    let Ok(mut parsed) = Url::parse(raw) else {
        return raw.to_string();
    };

    // Remove fragment
    parsed.set_fragment(None);

    // Filter out tracking query params
    let filtered: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(key, _)| !TRACKING_PARAMS.contains(&key.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if filtered.is_empty() {
        parsed.set_query(None);
    } else {
        let qs: Vec<String> = filtered.iter().map(|(k, v)| format!("{k}={v}")).collect();
        parsed.set_query(Some(&qs.join("&")));
    }

    parsed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_url_same_id() {
        let id1 = article_id_from_url("https://example.com/article/1");
        let id2 = article_id_from_url("https://example.com/article/1");
        assert_eq!(id1, id2);
    }

    #[test]
    fn tracking_params_stripped() {
        let id1 = article_id_from_url("https://example.com/article/1");
        let id2 =
            article_id_from_url("https://example.com/article/1?utm_source=twitter&utm_medium=social");
        assert_eq!(id1, id2);
    }

    #[test]
    fn fragment_stripped() {
        let id1 = article_id_from_url("https://example.com/article/1");
        let id2 = article_id_from_url("https://example.com/article/1#section");
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_urls_different_ids() {
        let id1 = article_id_from_url("https://example.com/article/1");
        let id2 = article_id_from_url("https://example.com/article/2");
        assert_ne!(id1, id2);
    }

    #[test]
    fn non_tracking_params_preserved() {
        let id1 = article_id_from_url("https://example.com/search?q=rust");
        let id2 = article_id_from_url("https://example.com/search?q=go");
        assert_ne!(id1, id2);
    }
}
