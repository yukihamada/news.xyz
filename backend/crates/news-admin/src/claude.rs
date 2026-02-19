use news_core::changes::AdminAction;
use news_core::config::ServiceConfig;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContentBlock {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommandInterpretation {
    pub confidence: f64,
    pub interpretation: String,
    pub actions: Vec<AdminAction>,
}

const SYSTEM_PROMPT: &str = r#"あなたはHyperNewsの管理AIです。ユーザーの自然言語コマンドを解釈し、ニュースサービスの設定変更アクションに変換します。

## アクション一覧（typeフィールドで識別、フラット構造）

- `{"type":"add_feed","url":"...","source":"...","category":"general|tech|business|entertainment|sports|science"}`
- `{"type":"remove_feed","feed_id":"..."}`
- `{"type":"enable_feed","feed_id":"..."}`
- `{"type":"disable_feed","feed_id":"..."}`
- `{"type":"toggle_feature","feature":"grouping|ogp_enrichment","enabled":true|false}`
- `{"type":"set_grouping_threshold","threshold":0.3}`

## ルール
- 日本語でも英語でも対応
- 「NHK以外を増やして」→ 著名なRSSフィードを提案（朝日新聞デジタル、毎日新聞、ITmedia、GIGAZINE等）
- 「同じようなニュースをまとめて」→ grouping機能を有効化
- 「写真を入れて」「画像を表示して」→ ogp_enrichment機能を有効化
- 不明確なコマンドにはconfidence 0.5以下で説明のみ返す

## 出力フォーマット（厳密にこの形式のJSONのみ出力。コードブロック不要）

{"confidence":0.9,"interpretation":"NHK以外の日本語ニュースフィードを追加します","actions":[{"type":"add_feed","url":"https://rss.itmedia.co.jp/rss/2.0/itmedia_all.xml","source":"ITmedia","category":"tech"}]}"#;

pub async fn interpret_command(
    client: &reqwest::Client,
    api_key: &str,
    command: &str,
    current_config: &ServiceConfig,
) -> Result<CommandInterpretation, String> {
    let config_json = serde_json::to_string_pretty(current_config)
        .map_err(|e| format!("Config serialization error: {}", e))?;

    let user_message = format!(
        "## 現在の設定\n```json\n{}\n```\n\n## ユーザーコマンド\n{}",
        config_json, command
    );

    let request = ClaudeRequest {
        model: "claude-sonnet-4-5-20250929".into(),
        max_tokens: 1024,
        messages: vec![
            ClaudeMessage {
                role: "user".into(),
                content: format!("{}\n\n{}", SYSTEM_PROMPT, user_message),
            },
        ],
    };

    info!(command = %command, "Sending command to Claude API");

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Claude API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        warn!(status = %status, body = %body, "Claude API error");
        return Err(format!("Claude API error: {} - {}", status, body));
    }

    let claude_response: ClaudeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Claude response: {}", e))?;

    let text = claude_response
        .content
        .first()
        .and_then(|b| b.text.as_ref())
        .ok_or_else(|| "Empty response from Claude".to_string())?;

    // Parse the JSON response, stripping any markdown code fences
    let clean_text = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let interpretation: CommandInterpretation = serde_json::from_str(clean_text)
        .map_err(|e| format!("Failed to parse Claude interpretation: {} — raw: {}", e, text))?;

    info!(
        confidence = interpretation.confidence,
        actions = interpretation.actions.len(),
        "Claude interpretation complete"
    );

    Ok(interpretation)
}
