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
- `{"type":"add_category","id":"lifestyle","label_ja":"ライフスタイル"}`
- `{"type":"remove_category","id":"sports"}`
- `{"type":"rename_category","id":"tech","label_ja":"IT・テック"}`
- `{"type":"reorder_categories","order":["tech","general","business","entertainment","sports","science"]}`

## ルール
- 日本語でも英語でも対応
- 「NHK以外を増やして」→ 著名なRSSフィードを提案（朝日新聞デジタル、毎日新聞、ITmedia、GIGAZINE等）
- 「同じようなニュースをまとめて」→ grouping機能を有効化
- 「写真を入れて」「画像を表示して」→ ogp_enrichment機能を有効化
- 「カテゴリを追加して」→ add_categoryで新カテゴリ追加（idは英語小文字、label_jaは日本語名）
- 「スポーツを消して」→ remove_categoryでカテゴリ削除
- 「テクノロジーをIT・テックに変更して」→ rename_categoryで名前変更
- 「テクノロジーを一番前にして」→ reorder_categoriesで並び替え
- 不明確なコマンドにはconfidence 0.5以下で説明のみ返す

## 出力フォーマット（厳密にこの形式のJSONのみ出力。コードブロック不要）

{"confidence":0.9,"interpretation":"NHK以外の日本語ニュースフィードを追加します","actions":[{"type":"add_feed","url":"https://rss.itmedia.co.jp/rss/2.0/itmedia_all.xml","source":"ITmedia","category":"tech"}]}"#;

pub async fn summarize_articles(
    client: &reqwest::Client,
    api_key: &str,
    articles: &[(String, String)],
    target_chars: usize,
) -> Result<String, String> {
    let article_list = articles
        .iter()
        .enumerate()
        .map(|(i, (title, source))| format!("{}. [{}] {}", i + 1, source, title))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "あなたはプロのニュースキャスターです。以下のニュース一覧を、約{}文字の日本語で自然にまとめて読み上げ原稿を作成してください。\n\n\
        ルール:\n\
        - ニュースキャスターが読み上げるような、聞き取りやすく自然な口語体で書く\n\
        - 重要なニュースを優先し、関連するニュースはまとめて紹介する\n\
        - 各トピックについて、想定される様々な立場からの見方や意見も織り交ぜて多角的に紹介する\n\
        - 「専門家の間では〜という見方もあります」「一方で〜という意見も」のように多様な視点を提示する\n\
        - 冒頭に簡単な挨拶、最後に締めの一言を入れる\n\
        - 原稿のテキストのみ出力（JSONやマークダウン不要）\n\n\
        ## ニュース一覧\n{}",
        target_chars, article_list
    );

    let request = ClaudeRequest {
        model: "claude-sonnet-4-5-20250929".into(),
        max_tokens: (target_chars as u32) * 2,
        messages: vec![ClaudeMessage {
            role: "user".into(),
            content: prompt,
        }],
    };

    info!(articles = articles.len(), target_chars, "Generating news summary");

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
        warn!(status = %status, body = %body, "Claude API error (summarize)");
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

    info!(chars = text.len(), "News summary generated");
    Ok(text.trim().to_string())
}

pub async fn generate_questions(
    client: &reqwest::Client,
    api_key: &str,
    title: &str,
    description: &str,
    source: &str,
    article_content: &str,
    custom_prompt: Option<&str>,
) -> Result<Vec<String>, String> {
    let content_section = if article_content.is_empty() {
        String::new()
    } else {
        format!("\n\n## 記事本文\n{}", article_content)
    };
    let custom_section = match custom_prompt {
        Some(p) if !p.is_empty() => format!("\n\n## 追加指示\n{}", p),
        _ => String::new(),
    };
    let prompt = format!(
        "以下のニュース記事について、読者が知りたいと思う質問を4つ生成してください。\n\n\
        ルール:\n\
        - 記事本文の情報を踏まえた具体的な質問を生成する\n\
        - 記事の内容を深掘りする興味深い質問\n\
        - 背景や影響、今後の展望に関する質問を含める\n\
        - 短く簡潔な質問文（20文字以内が理想）\n\
        - JSON配列のみ出力: [\"質問1\", \"質問2\", \"質問3\", \"質問4\"]\n\n\
        ## 記事\nタイトル: {}\nソース: {}\n概要: {}{}{}",
        title, source, description, content_section, custom_section
    );

    let request = ClaudeRequest {
        model: "claude-sonnet-4-5-20250929".into(),
        max_tokens: 512,
        messages: vec![ClaudeMessage {
            role: "user".into(),
            content: prompt,
        }],
    };

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
        warn!(status = %status, body = %body, "Claude API error (questions)");
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

    let clean = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let questions: Vec<String> = serde_json::from_str(clean)
        .map_err(|e| format!("Failed to parse questions: {} — raw: {}", e, text))?;

    Ok(questions)
}

