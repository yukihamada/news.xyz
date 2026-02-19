use chrono::{DateTime, Utc};
use news_core::changes::{AdminAction, ChangeRequest, ChangeStatus};
use news_core::config::{DynamicFeed, FeatureFlags, ServiceConfig};
use news_core::models::{Article, Category};
use rusqlite::{params, Connection};
use std::sync::Mutex;
use tracing::info;

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &str) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("SQLite open: {e}"))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=5000;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;",
        )
        .map_err(|e| format!("SQLite pragma: {e}"))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS articles (
                id TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                description TEXT,
                image_url TEXT,
                source TEXT NOT NULL,
                published_at TEXT NOT NULL,
                fetched_at TEXT NOT NULL,
                group_id TEXT,
                group_count INTEGER,
                view_count INTEGER NOT NULL DEFAULT 0,
                click_count INTEGER NOT NULL DEFAULT 0,
                enrichment_status TEXT,
                enriched_at TEXT,
                popularity_score REAL NOT NULL DEFAULT 0.0,
                ai_summary TEXT,
                ai_keywords TEXT,
                ai_sentiment TEXT,
                ai_importance REAL,
                ai_category TEXT,
                analyzed_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_articles_cat_pub
                ON articles(category, published_at DESC);
            CREATE INDEX IF NOT EXISTS idx_articles_pub
                ON articles(published_at DESC);
            CREATE INDEX IF NOT EXISTS idx_articles_popularity
                ON articles(popularity_score DESC, published_at DESC);
            CREATE INDEX IF NOT EXISTS idx_articles_enrichment_status
                ON articles(enrichment_status);

            CREATE TABLE IF NOT EXISTS feeds (
                feed_id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                source TEXT NOT NULL,
                category TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                added_by TEXT
            );

            CREATE TABLE IF NOT EXISTS features (
                feature TEXT PRIMARY KEY,
                enabled INTEGER NOT NULL DEFAULT 0,
                extra_json TEXT
            );

            CREATE TABLE IF NOT EXISTS changes (
                change_id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                command_text TEXT NOT NULL,
                interpretation TEXT NOT NULL DEFAULT '',
                actions_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS categories (
                id TEXT PRIMARY KEY,
                label_ja TEXT NOT NULL,
                label_en TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0,
                visible INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS subscriptions (
                api_token TEXT PRIMARY KEY,
                stripe_customer_id TEXT NOT NULL,
                stripe_subscription_id TEXT NOT NULL UNIQUE,
                status TEXT NOT NULL DEFAULT 'active',
                current_period_end TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_subs_stripe_sub_id
                ON subscriptions(stripe_subscription_id);
            CREATE INDEX IF NOT EXISTS idx_subs_stripe_cust_id
                ON subscriptions(stripe_customer_id);

            CREATE TABLE IF NOT EXISTS usage_limits (
                device_id TEXT NOT NULL,
                feature TEXT NOT NULL,
                used_date TEXT NOT NULL,
                count INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (device_id, feature, used_date)
            );

            CREATE TABLE IF NOT EXISTS ai_cache (
                cache_key TEXT PRIMARY KEY,
                endpoint TEXT NOT NULL,
                response_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_ai_cache_expires
                ON ai_cache(expires_at);

            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL DEFAULT '',
                picture_url TEXT,
                google_id TEXT NOT NULL UNIQUE,
                auth_token TEXT NOT NULL UNIQUE,
                device_id TEXT,
                konami_claimed INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_users_auth_token ON users(auth_token);

            CREATE TABLE IF NOT EXISTS enrichments (
                enrichment_id TEXT PRIMARY KEY,
                article_id TEXT NOT NULL,
                agent_type TEXT NOT NULL,
                content_type TEXT NOT NULL,
                data_json TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                error_message TEXT,
                created_at TEXT NOT NULL,
                completed_at TEXT,
                FOREIGN KEY (article_id) REFERENCES articles(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_enrichments_article
                ON enrichments(article_id, status);",
        )
        .map_err(|e| format!("SQLite schema: {e}"))?;

        // Migration: Add AI analysis columns if they don't exist
        let column_check: Result<i64, _> = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('articles') WHERE name='ai_summary'",
            [],
            |row| row.get(0),
        );

        if let Ok(0) = column_check {
            info!("Running migration: Adding AI analysis columns to articles table");
            conn.execute_batch(
                "ALTER TABLE articles ADD COLUMN ai_summary TEXT;
                 ALTER TABLE articles ADD COLUMN ai_keywords TEXT;
                 ALTER TABLE articles ADD COLUMN ai_sentiment TEXT;
                 ALTER TABLE articles ADD COLUMN ai_importance REAL;
                 ALTER TABLE articles ADD COLUMN ai_category TEXT;
                 ALTER TABLE articles ADD COLUMN analyzed_at TEXT;"
            )
            .map_err(|e| format!("Migration failed: {e}"))?;
            info!("Migration complete: AI analysis columns added");
        }

        info!(path, "SQLite database opened");
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // --- Articles ---

    pub fn insert_article(&self, article: &Article) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.execute(
            "INSERT OR IGNORE INTO articles
                (id, category, title, url, description, image_url, source, published_at, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                article.id,
                article.category.as_str(),
                article.title,
                article.url,
                article.description,
                article.image_url,
                article.source,
                article.published_at.to_rfc3339(),
                article.fetched_at.to_rfc3339(),
            ],
        );
        match result {
            Ok(n) => Ok(n > 0),
            Err(e) => Err(format!("Insert article: {e}")),
        }
    }

    pub fn insert_articles(&self, articles: &[Article]) -> Result<usize, String> {
        let mut inserted = 0;
        for a in articles {
            if self.insert_article(a)? {
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    pub fn update_image_url(&self, article_id: &str, image_url: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE articles SET image_url = ?1 WHERE id = ?2",
            params![image_url, article_id],
        )
        .map_err(|e| format!("Update image: {e}"))?;
        Ok(())
    }

    pub fn query_articles(
        &self,
        category: Option<&Category>,
        limit: i64,
        cursor: Option<&str>,
    ) -> Result<(Vec<Article>, Option<String>), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let (cursor_pub, cursor_id) = match cursor {
            Some(c) => decode_cursor(c).unwrap_or((String::new(), String::new())),
            None => (String::new(), String::new()),
        };
        let has_cursor = !cursor_pub.is_empty();
        let fetch_limit = limit + 1;

        // Build SQL dynamically to avoid borrow issues
        let mut conditions = Vec::new();
        if category.is_some() {
            conditions.push("category = :cat");
        }
        if has_cursor {
            conditions.push("(published_at < :cpub OR (published_at = :cpub AND id < :cid))");
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, category, title, url, description, image_url, source,
                    published_at, fetched_at, group_id, group_count
             FROM articles {}
             ORDER BY published_at DESC, id DESC
             LIMIT :lim",
            where_clause
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

        let cat_str = category.map(|c| c.as_str().to_string());
        let mut idx = 0;
        let mut param_names: Vec<&str> = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref cat) = cat_str {
            param_names.push(":cat");
            param_values.push(Box::new(cat.clone()));
            idx += 1;
        }
        if has_cursor {
            param_names.push(":cpub");
            param_values.push(Box::new(cursor_pub.clone()));
            param_names.push(":cid");
            param_values.push(Box::new(cursor_id.clone()));
            idx += 2;
        }
        param_names.push(":lim");
        param_values.push(Box::new(fetch_limit));
        let _ = idx;

        let params: Vec<(&str, &dyn rusqlite::types::ToSql)> = param_names
            .iter()
            .zip(param_values.iter())
            .map(|(name, val)| (*name, val.as_ref()))
            .collect();

        let rows = stmt
            .query_map(params.as_slice(), row_to_article)
            .map_err(|e| e.to_string())?;
        let mut articles: Vec<Article> = rows.filter_map(|r| r.ok()).collect();

        let next_cursor = if articles.len() as i64 > limit {
            articles.truncate(limit as usize);
            articles.last().map(|a| encode_cursor(a))
        } else {
            None
        };

        Ok((articles, next_cursor))
    }

    pub fn articles_without_image(&self, limit: i64) -> Result<Vec<Article>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, category, title, url, description, image_url, source,
                        published_at, fetched_at, group_id, group_count
                 FROM articles WHERE image_url IS NULL
                 ORDER BY published_at DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let articles = stmt
            .query_map(params![limit], row_to_article)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(articles)
    }

    pub fn delete_old_articles(&self, before: &DateTime<Utc>) -> Result<usize, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let deleted = conn
            .execute(
                "DELETE FROM articles WHERE published_at < ?1",
                params![before.to_rfc3339()],
            )
            .map_err(|e| format!("Delete old: {e}"))?;
        Ok(deleted)
    }

    pub fn get_article_by_id(&self, id: &str) -> Result<Option<Article>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, category, title, url, description, image_url, source,
                        published_at, fetched_at, group_id, group_count
                 FROM articles WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query_map(params![id], row_to_article)
            .map_err(|e| e.to_string())?;
        match rows.next() {
            Some(Ok(article)) => Ok(Some(article)),
            Some(Err(e)) => Err(e.to_string()),
            None => Ok(None),
        }
    }

    // --- Search ---

    pub fn search_articles(&self, query: &str, limit: i64) -> Result<Vec<Article>, String> {
        let search = format!("%{}%", query);
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, category, title, url, description, image_url, source,
                        published_at, fetched_at, group_id, group_count
                 FROM articles
                 WHERE title LIKE ?1 OR description LIKE ?1
                 ORDER BY published_at DESC
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let articles = stmt
            .query_map(params![search, limit], row_to_article)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(articles)
    }

    // --- Feeds ---

    pub fn get_enabled_feeds(&self) -> Result<Vec<DynamicFeed>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT feed_id, url, source, category, enabled, added_by FROM feeds WHERE enabled = 1")
            .map_err(|e| e.to_string())?;
        let feeds = stmt
            .query_map([], |row| {
                Ok(DynamicFeed {
                    feed_id: row.get(0)?,
                    url: row.get(1)?,
                    source: row.get(2)?,
                    category: row.get(3)?,
                    enabled: row.get::<_, i32>(4)? != 0,
                    added_by: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(feeds)
    }

    pub fn get_all_feeds(&self) -> Result<Vec<DynamicFeed>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT feed_id, url, source, category, enabled, added_by FROM feeds")
            .map_err(|e| e.to_string())?;
        let feeds = stmt
            .query_map([], |row| {
                Ok(DynamicFeed {
                    feed_id: row.get(0)?,
                    url: row.get(1)?,
                    source: row.get(2)?,
                    category: row.get(3)?,
                    enabled: row.get::<_, i32>(4)? != 0,
                    added_by: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(feeds)
    }

    pub fn put_feed(&self, feed: &DynamicFeed) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO feeds (feed_id, url, source, category, enabled, added_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                feed.feed_id,
                feed.url,
                feed.source,
                feed.category,
                feed.enabled as i32,
                feed.added_by,
            ],
        )
        .map_err(|e| format!("Put feed: {e}"))?;
        info!(feed_id = %feed.feed_id, source = %feed.source, "Feed saved");
        Ok(())
    }

    pub fn delete_feed(&self, feed_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM feeds WHERE feed_id = ?1", params![feed_id])
            .map_err(|e| format!("Delete feed: {e}"))?;
        info!(feed_id, "Feed deleted");
        Ok(())
    }

    pub fn feed_count(&self) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT COUNT(*) FROM feeds", [], |row| row.get(0))
            .map_err(|e| format!("Feed count: {e}"))
    }

    // --- Features ---

    pub fn get_feature_flags(&self) -> Result<FeatureFlags, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut flags = FeatureFlags::default();

        let mut stmt = conn
            .prepare("SELECT feature, enabled, extra_json FROM features")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i32>(1)? != 0,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows.flatten() {
            let (feature, enabled, extra) = row;
            match feature.as_str() {
                "grouping" => {
                    flags.grouping_enabled = enabled;
                    if let Some(ref json) = extra {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
                            if let Some(t) = v.get("similarity_threshold").and_then(|t| t.as_f64())
                            {
                                flags.grouping_threshold = t;
                            }
                        }
                    }
                }
                "ogp_enrichment" => {
                    flags.ogp_enrichment_enabled = enabled;
                }
                _ => {}
            }
        }
        Ok(flags)
    }

    pub fn get_service_config(&self) -> Result<ServiceConfig, String> {
        let feeds = self.get_all_feeds()?;
        let features = self.get_feature_flags()?;
        Ok(ServiceConfig { feeds, features })
    }

    pub fn set_feature_flag(
        &self,
        feature: &str,
        enabled: bool,
        extra_json: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO features (feature, enabled, extra_json) VALUES (?1, ?2, ?3)",
            params![feature, enabled as i32, extra_json],
        )
        .map_err(|e| format!("Set feature: {e}"))?;
        info!(feature, enabled, "Feature flag updated");
        Ok(())
    }

    // --- Categories ---

    pub fn category_count(&self) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))
            .map_err(|e| format!("Category count: {e}"))
    }

    pub fn seed_default_categories(&self) -> Result<(), String> {
        let defaults = [
            ("general", "総合", "General", 0),
            ("tech", "テクノロジー", "Technology", 1),
            ("business", "ビジネス", "Business", 2),
            ("entertainment", "エンタメ", "Entertainment", 3),
            ("sports", "スポーツ", "Sports", 4),
            ("science", "サイエンス", "Science", 5),
            ("podcast", "ポッドキャスト", "Podcast", 6),
        ];
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        for (id, ja, en, order) in defaults {
            conn.execute(
                "INSERT OR IGNORE INTO categories (id, label_ja, label_en, sort_order, visible) VALUES (?1, ?2, ?3, ?4, 1)",
                params![id, ja, en, order],
            ).map_err(|e| format!("Seed category: {e}"))?;
        }
        info!("Default categories seeded");
        Ok(())
    }

    pub fn ensure_all_categories_visible(&self) -> Result<usize, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let updated = conn
            .execute("UPDATE categories SET visible = 1 WHERE visible = 0", [])
            .map_err(|e| format!("Ensure visible: {e}"))?;
        if updated > 0 {
            info!(updated, "Made hidden categories visible");
        }
        Ok(updated)
    }

    pub fn get_categories(&self) -> Result<Vec<(String, String, String, i32, bool)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, label_ja, label_en, sort_order, visible FROM categories ORDER BY sort_order ASC, id ASC")
            .map_err(|e| e.to_string())?;
        let cats = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i32>(3)?,
                    row.get::<_, i32>(4)? != 0,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(cats)
    }

    pub fn put_category(&self, id: &str, label_ja: &str, label_en: &str, sort_order: i32) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO categories (id, label_ja, label_en, sort_order, visible) VALUES (?1, ?2, ?3, ?4, 1)",
            params![id, label_ja, label_en, sort_order],
        ).map_err(|e| format!("Put category: {e}"))?;
        info!(id, label_ja, "Category saved");
        Ok(())
    }

    pub fn rename_category(&self, id: &str, label_ja: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let affected = conn.execute(
            "UPDATE categories SET label_ja = ?1 WHERE id = ?2",
            params![label_ja, id],
        ).map_err(|e| format!("Rename category: {e}"))?;
        if affected == 0 {
            return Err(format!("Category not found: {}", id));
        }
        info!(id, label_ja, "Category renamed");
        Ok(())
    }

    pub fn delete_category(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM categories WHERE id = ?1", params![id])
            .map_err(|e| format!("Delete category: {e}"))?;
        info!(id, "Category deleted");
        Ok(())
    }

    pub fn reorder_categories(&self, order: &[String]) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        for (i, id) in order.iter().enumerate() {
            conn.execute(
                "UPDATE categories SET sort_order = ?1 WHERE id = ?2",
                params![i as i32, id],
            ).map_err(|e| format!("Reorder category: {e}"))?;
        }
        info!(count = order.len(), "Categories reordered");
        Ok(())
    }

    // --- Changes ---

    pub fn create_change(&self, change: &ChangeRequest) -> Result<(), String> {
        let actions_json =
            serde_json::to_string(&change.actions).map_err(|e| format!("Serialize actions: {e}"))?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO changes (change_id, status, command_text, interpretation, actions_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                change.change_id,
                change.status.as_str(),
                change.command_text,
                change.interpretation,
                actions_json,
                change.created_at,
            ],
        )
        .map_err(|e| format!("Create change: {e}"))?;
        info!(change_id = %change.change_id, "Change request created");
        Ok(())
    }

    pub fn get_change(&self, change_id: &str) -> Result<Option<ChangeRequest>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT change_id, status, command_text, interpretation, actions_json, created_at
                 FROM changes WHERE change_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_row(params![change_id], |row| {
                let status_str: String = row.get(1)?;
                let actions_json: String = row.get(4)?;
                Ok((
                    row.get::<_, String>(0)?,
                    status_str,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    actions_json,
                    row.get::<_, String>(5)?,
                ))
            })
            .ok();

        match result {
            Some((change_id, status_str, command_text, interpretation, actions_json, created_at)) => {
                let status = ChangeStatus::from_str(&status_str).unwrap_or(ChangeStatus::Pending);
                let actions: Vec<AdminAction> =
                    serde_json::from_str(&actions_json).unwrap_or_default();
                Ok(Some(ChangeRequest {
                    change_id,
                    status,
                    command_text,
                    interpretation,
                    actions,
                    preview_config: None,
                    created_at,
                }))
            }
            None => Ok(None),
        }
    }

    pub fn update_change_status(
        &self,
        change_id: &str,
        status: ChangeStatus,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE changes SET status = ?1 WHERE change_id = ?2",
            params![status.as_str(), change_id],
        )
        .map_err(|e| format!("Update change status: {e}"))?;
        info!(change_id, status = status.as_str(), "Change status updated");
        Ok(())
    }

    // --- Subscriptions ---

    pub fn create_subscription(
        &self,
        api_token: &str,
        stripe_customer_id: &str,
        stripe_subscription_id: &str,
        current_period_end: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO subscriptions
                (api_token, stripe_customer_id, stripe_subscription_id, status, current_period_end, created_at)
             VALUES (?1, ?2, ?3, 'active', ?4, ?5)",
            params![
                api_token,
                stripe_customer_id,
                stripe_subscription_id,
                current_period_end,
                chrono::Utc::now().to_rfc3339(),
            ],
        )
        .map_err(|e| format!("Create subscription: {e}"))?;
        info!(stripe_subscription_id, "Subscription created");
        Ok(())
    }

    pub fn get_subscription_by_token(
        &self,
        api_token: &str,
    ) -> Result<Option<(String, String, String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT stripe_customer_id, stripe_subscription_id, status, current_period_end
                 FROM subscriptions WHERE api_token = ?1",
            )
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_row(params![api_token], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .ok();
        Ok(result)
    }

    #[allow(dead_code)]
    pub fn get_subscription_by_stripe_id(
        &self,
        stripe_subscription_id: &str,
    ) -> Result<Option<(String, String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT api_token, status, current_period_end
                 FROM subscriptions WHERE stripe_subscription_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_row(params![stripe_subscription_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .ok();
        Ok(result)
    }

    #[allow(dead_code)]
    pub fn get_subscription_by_customer_id(
        &self,
        stripe_customer_id: &str,
    ) -> Result<Option<(String, String, String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT api_token, stripe_subscription_id, status, current_period_end
                 FROM subscriptions WHERE stripe_customer_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_row(params![stripe_customer_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .ok();
        Ok(result)
    }

    pub fn update_subscription_status(
        &self,
        stripe_subscription_id: &str,
        status: &str,
        current_period_end: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        if let Some(period_end) = current_period_end {
            conn.execute(
                "UPDATE subscriptions SET status = ?1, current_period_end = ?2 WHERE stripe_subscription_id = ?3",
                params![status, period_end, stripe_subscription_id],
            )
        } else {
            conn.execute(
                "UPDATE subscriptions SET status = ?1 WHERE stripe_subscription_id = ?2",
                params![status, stripe_subscription_id],
            )
        }
        .map_err(|e| format!("Update subscription: {e}"))?;
        info!(stripe_subscription_id, status, "Subscription status updated");
        Ok(())
    }

    // --- Usage Limits ---

    pub fn increment_usage(&self, device_id: &str, feature: &str) -> Result<i64, String> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO usage_limits (device_id, feature, used_date, count)
             VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(device_id, feature, used_date)
             DO UPDATE SET count = count + 1",
            params![device_id, feature, today],
        )
        .map_err(|e| format!("Increment usage: {e}"))?;
        let count: i64 = conn
            .query_row(
                "SELECT count FROM usage_limits WHERE device_id = ?1 AND feature = ?2 AND used_date = ?3",
                params![device_id, feature, today],
                |row| row.get(0),
            )
            .map_err(|e| format!("Get usage count: {e}"))?;
        Ok(count)
    }

    pub fn get_usage(&self, device_id: &str, feature: &str) -> Result<i64, String> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let count = conn
            .query_row(
                "SELECT count FROM usage_limits WHERE device_id = ?1 AND feature = ?2 AND used_date = ?3",
                params![device_id, feature, today],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0);
        Ok(count)
    }

    pub fn get_all_usage(&self, device_id: &str) -> Result<Vec<(String, i64)>, String> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT feature, count FROM usage_limits WHERE device_id = ?1 AND used_date = ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![device_id, today], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn cleanup_old_usage(&self, days_to_keep: i64) -> Result<usize, String> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days_to_keep))
            .format("%Y-%m-%d")
            .to_string();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let deleted = conn
            .execute(
                "DELETE FROM usage_limits WHERE used_date < ?1",
                params![cutoff],
            )
            .map_err(|e| format!("Cleanup usage: {e}"))?;
        Ok(deleted)
    }

    pub fn list_changes(&self, limit: i64) -> Result<Vec<ChangeRequest>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT change_id, status, command_text, interpretation, actions_json, created_at
                 FROM changes ORDER BY created_at DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let changes = stmt
            .query_map(params![limit], |row| {
                let status_str: String = row.get(1)?;
                let actions_json: String = row.get(4)?;
                Ok((
                    row.get::<_, String>(0)?,
                    status_str,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    actions_json,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .map(
                |(change_id, status_str, command_text, interpretation, actions_json, created_at)| {
                    let status =
                        ChangeStatus::from_str(&status_str).unwrap_or(ChangeStatus::Pending);
                    let actions: Vec<AdminAction> =
                        serde_json::from_str(&actions_json).unwrap_or_default();
                    ChangeRequest {
                        change_id,
                        status,
                        command_text,
                        interpretation,
                        actions,
                        preview_config: None,
                        created_at,
                    }
                },
            )
            .collect();
        Ok(changes)
    }

    // --- Top Articles per Category (for TTS pre-cache) ---

    pub fn top_articles_per_category(&self, per_category: i64) -> Result<Vec<Article>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, category, title, url, description, image_url, source,
                        published_at, fetched_at, group_id, group_count
                 FROM (
                     SELECT *, ROW_NUMBER() OVER (PARTITION BY category ORDER BY published_at DESC) AS rn
                     FROM articles
                     WHERE category != 'podcast'
                 )
                 WHERE rn <= ?1",
            )
            .map_err(|e| e.to_string())?;
        let articles = stmt
            .query_map(params![per_category], row_to_article)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(articles)
    }

    // --- AI Cache ---

    pub fn get_cache(&self, cache_key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn
            .prepare(
                "SELECT response_json FROM ai_cache WHERE cache_key = ?1 AND expires_at > ?2",
            )
            .map_err(|e| e.to_string())?;
        let result: Option<String> = stmt
            .query_row(params![cache_key, now], |row| row.get(0))
            .ok();
        Ok(result)
    }

    pub fn set_cache(
        &self,
        cache_key: &str,
        endpoint: &str,
        response_json: &str,
        ttl_secs: i64,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now();
        let expires = now + chrono::Duration::seconds(ttl_secs);
        conn.execute(
            "INSERT OR REPLACE INTO ai_cache (cache_key, endpoint, response_json, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                cache_key,
                endpoint,
                response_json,
                now.to_rfc3339(),
                expires.to_rfc3339()
            ],
        )
        .map_err(|e| format!("Set cache: {e}"))?;
        Ok(())
    }

    pub fn cleanup_expired_cache(&self) -> Result<usize, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        let deleted = conn
            .execute("DELETE FROM ai_cache WHERE expires_at < ?1", params![now])
            .map_err(|e| format!("Cleanup cache: {e}"))?;
        Ok(deleted)
    }

    // --- Users (Google Auth) ---

    /// Upsert a user from Google Sign-In. Returns (auth_token, user_id, is_new).
    pub fn upsert_user(
        &self,
        google_id: &str,
        email: &str,
        name: &str,
        picture_url: Option<&str>,
        device_id: Option<&str>,
    ) -> Result<(String, String, bool), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();

        // Check if user already exists
        let existing: Option<(String, String)> = conn
            .query_row(
                "SELECT id, auth_token FROM users WHERE google_id = ?1",
                params![google_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        if let Some((user_id, auth_token)) = existing {
            // Update existing user
            conn.execute(
                "UPDATE users SET email = ?1, name = ?2, picture_url = ?3, device_id = COALESCE(?4, device_id), updated_at = ?5 WHERE id = ?6",
                params![email, name, picture_url, device_id, now, user_id],
            )
            .map_err(|e| format!("Update user: {e}"))?;
            info!(user_id = %user_id, email = %email, "User updated");
            Ok((auth_token, user_id, false))
        } else {
            // Create new user
            let user_id = uuid::Uuid::new_v4().to_string();
            let auth_token = format!("ga_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
            conn.execute(
                "INSERT INTO users (id, email, name, picture_url, google_id, auth_token, device_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![user_id, email, name, picture_url, google_id, auth_token, device_id, now],
            )
            .map_err(|e| format!("Insert user: {e}"))?;
            info!(user_id = %user_id, email = %email, "New user created");
            Ok((auth_token, user_id, true))
        }
    }

    /// Get a user by their auth token. Returns (user_id, email, name, picture_url, device_id, konami_claimed).
    pub fn get_user_by_auth_token(
        &self,
        auth_token: &str,
    ) -> Result<Option<(String, String, String, Option<String>, Option<String>, bool)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn
            .query_row(
                "SELECT id, email, name, picture_url, device_id, konami_claimed FROM users WHERE auth_token = ?1",
                params![auth_token],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, i32>(5)? != 0,
                    ))
                },
            )
            .ok();
        Ok(result)
    }

    /// Claim the konami code bonus for a user. Returns true if successfully claimed, false if already used.
    pub fn claim_konami(&self, user_id: &str) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        let affected = conn
            .execute(
                "UPDATE users SET konami_claimed = 1, updated_at = ?1 WHERE id = ?2 AND konami_claimed = 0",
                params![now, user_id],
            )
            .map_err(|e| format!("Claim konami: {e}"))?;
        if affected > 0 {
            info!(user_id = %user_id, "Konami code claimed");
        }
        Ok(affected > 0)
    }

    // --- Enrichment & Popularity ---

    /// Increment view count for an article and update popularity score.
    pub fn increment_view_count(&self, article_id: &str) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE articles SET view_count = view_count + 1 WHERE id = ?1",
            params![article_id],
        )
        .map_err(|e| format!("Increment view: {e}"))?;

        // Update popularity score: view_count * 0.7 + click_count * 0.3
        conn.execute(
            "UPDATE articles SET popularity_score = view_count * 0.7 + click_count * 0.3 WHERE id = ?1",
            params![article_id],
        )
        .map_err(|e| format!("Update popularity: {e}"))?;

        let view_count: i64 = conn
            .query_row(
                "SELECT view_count FROM articles WHERE id = ?1",
                params![article_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Get view count: {e}"))?;
        Ok(view_count)
    }

    /// Increment click count for an article and update popularity score.
    pub fn increment_click_count(&self, article_id: &str) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE articles SET click_count = click_count + 1 WHERE id = ?1",
            params![article_id],
        )
        .map_err(|e| format!("Increment click: {e}"))?;

        // Update popularity score
        conn.execute(
            "UPDATE articles SET popularity_score = view_count * 0.7 + click_count * 0.3 WHERE id = ?1",
            params![article_id],
        )
        .map_err(|e| format!("Update popularity: {e}"))?;

        let click_count: i64 = conn
            .query_row(
                "SELECT click_count FROM articles WHERE id = ?1",
                params![article_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Get click count: {e}"))?;
        Ok(click_count)
    }

    /// Get popular articles by percentile range (e.g., top 10-20%).
    /// Returns articles with popularity_score in the specified percentile range, ordered by score DESC.
    pub fn get_popular_articles(&self, min_percentile: f64, max_percentile: f64, limit: i64) -> Result<Vec<Article>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Get total article count
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM articles WHERE popularity_score > 0", [], |row| row.get(0))
            .unwrap_or(0);

        if total == 0 {
            return Ok(Vec::new());
        }

        // Calculate offset and limit based on percentiles
        let skip = ((100.0 - max_percentile) / 100.0 * total as f64).floor() as i64;
        let take = (((max_percentile - min_percentile) / 100.0 * total as f64).ceil() as i64).min(limit);

        let mut stmt = conn
            .prepare(
                "SELECT id, category, title, url, description, image_url, source,
                        published_at, fetched_at, group_id, group_count
                 FROM articles
                 WHERE popularity_score > 0
                 ORDER BY popularity_score DESC, published_at DESC
                 LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| e.to_string())?;

        let articles = stmt
            .query_map(params![take, skip], row_to_article)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(articles)
    }

    /// Update enrichment status for an article.
    pub fn update_enrichment_status(&self, article_id: &str, status: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE articles SET enrichment_status = ?1, enriched_at = ?2 WHERE id = ?3",
            params![status, now, article_id],
        )
        .map_err(|e| format!("Update enrichment status: {e}"))?;
        Ok(())
    }

    /// Create an enrichment record.
    pub fn create_enrichment(
        &self,
        enrichment_id: &str,
        article_id: &str,
        agent_type: &str,
        content_type: &str,
        data_json: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO enrichments (enrichment_id, article_id, agent_type, content_type, data_json, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6)",
            params![enrichment_id, article_id, agent_type, content_type, data_json, now],
        )
        .map_err(|e| format!("Create enrichment: {e}"))?;
        info!(enrichment_id, article_id, agent_type, "Enrichment created");
        Ok(())
    }

    /// Update enrichment status.
    pub fn update_enrichment(
        &self,
        enrichment_id: &str,
        status: &str,
        data_json: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();

        if let Some(data) = data_json {
            conn.execute(
                "UPDATE enrichments SET status = ?1, data_json = ?2, completed_at = ?3, error_message = ?4 WHERE enrichment_id = ?5",
                params![status, data, now, error_message, enrichment_id],
            )
        } else {
            conn.execute(
                "UPDATE enrichments SET status = ?1, completed_at = ?2, error_message = ?3 WHERE enrichment_id = ?4",
                params![status, now, error_message, enrichment_id],
            )
        }
        .map_err(|e| format!("Update enrichment: {e}"))?;
        Ok(())
    }

    /// Get all enrichments for an article.
    pub fn get_enrichments(&self, article_id: &str) -> Result<Vec<(String, String, String, String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT enrichment_id, agent_type, content_type, data_json, status
                 FROM enrichments
                 WHERE article_id = ?1 AND status = 'completed'
                 ORDER BY created_at DESC",
            )
            .map_err(|e| e.to_string())?;

        let enrichments = stmt
            .query_map(params![article_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(enrichments)
    }

    /// Degrade images for old unpopular articles (older than hours_old, below median popularity).
    pub fn degrade_old_unpopular_images(&self, hours_old: i64) -> Result<usize, String> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(hours_old)).to_rfc3339();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Get median popularity score for old articles
        let median_score: f64 = conn
            .query_row(
                "SELECT popularity_score FROM articles
                 WHERE published_at < ?1 AND popularity_score > 0
                 ORDER BY popularity_score
                 LIMIT 1 OFFSET (SELECT COUNT(*) FROM articles WHERE published_at < ?1 AND popularity_score > 0) / 2",
                params![cutoff],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        // Degrade images for articles below median popularity
        let degraded = conn
            .execute(
                "UPDATE articles
                 SET image_url = NULL
                 WHERE published_at < ?1
                 AND popularity_score < ?2
                 AND popularity_score > 0
                 AND image_url IS NOT NULL",
                params![cutoff, median_score],
            )
            .map_err(|e| format!("Degrade images: {}", e))?;

        Ok(degraded)
    }

    /// Delete bottom 80% of articles older than days_old (keep top 20% by popularity).
    pub fn cleanup_old_articles_bottom_80(&self, days_old: i64) -> Result<usize, String> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days_old)).to_rfc3339();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Get 20th percentile popularity score for old articles
        let percentile_20_score: f64 = conn
            .query_row(
                "SELECT popularity_score FROM articles
                 WHERE published_at < ?1
                 ORDER BY popularity_score DESC
                 LIMIT 1 OFFSET (SELECT COUNT(*) * 20 / 100 FROM articles WHERE published_at < ?1)",
                params![cutoff],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        // Delete bottom 80% (below 20th percentile)
        let deleted = conn
            .execute(
                "DELETE FROM articles
                 WHERE published_at < ?1
                 AND popularity_score < ?2",
                params![cutoff, percentile_20_score],
            )
            .map_err(|e| format!("Delete old articles: {}", e))?;

        Ok(deleted)
    }

    /// Get articles pending enrichment.
    pub fn get_pending_enrichment_articles(&self, limit: i64) -> Result<Vec<Article>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, category, title, url, description, image_url, source,
                        published_at, fetched_at, group_id, group_count
                 FROM articles
                 WHERE enrichment_status = 'pending'
                 ORDER BY popularity_score DESC, published_at DESC
                 LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;

        let articles = stmt
            .query_map(params![limit], row_to_article)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(articles)
    }

    /// Get fresh articles within specified time window (in minutes).
    pub fn get_fresh_articles(
        &self,
        category: Option<&Category>,
        minutes: i64,
        limit: i64,
    ) -> Result<Vec<Article>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::minutes(minutes))
            .to_rfc3339();

        let sql = if category.is_some() {
            "SELECT id, category, title, url, description, image_url, source,
                    published_at, fetched_at, group_id, group_count
             FROM articles
             WHERE category = ?1 AND published_at >= ?2
             ORDER BY published_at DESC
             LIMIT ?3"
        } else {
            "SELECT id, category, title, url, description, image_url, source,
                    published_at, fetched_at, group_id, group_count
             FROM articles
             WHERE published_at >= ?1
             ORDER BY published_at DESC
             LIMIT ?2"
        };

        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

        let articles = if let Some(cat) = category {
            stmt.query_map(params![cat.as_str(), cutoff, limit], row_to_article)
        } else {
            stmt.query_map(params![cutoff, limit], row_to_article)
        }
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        Ok(articles)
    }

    // --- AI Analysis ---

    /// Get articles that need AI analysis (not yet analyzed)
    pub fn get_articles_for_analysis(&self, limit: i64) -> Result<Vec<Article>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, category, title, url, description, image_url, source,
                        published_at, fetched_at, group_id, group_count
                 FROM articles
                 WHERE analyzed_at IS NULL
                   AND description IS NOT NULL
                   AND length(description) > 10
                 ORDER BY published_at DESC
                 LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;

        let articles = stmt
            .query_map(params![limit], row_to_article)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(articles)
    }

    /// Update article with AI analysis results
    pub fn update_article_analysis(
        &self,
        article_id: &str,
        summary: &str,
        keywords: &[String],
        sentiment: &str,
        importance: f32,
        category: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let keywords_json = serde_json::to_string(keywords)
            .map_err(|e| format!("Failed to serialize keywords: {}", e))?;

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE articles
             SET ai_summary = ?1,
                 ai_keywords = ?2,
                 ai_sentiment = ?3,
                 ai_importance = ?4,
                 ai_category = ?5,
                 analyzed_at = ?6
             WHERE id = ?7",
            params![
                summary,
                keywords_json,
                sentiment,
                importance,
                category,
                now,
                article_id
            ],
        )
        .map_err(|e| format!("Failed to update analysis: {}", e))?;

        Ok(())
    }

    /// Get analysis statistics
    pub fn get_analysis_stats(&self) -> Result<(i64, i64), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM articles", [], |row| row.get(0))
            .unwrap_or(0);

        let analyzed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM articles WHERE analyzed_at IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok((total, analyzed))
    }
}

fn row_to_article(row: &rusqlite::Row) -> rusqlite::Result<Article> {
    let cat_str: String = row.get(1)?;
    let category = Category::from_str(&cat_str).unwrap_or(Category::General);
    let pub_str: String = row.get(7)?;
    let fetch_str: String = row.get(8)?;
    let published_at: DateTime<Utc> = pub_str.parse().unwrap_or_default();
    let fetched_at: DateTime<Utc> = fetch_str.parse().unwrap_or_default();

    Ok(Article {
        id: row.get(0)?,
        category,
        title: row.get(2)?,
        url: row.get(3)?,
        description: row.get(4)?,
        image_url: row.get(5)?,
        source: row.get(6)?,
        published_at,
        fetched_at,
        group_id: row.get(9)?,
        group_count: row.get(10)?,
    })
}

fn encode_cursor(article: &Article) -> String {
    use base64::Engine;
    let json = serde_json::json!({
        "p": article.published_at.to_rfc3339(),
        "i": article.id,
    });
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.to_string().as_bytes())
}

fn decode_cursor(cursor: &str) -> Option<(String, String)> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let p = v.get("p")?.as_str()?.to_string();
    let i = v.get("i")?.as_str()?.to_string();
    Some((p, i))
}
