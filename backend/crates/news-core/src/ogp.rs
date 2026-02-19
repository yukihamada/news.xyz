use tracing::warn;

/// Extract article body text from HTML (strips scripts/styles, extracts p/h/li text).
/// Returns up to 3000 chars of meaningful content.
pub fn extract_article_text(html: &str) -> String {
    // Remove <script>...</script> and <style>...</style> blocks
    let re_script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let re_style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let cleaned = re_script.replace_all(html, "");
    let cleaned = re_style.replace_all(&cleaned, "");

    // Extract text from <p>, <h1>-<h6>, <li> tags
    let re_tags = regex::Regex::new(r"(?is)<(?:p|h[1-6]|li)[^>]*>(.*?)</(?:p|h[1-6]|li)>").unwrap();
    let re_html_tag = regex::Regex::new(r"<[^>]+>").unwrap();

    let mut texts = Vec::new();
    let mut total_len = 0;
    for cap in re_tags.captures_iter(&cleaned) {
        let inner = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let text = re_html_tag.replace_all(inner, "");
        let text = text.trim();
        if text.is_empty() || text.len() < 5 {
            continue;
        }
        let decoded = text
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&nbsp;", " ");
        let decoded = decoded.trim().to_string();
        if decoded.is_empty() {
            continue;
        }
        total_len += decoded.len();
        texts.push(decoded);
        if total_len >= 3000 {
            break;
        }
    }

    let mut result = texts.join("\n");
    if result.len() > 3000 {
        // Truncate at char boundary
        let mut end = 3000;
        while end > 0 && !result.is_char_boundary(end) {
            end -= 1;
        }
        result.truncate(end);
    }
    result
}

/// Fetch article content from a URL. Returns None on failure or empty content.
pub async fn fetch_article_content(client: &reqwest::Client, url: &str) -> Option<String> {
    let response = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            warn!(url = %url, error = %e, "Failed to fetch article content");
            return None;
        }
    };

    if !response.status().is_success() {
        return None;
    }

    // Read up to 256KB
    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(_) => return None,
    };

    let html = String::from_utf8_lossy(&bytes[..bytes.len().min(262144)]);
    let text = extract_article_text(&html);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Extract og:image URL from HTML content using regex (lightweight, no scraper crate).
pub fn extract_og_image(html: &str) -> Option<String> {
    // Match <meta property="og:image" content="..."> in various formats
    // Handles: single/double quotes, content before/after property, self-closing tags
    let patterns = [
        r#"<meta[^>]+property\s*=\s*["']og:image["'][^>]+content\s*=\s*["']([^"']+)["']"#,
        r#"<meta[^>]+content\s*=\s*["']([^"']+)["'][^>]+property\s*=\s*["']og:image["']"#,
    ];

    for pattern in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(html) {
                if let Some(url) = caps.get(1) {
                    let url_str = url.as_str().trim();
                    if url_str.starts_with("http") {
                        return Some(url_str.to_string());
                    }
                }
            }
        }
    }

    None
}

/// Fetch og:image from a URL. Returns None on any failure.
pub async fn fetch_og_image(client: &reqwest::Client, url: &str) -> Option<String> {
    let response = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            warn!(url = %url, error = %e, "Failed to fetch page for OGP");
            return None;
        }
    };

    if !response.status().is_success() {
        return None;
    }

    // Only read first 64KB to find og:image (it's in <head>)
    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(_) => return None,
    };

    let html = String::from_utf8_lossy(&bytes[..bytes.len().min(65536)]);
    extract_og_image(&html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_standard_og_image() {
        let html = r#"
        <html><head>
        <meta property="og:image" content="https://example.com/image.jpg">
        </head></html>
        "#;
        assert_eq!(
            extract_og_image(html),
            Some("https://example.com/image.jpg".into())
        );
    }

    #[test]
    fn extract_content_before_property() {
        let html = r#"
        <meta content="https://example.com/photo.png" property="og:image" />
        "#;
        assert_eq!(
            extract_og_image(html),
            Some("https://example.com/photo.png".into())
        );
    }

    #[test]
    fn extract_single_quotes() {
        let html = r#"
        <meta property='og:image' content='https://cdn.example.com/img.webp'>
        "#;
        assert_eq!(
            extract_og_image(html),
            Some("https://cdn.example.com/img.webp".into())
        );
    }

    #[test]
    fn no_og_image() {
        let html = r#"<html><head><title>Test</title></head></html>"#;
        assert_eq!(extract_og_image(html), None);
    }

    #[test]
    fn ignores_relative_urls() {
        let html = r#"<meta property="og:image" content="/images/local.jpg">"#;
        assert_eq!(extract_og_image(html), None);
    }

    #[test]
    fn extract_article_text_basic() {
        let html = r#"
        <html><head><script>var x = 1;</script><style>body{}</style></head>
        <body>
        <h1>Big News Today</h1>
        <p>This is the first paragraph with some content.</p>
        <p>Second paragraph here.</p>
        </body></html>
        "#;
        let text = extract_article_text(html);
        assert!(text.contains("Big News Today"));
        assert!(text.contains("first paragraph"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn extract_article_text_strips_scripts() {
        let html = r#"<script>alert('xss')</script><p>Safe content here.</p>"#;
        let text = extract_article_text(html);
        assert!(!text.contains("alert"));
        assert!(text.contains("Safe content"));
    }

    #[test]
    fn extract_article_text_empty() {
        let html = r#"<html><body><div>No p tags</div></body></html>"#;
        let text = extract_article_text(html);
        assert!(text.is_empty());
    }

    #[test]
    fn extract_article_text_truncates() {
        let long_para = "A".repeat(4000);
        let html = format!("<p>{}</p>", long_para);
        let text = extract_article_text(&html);
        assert!(text.len() <= 3000);
    }
}