/// Transform a potentially negative question into a positive, constructive one.
pub async fn transform_question_to_positive(
    client: &reqwest::Client,
    api_key: &str,
    question: &str,
) -> Result<String, String> {
    // Quick check: if question is already positive, return as-is
    let negative_keywords = ["なぜダメ", "なぜ悪い", "問題", "失敗", "批判", "ひどい", "最悪"];
    let has_negative = negative_keywords.iter().any(|&kw| question.contains(kw));

    if !has_negative && question.len() < 100 {
        return Ok(question.to_string());
    }

    let prompt = format!(
        "以下の質問をポジティブでシンプルな質問に変換してください。\n\n\
        ルール:\n\
        - ネガティブな表現（批判、問題点、失敗等）をポジティブな問いに変換\n\
        - 「なぜダメか」→「どうすればよいか」「どのような改善策があるか」\n\
        - 「なぜ失敗したか」→「成功のために何が必要か」「次にどう活かすか」\n\
        - シンプルで建設的な質問にする（50文字以内）\n\
        - 変換後の質問テキストのみ出力（説明不要）\n\n\
        元の質問: {}",
        question
    );

    let request = ClaudeRequest {
        model: "claude-haiku-4-5-20251001".into(),
        max_tokens: 256,
        messages: vec![ClaudeMessage {
            role: "user".into(),
            content: prompt,
        }],
    };

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
        // If transformation fails, return original question
        warn!("Question transformation failed, using original");
        return Ok(question.to_string());
    }

    let claude_response: ClaudeResponse = response
        .json()
        .await
        .map_err(|_| "Failed to parse response".to_string())?;

    let transformed = claude_response
        .content
        .first()
        .and_then(|b| b.text.as_ref())
        .map(|t| t.trim().to_string())
        .unwrap_or_else(|| question.to_string());

    info!(
        original = %question,
        transformed = %transformed,
        "Question transformed to positive"
    );

    Ok(transformed)
}

pub async fn answer_question(
    client: &reqwest::Client,
    api_key: &str,
    title: &str,
    description: &str,
    source: &str,
    question: &str,
    article_content: &str,
    custom_prompt: Option<&str>,
) -> Result<String, String> {
    let content_section = if article_content.is_empty() {
        String::new()
    } else {
        format!("\n\n## 記事本文\n{}", article_content)
    };
    let custom_section = match custom_prompt {
        Some(p) if !p.is_empty() => format!("\n\n## 追加指示\n{}", p),
        _ => String::new(),
    };
    let prompt = format!(
        "以下のニュース記事に関する質問に、わかりやすく具体的に回答してください。\n\n\
        ルール:\n\
        - 300〜600文字程度で回答\n\
        - 記事本文を参照し、事実に基づいて具体的に回答する\n\
        - 不明な部分は一般的な知識で補完する\n\
        - 複数の視点や立場からの見方も紹介\n\
        - 回答テキストのみ出力（JSON不要）\n\n\
        ## 記事\nタイトル: {}\nソース: {}\n概要: {}{}\n\n## 質問\n{}{}",
        title, source, description, content_section, question, custom_section
    );

    let request = ClaudeRequest {
        model: "claude-sonnet-4-5-20250929".into(),
        max_tokens: 1536,
        messages: vec![ClaudeMessage {
            role: "user".into(),
            content: prompt,
        }],
    };

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
        warn!(status = %status, body = %body, "Claude API error (answer)");
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

    Ok(text.trim().to_string())
}

