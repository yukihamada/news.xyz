use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Article categories matching DynamoDB partition keys.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    General,
    Tech,
    Business,
    Entertainment,
    Sports,
    Science,
    Podcast,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Tech => "tech",
            Self::Business => "business",
            Self::Entertainment => "entertainment",
            Self::Sports => "sports",
            Self::Science => "science",
            Self::Podcast => "podcast",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "general" => Some(Self::General),
            "tech" => Some(Self::Tech),
            "business" => Some(Self::Business),
            "entertainment" => Some(Self::Entertainment),
            "sports" => Some(Self::Sports),
            "science" => Some(Self::Science),
            "podcast" => Some(Self::Podcast),
            _ => None,
        }
    }

    pub fn all() -> &'static [Category] {
        &[
            Self::General,
            Self::Tech,
            Self::Business,
            Self::Entertainment,
            Self::Sports,
            Self::Science,
            Self::Podcast,
        ]
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single news article.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: String,
    pub category: Category,
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    pub source: String,
    pub published_at: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_count: Option<u32>,
}

/// Paginated response for article listing.
#[derive(Debug, Serialize, Deserialize)]
pub struct ArticlesResponse {
    pub articles: Vec<Article>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Category info for /api/categories endpoint.
#[derive(Debug, Serialize)]
pub struct CategoryInfo {
    pub id: String,
    pub label: String,
    pub label_ja: String,
}

impl CategoryInfo {
    pub fn all() -> Vec<Self> {
        vec![
            Self { id: "general".into(), label: "General".into(), label_ja: "総合".into() },
            Self { id: "tech".into(), label: "Technology".into(), label_ja: "テクノロジー".into() },
            Self { id: "business".into(), label: "Business".into(), label_ja: "ビジネス".into() },
            Self { id: "entertainment".into(), label: "Entertainment".into(), label_ja: "エンタメ".into() },
            Self { id: "sports".into(), label: "Sports".into(), label_ja: "スポーツ".into() },
            Self { id: "science".into(), label: "Science".into(), label_ja: "サイエンス".into() },
            Self { id: "podcast".into(), label: "Podcast".into(), label_ja: "ポッドキャスト".into() },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_roundtrip() {
        for cat in Category::all() {
            let s = cat.as_str();
            let parsed = Category::from_str(s).unwrap();
            assert_eq!(*cat, parsed);
        }
    }

    #[test]
    fn category_from_str_case_insensitive() {
        assert_eq!(Category::from_str("TECH"), Some(Category::Tech));
        assert_eq!(Category::from_str("General"), Some(Category::General));
        assert_eq!(Category::from_str("unknown"), None);
    }
}
