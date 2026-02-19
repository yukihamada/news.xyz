/**
 * i18n.js — Lightweight internationalization (EN/JA)
 * No framework — plain IIFE matching existing patterns.
 * Language priority: URL ?lang= > localStorage hn_lang > navigator.language > 'en'
 */
'use strict';

const I18n = (() => {
  const SUPPORTED = ['en', 'ja'];
  const DEFAULT_LANG = 'en';

  const DICT = {
    // Common UI
    'all':              { en: 'All', ja: 'すべて' },
    'loading':          { en: 'Loading...', ja: '読み込み中...' },
    'load_more':        { en: 'Load more', ja: 'もっと読む' },
    'loading_more':     { en: 'Loading', ja: '読み込み中' },
    'no_articles':      { en: 'No articles found', ja: '記事が見つかりません' },
    'skip_to_articles': { en: 'Skip to articles', ja: '記事へスキップ' },
    'top_10':           { en: 'TOP 10', ja: 'トップ10' },
    'archive':          { en: 'ARCHIVE', ja: 'アーカイブ' },

    // Time
    'time.just_now':    { en: 'just now', ja: 'たった今' },
    'time.mins_ago':    { en: '{{n}}m ago', ja: '{{n}}分前' },
    'time.hours_ago':   { en: '{{n}}h ago', ja: '{{n}}時間前' },
    'time.days_ago':    { en: '{{n}}d ago', ja: '{{n}}日前' },

    // Header
    'search':           { en: 'Search', ja: '検索' },
    'search.placeholder': { en: 'Search articles...', ja: '記事を検索...' },
    'search.shortcut':  { en: 'Search (Ctrl+K)', ja: '検索 (Ctrl+K)' },
    'bookmarks':        { en: 'Bookmarks', ja: 'ブックマーク' },
    'bookmarks.empty':  { en: 'No bookmarked articles', ja: 'ブックマークした記事はありません' },
    'bookmarks.clear':  { en: 'Clear all', ja: 'すべて削除' },
    'settings':         { en: 'Settings', ja: '設定' },
    'about':            { en: 'About', ja: 'About' },
    'mode_toggle':      { en: 'Toggle dark/light mode', ja: 'ダーク/ライトモード切替' },
    'chat_open':        { en: 'Open settings chat', ja: '設定チャットを開く' },
    'close':            { en: 'Close', ja: '閉じる' },
    'clear':            { en: 'Clear', ja: 'クリア' },

    // Article
    'tts.read_aloud':   { en: 'Read aloud', ja: '読み上げ' },
    'bookmark':         { en: 'Bookmark', ja: 'ブックマーク' },
    'group_badge':      { en: '+{{n}} related', ja: '+{{n}}件の関連記事' },
    'url_copied':       { en: 'URL copied', ja: 'URLをコピーしました' },

    // Detail panel
    'detail.article':                  { en: 'Article detail', ja: '記事詳細' },
    'detail.back':                     { en: 'Back', ja: '戻る' },
    'detail.generating_questions':     { en: 'Generating questions...', ja: '質問を生成中...' },
    'detail.generating_questions_short': { en: 'Generating questions', ja: '質問を生成中' },
    'detail.questions_failed':         { en: 'Failed to generate questions', ja: '質問を生成できませんでした' },
    'detail.questions_error':          { en: 'Failed to generate questions ', ja: '質問の生成に失敗しました ' },
    'detail.generating_answer':        { en: 'Generating answer', ja: '回答を生成中' },
    'detail.answer_failed':            { en: 'Failed to get answer', ja: '回答の取得に失敗しました' },
    'detail.answer_error':             { en: 'Failed to get answer ', ja: '回答の取得に失敗しました ' },
    'detail.retry':                    { en: 'Retry', ja: '再試行' },
    'detail.offline':                  { en: 'AI features unavailable offline', ja: 'オフラインのためAI機能は利用できません' },
    'detail.related':                  { en: 'Related articles', ja: '関連記事' },
    'detail.read_original':            { en: 'Read original', ja: '元記事を読む' },
    'detail.share':                    { en: 'Share', ja: '共有' },
    'detail.open_original':            { en: 'Open original article', ja: '元記事を開く' },

    // Search
    'search.failed':    { en: 'Search failed', ja: '検索に失敗しました' },
    'search.no_results': { en: 'No results', ja: '結果なし' },

    // Error
    'error.load_failed': { en: 'Failed to load. Please try again.', ja: 'ニュースの読み込みに失敗しました。後ほどお試しください。' },

    // EcoSystem
    'eco.pro_unlimited':   { en: 'Pro: Unlimited', ja: 'Pro: 無制限' },
    'eco.cache_hit':       { en: 'Cache hit rate', ja: 'キャッシュヒット率' },
    'eco.tokens_daily':    { en: 'AI tokens (+{{n}}/day)', ja: 'AIトークン（毎日+{{n}}）' },
    'eco.tokens_short':    { en: 'Not enough tokens.', ja: 'トークン不足です。' },
    'eco.pro_unlimited_link': { en: 'Upgrade to Pro for unlimited.', ja: 'Proプランで無制限に。' },

    // Subscription
    'sub.upgrade_google':      { en: 'Sign in with Google for 2x limits', ja: 'Googleログインで制限2倍' },
    'sub.upgrade_pro':         { en: 'Upgrade to Pro for unlimited', ja: 'Proで無制限に' },
    'sub.limit_reached':       { en: '{{feature}} daily limit ({{limit}}) reached.', ja: '{{feature}}の本日の利用回数（{{limit}}回）に達しました。' },
    'sub.limit_google':        { en: 'Sign in with Google for 2x limits!', ja: 'Googleログインで制限が2倍に！' },
    'sub.limit_pro':           { en: 'Upgrade to Pro (¥500/mo) for unlimited.', ja: 'Proプラン（¥500/月）で無制限にご利用いただけます。' },
    'sub.upgrade_to_pro':      { en: 'Upgrade to Pro', ja: 'Proにアップグレード' },
    'sub.google_signin':       { en: 'Sign in with Google', ja: 'Googleでログイン' },
    'sub.pro_plan':            { en: 'Pro plan (unlimited)', ja: 'Proプラン（無制限）' },
    'sub.pro_complete':        { en: 'Pro plan upgrade complete! Enjoy unlimited AI features.', ja: 'Proプランへのアップグレードが完了しました！AI機能が無制限でご利用いただけます。' },
    'sub.pro_verifying':       { en: 'Verifying Pro plan activation. Please wait.', ja: 'Proプランの有効化を確認中です。しばらくお待ちください。' },
    'sub.not_ready':           { en: 'Subscription is not available yet. Please try again later.', ja: 'サブスクリプション機能は現在準備中です。もうしばらくお待ちください。' },
    'sub.billing_not_ready':   { en: 'Billing management is not available yet. Please try again later.', ja: '課金管理機能は現在準備中です。もうしばらくお待ちください。' },

    // Meta
    'meta.description':    { en: 'AI-powered ultra-fast news aggregator with AI summaries, Q&A, and voice reading.', ja: 'AI搭載の超高速ニュースアグリゲーター。AIによる要約・質問応答・読み上げ。' },
    'meta.title_suffix':   { en: 'AI-Powered News', ja: 'AI超高速ニュース' },
    'meta.cat_news':       { en: '{{cat}} News', ja: '{{cat}}ニュース' },
    'meta.cat_desc':       { en: 'Latest {{cat}} news with AI summaries and Q&A.', ja: '{{cat}}カテゴリの最新ニュースをAIが要約・質問応答。' },
    'meta.default_desc':   { en: 'AI-powered ultra-fast news aggregator. Latest news with AI summaries, Q&A, and voice reading.', ja: 'AI搭載の超高速ニュースアグリゲーター。最新ニュースをAIが要約・質問応答・読み上げ。' },
    'meta.latest_news':    { en: '{{site}} Latest News', ja: '{{site}} 最新ニュース' },

    // Footer
    'footer.sister_sites': { en: 'Sister Sites', ja: '姉妹サイト' },

    // TTS
    'tts.select_voice':    { en: 'Select a voice to use TTS.', ja: '読み上げを使うには、ボイスを選んでください。' },

    // Chat
    'chat.settings_assistant': { en: 'Settings Assistant', ja: '設定アシスタント' },
    'chat.placeholder':        { en: 'e.g. Switch to dark mode', ja: '例: ダークモードにして' },
    'chat.settings_chat':      { en: 'Settings chat', ja: '設定チャット' },
    'chat.welcome':            { en: 'Choose from 8 themes. Try random too!', ja: '8種類のテーマから選べます。ランダムもどうぞ！' },

    // Site descriptions (for footer)
    'site.desc.xyz':       { en: 'AI News', ja: 'AIニュース' },
    'site.desc.online':    { en: 'Voice Feed', ja: '音声ニュース' },
    'site.desc.chatnews':  { en: 'Chat News', ja: 'チャット' },
    'site.desc.yournews':  { en: 'Personal', ja: 'パーソナル' },
    'site.desc.cloud':     { en: 'API', ja: 'API' },
    'site.desc.velo':      { en: 'Performance', ja: '速度計測' },
    'site.desc.claud':     { en: 'Claude AI News', ja: 'AIニュース' },
  };

  let currentLang = DEFAULT_LANG;

  function init() {
    const urlLang = new URLSearchParams(location.search).get('lang');
    if (urlLang && SUPPORTED.includes(urlLang)) {
      currentLang = urlLang;
      Storage.set('lang', urlLang);
    } else {
      const stored = Storage.get('lang');
      if (stored && SUPPORTED.includes(stored)) {
        currentLang = stored;
      } else {
        currentLang = detectBrowserLang();
        Storage.set('lang', currentLang);
      }
    }
    document.documentElement.lang = currentLang;
    translateDOM();
  }

  function detectBrowserLang() {
    const langs = navigator.languages || [navigator.language || 'en'];
    for (const l of langs) {
      const short = l.split('-')[0].toLowerCase();
      if (SUPPORTED.includes(short)) return short;
    }
    return DEFAULT_LANG;
  }

  function t(key, params) {
    const entry = DICT[key];
    if (!entry) return key;
    let str = entry[currentLang] || entry[DEFAULT_LANG] || key;
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        str = str.replaceAll('{{' + k + '}}', String(v));
      }
    }
    return str;
  }

  function categoryLabel(cat) {
    return currentLang === 'ja' ? (cat.label_ja || cat.label) : (cat.label || cat.label_ja);
  }

  function relativeTime(isoStr) {
    const diff = Date.now() - new Date(isoStr).getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 1) return t('time.just_now');
    if (mins < 60) return t('time.mins_ago', { n: mins });
    const hrs = Math.floor(mins / 60);
    if (hrs < 24) return t('time.hours_ago', { n: hrs });
    const days = Math.floor(hrs / 24);
    if (days < 7) return t('time.days_ago', { n: days });
    const locale = currentLang === 'ja' ? 'ja-JP' : 'en-US';
    return new Date(isoStr).toLocaleDateString(locale);
  }

  function setLang(lang) {
    if (!SUPPORTED.includes(lang)) return;
    currentLang = lang;
    Storage.set('lang', lang);
    document.documentElement.lang = lang;
    translateDOM();
  }

  function getLang() { return currentLang; }
  function isJa() { return currentLang === 'ja'; }

  function translateDOM() {
    document.querySelectorAll('[data-i18n]').forEach(el => {
      el.textContent = t(el.dataset.i18n);
    });
    document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
      el.placeholder = t(el.dataset.i18nPlaceholder);
    });
    document.querySelectorAll('[data-i18n-title]').forEach(el => {
      const val = t(el.dataset.i18nTitle);
      el.title = val;
      el.setAttribute('aria-label', val);
    });
  }

  return { init, t, setLang, getLang, isJa, categoryLabel, relativeTime, translateDOM, SUPPORTED };
})();
const t = I18n.t;