pub async fn convert_to_reading(
    client: &reqwest::Client,
    api_key: &str,
    text: &str,
    engine: &str,
) -> Result<String, String> {
    let is_qwen = matches!(engine, "qwen-tts" | "qwen-omni" | "cosyvoice");

    let prompt = if is_qwen {
        format!(
            "以下のテキストを、日本語TTSエンジンで自然に読み上げられるよう前処理してください。\n\n\
            ## 最重要\n\
            **出力は必ず日本語にしてください。**英語やその他の言語のテキストは、自然な日本語に翻訳してから出力してください。\n\n\
            ## ルール\n\
            1. **英語・外国語の記事は日本語に翻訳する** — 内容を正確に保ちながら自然な日本語に変換\n\
            2. **漢字はそのまま残す** — 一般的な漢字語彙はすべて漢字のまま（例: 経済、政府、発表）\n\
            3. **略語はカタカナ展開** — NATO→ナトー、GDP→ジーディーピー、AI→エーアイ\n\
            4. **数字は日本語読み** — 100万円→百万円、3.5%→3.5パーセント\n\
            5. **URL・メールアドレスを除去** — 読み上げに不要\n\
            6. **長い括弧補足を除去** — 文の流れを妨げる括弧注記は削除\n\
            7. **記号を句読点に** — 「・」→読点、「／」→読点、「→」→は に変換\n\
            8. **カタカナはそのまま** — 外来語のカタカナ表記は変更しない\n\
            9. **変換後のテキストのみ出力** — 説明や注釈は不要\n\n\
            ## テキスト\n{}",
            text
        )
    } else {
        format!(
            "以下のテキストを、ElevenLabs TTSエンジンで高品質に読み上げられるよう前処理してください。\n\n\
            ## 重要な方針\n\
            ElevenLabsのmultilingual v2モデルは漢字からアクセントや抑揚を判定します。\n\
            そのため**漢字はそのまま残す**ことが最も重要です。ひらがなに変換しないでください。\n\n\
            ## ルール\n\
            1. **漢字はそのまま残す** — 一般的な漢字語彙はすべて漢字のまま（例: 経済、政府、発表）\n\
            2. **難読固有名詞のみ括弧で読み補足** — 例: 石破（いしば）茂、枚方（ひらかた）市\n\
            3. **略語はカタカナ展開** — NATO→ナトー、GDP→ジーディーピー、AI→エーアイ、WHO→ダブリューエイチオー\n\
            4. **数字は日本語読み** — 100万円→百万円、3.5%→3.5パーセント、2024年→2024年（そのまま）\n\
            5. **URL・メールアドレスを除去** — 読み上げに不要\n\
            6. **長い括弧補足を除去** — 文の流れを妨げる括弧注記は削除\n\
            7. **記号を句読点に** — 「・」→読点、「／」→読点、「→」→は に変換\n\
            8. **カタカナはそのまま** — 外来語のカタカナ表記は変更しない\n\
            9. **変換後のテキストのみ出力** — 説明や注釈は不要\n\n\
            ## テキスト\n{}",
            text
        )
    };

    let request = ClaudeRequest {
        model: "claude-haiku-4-5-20251001".into(),
        max_tokens: (text.len() as u32) * 2 + 256,
        messages: vec![ClaudeMessage {
            role: "user".into(),
            content: prompt,
        }],
    };

    info!(chars = text.len(), "Converting text for TTS preprocessing");

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
        warn!(status = %status, body = %body, "Claude API error (to-reading)");
        return Err(format!("Claude API error: {} - {}", status, body));
    }

    let claude_response: ClaudeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Claude response: {}", e))?;

    let result = claude_response
        .content
        .first()
        .and_then(|b| b.text.as_ref())
        .ok_or_else(|| "Empty response from Claude".to_string())?;

    info!(chars = result.len(), "Reading conversion complete");
    Ok(result.trim().to_string())
}

