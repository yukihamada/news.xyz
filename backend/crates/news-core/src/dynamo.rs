use crate::error::{AppError, Result};
use crate::models::{Article, Category};
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use tracing::{info, warn};

const ALL_PARTITION: &str = "ALL";
const TTL_DAYS: i64 = 7;

/// DynamoDB client wrapper for article operations.
#[derive(Clone)]
pub struct ArticleStore {
    client: Client,
    table_name: String,
}

impl ArticleStore {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    /// Build the sort key: `{published_at_iso}#{article_id}`
    fn sort_key(article: &Article) -> String {
        format!(
            "{}#{}",
            article.published_at.to_rfc3339(),
            article.id
        )
    }

    /// Put an article with idempotent write (attribute_not_exists).
    pub async fn put_article(&self, article: &Article) -> Result<bool> {
        let sk = Self::sort_key(article);
        let ttl = (Utc::now() + Duration::days(TTL_DAYS)).timestamp();

        let mut item: HashMap<String, AttributeValue> = HashMap::new();
        item.insert("category".into(), AttributeValue::S(article.category.to_string()));
        item.insert("sk".into(), AttributeValue::S(sk));
        item.insert("gsi_pk".into(), AttributeValue::S(ALL_PARTITION.into()));
        item.insert("article_id".into(), AttributeValue::S(article.id.clone()));
        item.insert("title".into(), AttributeValue::S(article.title.clone()));
        item.insert("url".into(), AttributeValue::S(article.url.clone()));
        item.insert("source".into(), AttributeValue::S(article.source.clone()));
        item.insert(
            "published_at".into(),
            AttributeValue::S(article.published_at.to_rfc3339()),
        );
        item.insert(
            "fetched_at".into(),
            AttributeValue::S(article.fetched_at.to_rfc3339()),
        );
        item.insert("ttl".into(), AttributeValue::N(ttl.to_string()));

        if let Some(ref desc) = article.description {
            item.insert("description".into(), AttributeValue::S(desc.clone()));
        }
        if let Some(ref img) = article.image_url {
            item.insert("image_url".into(), AttributeValue::S(img.clone()));
        }

        let result = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .condition_expression("attribute_not_exists(sk)")
            .send()
            .await;

        match result {
            Ok(_) => Ok(true),
            Err(e) => {
                let service_err = e.into_service_error();
                if service_err.is_conditional_check_failed_exception() {
                    Ok(false) // duplicate, skip
                } else {
                    Err(AppError::DynamoError(service_err.to_string()))
                }
            }
        }
    }

    /// Batch put articles, returning count of newly inserted.
    pub async fn put_articles(&self, articles: &[Article]) -> Result<usize> {
        let mut inserted = 0;
        for article in articles {
            match self.put_article(article).await {
                Ok(true) => inserted += 1,
                Ok(false) => {} // duplicate
                Err(e) => warn!(article_id = %article.id, error = %e, "Failed to put article"),
            }
        }
        info!(total = articles.len(), inserted, "Batch put complete");
        Ok(inserted)
    }

    /// Update the image_url of an article.
    pub async fn update_image_url(
        &self,
        category: &str,
        sk: &str,
        image_url: &str,
    ) -> Result<()> {
        self.client
            .update_item()
            .table_name(&self.table_name)
            .key("category", AttributeValue::S(category.into()))
            .key("sk", AttributeValue::S(sk.into()))
            .update_expression("SET image_url = :img")
            .expression_attribute_values(":img", AttributeValue::S(image_url.into()))
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;
        Ok(())
    }

    /// Query articles by category, newest first, with cursor-based pagination.
    pub async fn query_articles(
        &self,
        category: Option<&Category>,
        limit: i32,
        cursor: Option<&str>,
    ) -> Result<(Vec<Article>, Option<String>)> {
        let (pk_name, pk_value) = match category {
            Some(cat) => ("category", cat.to_string()),
            None => ("gsi_pk", ALL_PARTITION.to_string()),
        };

        let index_name = if category.is_none() {
            Some("all-articles".to_string())
        } else {
            None
        };

        let mut query = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("#pk = :pk")
            .expression_attribute_names("#pk", pk_name)
            .expression_attribute_values(":pk", AttributeValue::S(pk_value))
            .scan_index_forward(false) // newest first
            .limit(limit);

        if let Some(idx) = index_name {
            query = query.index_name(idx);
        }

        if let Some(cursor_str) = cursor {
            if let Some(start_key) = decode_cursor(cursor_str) {
                query = query.set_exclusive_start_key(Some(start_key));
            }
        }

        let output = query
            .send()
            .await
            .map_err(|e| AppError::DynamoError(e.into_service_error().to_string()))?;

        let items = output.items.unwrap_or_default();
        let articles: Vec<Article> = items.iter().filter_map(item_to_article).collect();

        let next_cursor = output
            .last_evaluated_key
            .map(|key| encode_cursor(&key));

        Ok((articles, next_cursor))
    }
}

fn item_to_article(item: &HashMap<String, AttributeValue>) -> Option<Article> {
    let category_str = item.get("category")?.as_s().ok()?;
    let category = Category::from_str(category_str)?;
    let id = item.get("article_id")?.as_s().ok()?.clone();
    let title = item.get("title")?.as_s().ok()?.clone();
    let url = item.get("url")?.as_s().ok()?.clone();
    let source = item.get("source")?.as_s().ok()?.clone();
    let published_at: DateTime<Utc> = item
        .get("published_at")?
        .as_s()
        .ok()?
        .parse()
        .ok()?;
    let fetched_at: DateTime<Utc> = item
        .get("fetched_at")?
        .as_s()
        .ok()?
        .parse()
        .ok()?;
    let description = item
        .get("description")
        .and_then(|v| v.as_s().ok().cloned());
    let image_url = item.get("image_url").and_then(|v| v.as_s().ok().cloned());

    Some(Article {
        id,
        category,
        title,
        url,
        description,
        image_url,
        source,
        published_at,
        fetched_at,
        group_id: None,
        group_count: None,
    })
}

/// Encode DynamoDB last_evaluated_key as a base64 JSON cursor.
fn encode_cursor(key: &HashMap<String, AttributeValue>) -> String {
    use base64::Engine;
    let map: HashMap<String, String> = key
        .iter()
        .filter_map(|(k, v)| v.as_s().ok().map(|s| (k.clone(), s.clone())))
        .collect();
    let json = serde_json::to_string(&map).unwrap_or_default();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes())
}

/// Decode a base64 JSON cursor back to DynamoDB exclusive_start_key.
fn decode_cursor(cursor: &str) -> Option<HashMap<String, AttributeValue>> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()?;
    let json: HashMap<String, String> = serde_json::from_slice(&bytes).ok()?;
    let map: HashMap<String, AttributeValue> = json
        .into_iter()
        .map(|(k, v)| (k, AttributeValue::S(v)))
        .collect();
    Some(map)
}