// --- Podcast Dialogue ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DialogueLine {
    pub speaker: String,  // "host" | "analyst"
    pub text: String,
}

pub async fn generate_dialogue_script(
    client: &reqwest::Client,
    api_key: &str,
    title: &str,
    description: &str,
    source: &str,
    article_content: &str,
) -> Result<Vec<DialogueLine>, String> {
    let content_section = if article_content.is_empty() {
        String::new()
    } else {
        format!("\n\n## 記事本文\n{}", &article_content[..article_content.len().min(3000)])
    };

    let prompt = format!(
        "以下のニュース記事について、2人の対話形式のポッドキャスト台本を生成してください。\n\n\
        ## 登場人物\n\
        - host: 番組ホスト。親しみやすく、わかりやすく話す。\n\
        - analyst: 解説者。専門的な視点で補足・分析する。\n\n\
        ## ルール\n\
        - 8〜12行の対話（合計800〜1200文字）\n\
        - 60〜90秒で読み上げられる長さ\n\
        - hostが話題を振り、analystが解説する流れ\n\
        - 冒頭でニュースの要点を紹介、中盤で深掘り、最後に展望やまとめ\n\
        - 自然な口語体（「〜ですね」「〜なんですよ」など）\n\
        - JSON配列のみ出力: [{{\"speaker\":\"host\",\"text\":\"...\"}},{{\"speaker\":\"analyst\",\"text\":\"...\"}},...]\n\n\
        ## 記事\nタイトル: {}\nソース: {}\n概要: {}{}",
        title, source, description, content_section
    );

    let request = ClaudeRequest {
        model: "claude-sonnet-4-5-20250929".into(),
        max_tokens: 2048,
        messages: vec![ClaudeMessage {
            role: "user".into(),
            content: prompt,
        }],
    };

    info!(title = %title, "Generating dialogue script");

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
        warn!(status = %status, body = %body, "Claude API error (dialogue)");
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

    let clean = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let dialogue: Vec<DialogueLine> = serde_json::from_str(clean)
        .map_err(|e| format!("Failed to parse dialogue: {} — raw: {}", e, text))?;

    info!(lines = dialogue.len(), "Dialogue script generated");
    Ok(dialogue)
}

pub async fn generate_murmur(
    client: &reqwest::Client,
    api_key: &str,
    title: &str,
    description: &str,
    source: &str,
) -> Result<String, String> {
    let prompt = format!(
        "以下のニュース記事について、カジュアルな独り言を80〜120文字、2〜3文でつぶやいてください。\n\n\
        ルール:\n\
        - 「へぇ〜」「マジか」「なるほど〜」「〜だよね」「すごいな〜」など口語体で\n\
        - ニュースキャスター調は禁止。友達に話すような砕けたトーン\n\
        - 自分の感想や驚き、ちょっとした疑問を自然に\n\
        - テキストのみ出力（JSON不要、引用符不要）\n\n\
        ## 記事\nタイトル: {}\nソース: {}\n概要: {}",
        title, source, description
    );

    let request = ClaudeRequest {
        model: "claude-haiku-4-5-20251001".into(),
        max_tokens: 256,
        messages: vec![ClaudeMessage {
            role: "user".into(),
            content: prompt,
        }],
    };

    info!(title = %title, "Generating murmur");

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
        warn!(status = %status, body = %body, "Claude API error (murmur)");
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

    info!(chars = text.len(), "Murmur generated");
    Ok(text.trim().to_string())
}

// --- Smart News Classification & Action Plans ---

#[derive(Debug, Serialize, Deserialize)]
pub struct ArticleClassification {
    pub category: String,  // "timemachine" | "goldmining" | "frustration" | "general"
    pub reasoning: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionPlan {
    pub summary: String,           // 1行サマリー
    pub steps: Vec<String>,        // 具体的なアクション3-5個
    pub tools_or_templates: Vec<String>,  // 使えるツール・テンプレート
}

/// 記事を「タイムマシン」「砂金掘り」「不満の可視化」に自動分類
pub async fn classify_article(
    client: &reqwest::Client,
    api_key: &str,
    title: &str,
    description: &str,
    source: &str,
    category: &str,
) -> Result<ArticleClassification, String> {
    let prompt = format!(
        "以下のニュース記事を分類してください。\n\n\
        ## 分類カテゴリ\n\
        1. **timemachine（情報のタイムマシン）**: 海外の最先端情報で、日本ではまだ知られていない。Product Hunt、Reddit、海外Substack、先進的な技術論文など\n\
        2. **goldmining（砂金掘り）**: 官公庁資料、特許、学術論文、SEC申請書類など、難解な一次情報を噛み砕いて解説\n\
        3. **frustration（不満の可視化）**: ユーザーの困りごと、不満、問題点を可視化。Yahoo!知恵袋、Xの不満ツイート、低評価レビューなど\n\
        4. **general（一般）**: 上記に当てはまらない通常のニュース\n\n\
        ## ルール\n\
        - 記事のソースとカテゴリを考慮して最も適切な分類を選ぶ\n\
        - reasoningに分類の理由を簡潔に（50文字以内）\n\
        - tagsに関連タグを2-3個（例: [\"AI\", \"スタートアップ\", \"日本未上陸\"]）\n\
        - JSON出力のみ: {{\"category\":\"...\",\"reasoning\":\"...\",\"tags\":[...]}}\n\n\
        ## 記事\nタイトル: {}\nソース: {}\nカテゴリ: {}\n概要: {}",
        title, source, category, description
    );

    let request = ClaudeRequest {
        model: "claude-haiku-4-5-20251001".into(),
        max_tokens: 256,
        messages: vec![ClaudeMessage {
            role: "user".into(),
            content: prompt,
        }],
    };

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
        warn!(status = %status, body = %body, "Claude API error (classify)");
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

    let clean = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let classification: ArticleClassification = serde_json::from_str(clean)
        .map_err(|e| format!("Failed to parse classification: {} — raw: {}", e, text))?;

    Ok(classification)
}

/// 「で、どうすればいい？」のアクションプランを生成
pub async fn generate_action_plan(
    client: &reqwest::Client,
    api_key: &str,
    title: &str,
    description: &str,
    article_content: &str,
    classification: &str,
) -> Result<ActionPlan, String> {
    let content_section = if article_content.is_empty() {
        String::new()
    } else {
        format!("\n\n## 記事本文\n{}", &article_content[..article_content.len().min(2000)])
    };

    let prompt = format!(
        "以下のニュース記事を読んだ人が「で、どうすればいい？」と思った時の具体的なアクションプランを生成してください。\n\n\
        ## ルール\n\
        - summary: 1行で「〇〇すべき」「〇〇をチェック」など具体的な指針（30文字以内）\n\
        - steps: 今すぐできる具体的なアクション3-5個（各50文字以内）\n\
        - tools_or_templates: 使えるツール、テンプレート、リンク先の提案2-3個\n\
        - 記事の分類（{}）を考慮する\n\
        - JSON出力のみ: {{\"summary\":\"...\",\"steps\":[...],\"tools_or_templates\":[...]}}\n\n\
        ## 記事\nタイトル: {}\n概要: {}{}",
        classification, title, description, content_section
    );

    let request = ClaudeRequest {
        model: "claude-sonnet-4-5-20250929".into(),
        max_tokens: 768,
        messages: vec![ClaudeMessage {
            role: "user".into(),
            content: prompt,
        }],
    };

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
        warn!(status = %status, body = %body, "Claude API error (action_plan)");
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

    let clean = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let action_plan: ActionPlan = serde_json::from_str(clean)
        .map_err(|e| format!("Failed to parse action plan: {} — raw: {}", e, text))?;

    Ok(action_plan)
}

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
        messages: vec![ClaudeMessage {
            role: "user".into(),
            content: format!("{}\n\n{}", SYSTEM_PROMPT, user_message),
        }],
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
