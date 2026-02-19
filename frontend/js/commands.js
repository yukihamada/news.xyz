/**
 * commands.js — Chat command parser (Japanese/English)
 * Returns { action, response } or null if no command matched.
 */
'use strict';

const Commands = (() => {
  /**
   * Process a user message and return command result or null.
   */
  function process(input) {
    const msg = input.trim().toLowerCase();
    if (!msg) return null;

    // --- Summarize & speak ---
    {
      // "3分でまとめてしゃべって", "5分ニュース", "1分で要約して読んで", etc.
      const sumMatch = msg.match(/(\d+)\s*分.*(まとめ|要約).*(しゃべ|喋|読|話)/)
                     || msg.match(/(まとめ|要約).*(しゃべ|喋|読|話).*?(\d+)\s*分/)
                     || msg.match(/(\d+)\s*分\s*(ニュース|にゅーす)/)
                     || msg.match(/(\d+)\s*分.*(まとめ|要約|サマリ)/)
                     || msg.match(/(まとめ|要約|サマリ).*?(\d+)\s*分/);
      if (sumMatch) {
        const minutes = parseInt(sumMatch[1] || sumMatch[2] || sumMatch[3], 10);
        if (minutes > 0 && minutes <= 10) {
          return { action: 'summarize', minutes, response: `${minutes}分のニュース要約を生成中...` };
        }
      }
      // "今日のニュースまとめて" (default 3 min)
      if (/(今日|最新).*(ニュース|にゅーす).*(まとめ|要約|教えて|しゃべ|喋|読)/.test(msg)
       || /(ニュース|にゅーす).*(まとめ|要約).*(しゃべ|喋|読|話|聞)/.test(msg)) {
        return { action: 'summarize', minutes: 3, response: '3分のニュース要約を生成中...' };
      }
    }

    // --- Dark/Light mode ---
    if (/ダーク|dark\s*mode|ダークモード/.test(msg)) {
      Theme.setMode('dark');
      return { response: 'ダークモードに切り替えました。' };
    }
    if (/ライト|light\s*mode|ライトモード/.test(msg)) {
      Theme.setMode('light');
      return { response: 'ライトモードに切り替えました。' };
    }
    if (/モード(を)?(切替|切り替え|変更|トグル)|toggle\s*mode/.test(msg)) {
      Theme.toggleMode();
      const state = Theme.getState();
      return { response: `${state.mode === 'dark' ? 'ダーク' : 'ライト'}モードに切り替えました。` };
    }

    // --- Theme ---
    if (/ハッカー|hacker/.test(msg)) {
      Theme.setTheme('hacker');
      return { response: 'Hacker Newsテーマに変更しました。' };
    }
    if (/カード|card/.test(msg)) {
      Theme.setTheme('card');
      return { response: 'カードテーマに変更しました。' };
    }
    if (/ライト表示|lite|シンプル/.test(msg)) {
      Theme.setTheme('lite');
      return { response: 'Liteテーマに変更しました。' };
    }
    if (/ターミナル|terminal/.test(msg)) {
      Theme.setTheme('terminal');
      return { response: 'Terminalテーマに変更しました。' };
    }
    if (/マガジン|magazine|雑誌/.test(msg)) {
      Theme.setTheme('magazine');
      return { response: 'Magazineテーマに変更しました。' };
    }
    if (/ブルータリスト|brutalist/.test(msg)) {
      Theme.setTheme('brutalist');
      return { response: 'Brutalistテーマに変更しました。' };
    }
    if (/パステル|pastel/.test(msg)) {
      Theme.setTheme('pastel');
      return { response: 'Pastelテーマに変更しました。' };
    }
    if (/ネオン|neon|サイバー/.test(msg)) {
      Theme.setTheme('neon');
      return { response: 'Neonテーマに変更しました。' };
    }
    if (/ランダム|random|おまかせ|シャッフル/.test(msg) && /(テーマ|theme|表示|にして|して|変更)?/.test(msg)) {
      return { action: 'random_theme' };
    }

    // --- Font size ---
    if (/文字.*(大き|おおき|拡大)|font\s*size\s*up|bigger|larger/.test(msg)) {
      Theme.adjustFontSize(2);
      return { response: `文字サイズを${Storage.get('fontSize')}pxに変更しました。` };
    }
    if (/文字.*(小さ|ちいさ|縮小)|font\s*size\s*down|smaller/.test(msg)) {
      Theme.adjustFontSize(-2);
      return { response: `文字サイズを${Storage.get('fontSize')}pxに変更しました。` };
    }
    {
      const sizeMatch = msg.match(/文字.*?(\d+)\s*px|font\s*size\s*(\d+)/);
      if (sizeMatch) {
        const size = parseInt(sizeMatch[1] || sizeMatch[2], 10);
        if (Theme.setFontSize(size)) {
          return { response: `文字サイズを${size}pxに変更しました。` };
        }
      }
    }

    // --- TTS voice style ---
    if (/読み上げ.*(off|オフ|停止|無効)|tts\s*off/.test(msg)) {
      Tts.setStyle('off');
      Tts.stop();
      return { response: '読み上げをOFFにしました。' };
    }
    if (/ボイス.*(選|変更|一覧)|voice|声.*(選|変)/.test(msg)) {
      return { action: 'voice_picker', response: 'ボイス一覧を表示します。' };
    }
    // Match ElevenLabs voices by name
    {
      const voices = Tts.getVoices();
      for (const voice of voices) {
        if (msg.includes(voice.label.toLowerCase()) || msg.includes(voice.label)) {
          Tts.setStyle(voice.id);
          return { response: `ボイスを「${voice.label}」に設定しました。` };
        }
      }
    }
    // Legacy web speech shortcuts
    if (/ニュースキャスター|newscaster/.test(msg)) {
      Tts.setStyle('web:newscaster');
      return { response: 'ボイスを「ニュースキャスター」に設定しました。' };
    }
    if (/イケボ|ikebo/.test(msg)) {
      Tts.setStyle('web:ikebo');
      return { response: 'ボイスを「イケボ」に設定しました。' };
    }

    // --- Category filter ---
    {
      const categories = App.getCategories();
      for (const cat of categories) {
        const pattern = new RegExp(
          `${cat.label_ja}|${cat.label.toLowerCase()}|${cat.id}`,
          'i',
        );
        if (pattern.test(msg) && /(だけ|のみ|表示|フィルタ|filter|show|only)/.test(msg)) {
          App.setCategory(cat.id);
          return { response: `${cat.label_ja}カテゴリのみ表示します。` };
        }
      }
    }

    // --- Category management ---
    {
      // List: 「カテゴリ一覧」「項目管理」
      if (/カテゴリ.*(一覧|リスト|管理)|項目.*(一覧|管理|変更|編集)/.test(msg)) {
        return { action: 'category_list', response: 'カテゴリ一覧を表示します。' };
      }
      // Add: 「AIカテゴリを追加」
      const addMatch = msg.match(/(.+?)(カテゴリ)?(を|の)?\s*(追加|加えて|入れて)/);
      if (addMatch) {
        const label = addMatch[1].replace(/(新しい|新規)/, '').trim();
        if (label && label.length > 0 && label.length < 20 && !/フィード/.test(label)) {
          const id = label.replace(/[^a-zA-Z0-9\u3040-\u309F\u30A0-\u30FF\u4E00-\u9FFF]/g, '').toLowerCase() || label;
          return { action: 'category_add', id, label_ja: label, response: `カテゴリ「${label}」を追加中...` };
        }
      }
      // Remove: 「スポーツカテゴリを削除」
      const removeMatch = msg.match(/(.+?)(カテゴリ)?(を|の)?\s*(削除|消して|除去|なくして)/);
      if (removeMatch) {
        const label = removeMatch[1].trim();
        const cats = App.getCategories();
        const cat = cats.find(c => c.label_ja === label || c.id === label);
        if (cat) {
          return { action: 'category_remove', id: cat.id, label_ja: cat.label_ja, response: `カテゴリ「${cat.label_ja}」を削除中...` };
        }
      }
      // Rename: 「テクノロジーをITに変更」
      const renameMatch = msg.match(/(.+?)(を|の名前を)\s*(.+?)\s*(に変更|にリネーム|に変えて|にして)/);
      if (renameMatch && /(変更|リネーム|変えて)/.test(msg)) {
        const oldLabel = renameMatch[1].trim();
        const newLabel = renameMatch[3].trim();
        const cats = App.getCategories();
        const cat = cats.find(c => c.label_ja === oldLabel || c.id === oldLabel);
        if (cat && newLabel && newLabel.length < 20) {
          return { action: 'category_rename', id: cat.id, label_ja: newLabel, response: `カテゴリを「${newLabel}」に変更中...` };
        }
      }
    }

    // --- Accent color ---
    {
      const colorMap = {
        '青': 'blue', 'ブルー': 'blue', 'blue': 'blue',
        '緑': 'green', 'グリーン': 'green', 'green': 'green',
        '紫': 'purple', 'パープル': 'purple', 'purple': 'purple',
        '赤': 'red', 'レッド': 'red', 'red': 'red',
        'オレンジ': 'orange', '橙': 'orange', 'orange': 'orange',
        'ピンク': 'pink', 'pink': 'pink',
        'デフォルト': 'default', 'default': 'default',
      };
      if (/(アクセント|色|カラー).*(変|選|設定)|accent|color/.test(msg)) {
        for (const [keyword, colorKey] of Object.entries(colorMap)) {
          if (msg.includes(keyword.toLowerCase())) {
            Theme.setAccentColor(colorKey);
            return { response: `アクセントカラーを「${keyword}」に変更しました。` };
          }
        }
        // No specific color matched — show picker
        return { action: 'color_picker', response: 'アクセントカラーを選んでください。' };
      }
    }

    // --- Layout density ---
    if (/コンパクト|compact|詰めて/.test(msg) && /(表示|レイアウト|密度|にして|して|mode)/.test(msg)) {
      Theme.setDensity('compact');
      return { response: 'コンパクト表示に変更しました。' };
    }
    if (/ゆったり|spacious|広め/.test(msg) && /(表示|レイアウト|密度|にして|して|mode)/.test(msg)) {
      Theme.setDensity('spacious');
      return { response: 'ゆったり表示に変更しました。' };
    }
    if (/通常表示|normal|ノーマル表示/.test(msg)) {
      Theme.setDensity('normal');
      return { response: '通常表示に変更しました。' };
    }

    // --- Bookmarks ---
    if (/ブックマーク.*(一覧|リスト|表示)|bookmark.*list/.test(msg)) {
      return { action: 'bookmark_list', response: 'ブックマーク一覧を表示します。' };
    }

    // --- Auto-refresh ---
    {
      const refreshMatch = msg.match(/(\d+)\s*分.*(自動更新|オートリフレッシュ|auto.*refresh)/);
      if (refreshMatch) {
        const mins = parseInt(refreshMatch[1], 10);
        if ([1, 5, 15].includes(mins)) {
          App.setAutoRefresh(mins);
          return { response: `${mins}分ごとに自動更新します。` };
        }
      }
      if (/自動更新.*(off|オフ|停止|無効|解除)|auto.*refresh.*(off|disable|stop)/.test(msg)) {
        App.setAutoRefresh(0);
        return { response: '自動更新をOFFにしました。' };
      }
      if (/自動更新/.test(msg) && !/(off|オフ|停止|無効|解除)/.test(msg)) {
        const m = msg.match(/(\d+)/);
        if (m) {
          const mins = parseInt(m[1], 10);
          if (mins > 0 && mins <= 30) {
            App.setAutoRefresh(mins);
            return { response: `${mins}分ごとに自動更新します。` };
          }
        }
      }
    }

    // --- Settings reset ---
    if (/設定.*(リセット|初期化|デフォルト)|reset.*settings/.test(msg)) {
      return { action: 'settings_reset', response: '設定をリセットしますか？' };
    }

    // --- Clear filter ---
    if (/フィルタ.*(解除|クリア|リセット)|すべて表示|全.*(表示|記事)|clear\s*filter|show\s*all|reset/.test(msg)) {
      App.setCategory('');
      return { response: 'フィルタを解除しました。すべてのカテゴリを表示します。' };
    }

    // --- Refresh ---
    if (/更新|リフレッシュ|refresh|reload/.test(msg)) {
      App.refresh();
      return { response: '記事を再読み込みしました。' };
    }

    // --- Google login/logout ---
    if (/ログイン|login|サインイン|sign\s*in|google.*(ログイン|認証)|認証/.test(msg) && !/ログアウト|logout|sign\s*out/.test(msg)) {
      return { action: 'google_login', response: 'Googleログイン画面を表示します...' };
    }
    if (/ログアウト|logout|サインアウト|sign\s*out/.test(msg)) {
      return { action: 'google_logout', response: 'ログアウトします...' };
    }

    // --- Subscription ---
    if (/プロ|pro|サブスク|subscribe|課金|アップグレード|upgrade/.test(msg) && !/解約|キャンセル|cancel|管理|portal/.test(msg)) {
      return { action: 'subscribe', response: 'Proプランのチェックアウトを開きます...' };
    }
    if (/課金管理|billing|ポータル|portal|解約|キャンセル|cancel/.test(msg)) {
      return { action: 'billing_portal', response: '課金管理ポータルを開きます...' };
    }
    if (/利用状況|usage|残り回数/.test(msg)) {
      return { action: 'show_usage', response: '利用状況を確認中...' };
    }

    // --- Feed management ---
    if (/フィード.*(一覧|リスト)|feed.*list/.test(msg)) {
      return { action: 'feed_list', response: 'フィード一覧を取得中...' };
    }
    {
      const feedAddMatch = msg.match(/フィード.*追加[:：]\s*(.+?)[,、]\s*(.+)/);
      if (feedAddMatch) {
        const url = feedAddMatch[1].trim();
        const source = feedAddMatch[2].trim();
        return { action: 'feed_add', url, source, category: 'general', response: `フィード「${source}」を追加中...` };
      }
    }
    {
      const feedDelMatch = msg.match(/フィード.*削除[:：]\s*(.+)/);
      if (feedDelMatch) {
        const feedId = feedDelMatch[1].trim();
        return { action: 'feed_delete', feed_id: feedId, response: `フィード「${feedId}」を削除中...` };
      }
    }

    // --- Data management ---
    if (/履歴.*(クリア|削除|消去|リセット)|clear.*history/.test(msg)) {
      ReadHistory.clear();
      return { response: '閲覧履歴をクリアしました。' };
    }
    if (/ブックマーク.*(クリア|削除|全削除|消去|リセット)|clear.*bookmark/.test(msg)) {
      Bookmarks.clear();
      return { response: 'ブックマークをクリアしました。' };
    }
    if (/キャッシュ.*(クリア|削除|消去|リセット)|clear.*cache/.test(msg)) {
      try {
        const eco = JSON.parse(localStorage.getItem('hn_eco') || '{}');
        eco.queryCache = {};
        eco.cacheHits = 0;
        localStorage.setItem('hn_eco', JSON.stringify(eco));
      } catch {}
      return { response: 'AIキャッシュをクリアしました。' };
    }

    // --- Custom prompt ---
    {
      const promptMatch = msg.match(/プロンプト設定[:：]\s*(.+)/);
      if (promptMatch) {
        const prompt = promptMatch[1].trim();
        Storage.set('aiQuestionPrompt', prompt);
        Storage.set('aiAnswerPrompt', prompt);
        return { response: `カスタムプロンプトを設定しました:\n「${prompt}」` };
      }
    }

    // --- Open settings page ---
    if (/設定.*(開|ページ|画面)|settings.*open|設定を開く/.test(msg)) {
      return { action: 'open_settings', response: '設定画面を開きます...' };
    }

    // --- Help ---
    if (/ヘルプ|help|使い方|コマンド/.test(msg)) {
      return {
        response: `利用可能なコマンド:

【見た目】
• 「ダークモードにして」— ダーク/ライト切替
• 「カード表示にして」— テーマ切替 (8種類)
• 「ランダム」— テーマをランダムに変更
• 「文字を大きくして」— フォントサイズ変更
• 「アクセント 青」— アクセントカラー変更
• 「コンパクト表示にして」— レイアウト密度変更

【コンテンツ】
• 「テクノロジーだけ表示して」— カテゴリフィルタ
• 「フィルタ解除」— 全記事表示
• 「5分ごと自動更新」— 自動更新設定
• 「ブックマーク一覧」— 保存した記事を表示

【カテゴリ管理】
• 「カテゴリ一覧」— カテゴリの管理
• 「AIカテゴリを追加」— カテゴリを追加
• 「スポーツを削除」— カテゴリを削除

【音声】
• 「ボイスを選ぶ」— ボイス選択
• 「読み上げOFF」— 音声読み上げを停止
• 「3分でまとめてしゃべって」— ニュース要約・読み上げ

【フィード管理】
• 「フィード一覧」— 登録フィードの一覧
• 「フィード追加: URL, ソース名」— フィード追加
• 「フィード削除: feed-xxx」— フィード削除

【データ管理】
• 「履歴クリア」— 閲覧履歴を削除
• 「ブックマーククリア」— ブックマーク全削除
• 「キャッシュクリア」— AIキャッシュ削除
• 「プロンプト設定: ...」— AIカスタムプロンプト設定

【アカウント】
• 「ログイン」— Googleでログイン（制限2倍）
• 「ログアウト」— ログアウト

【その他】
• 「設定を開く」— 設定画面へ移動
• 「設定リセット」— 設定を初期化
• 「ステータス」— 現在の設定確認
• 「更新」— 記事を再読み込み
• 「ヘルプ」— このヘルプを表示`
      };
    }

    // --- Status ---
    if (/ステータス|status|設定|settings/.test(msg)) {
      const s = Theme.getState();
      const cat = Storage.get('category') || 'すべて';
      const ttsStyle = Tts.getStyle();
      const ttsLabel = ttsStyle === 'off' ? 'OFF' : (Tts.STYLES[ttsStyle]?.label || ttsStyle);
      const autoRefresh = Storage.get('autoRefresh');
      const bookmarkCount = Bookmarks.getCount();
      return {
        response: `現在の設定:
• テーマ: ${s.theme}
• モード: ${s.mode}
• 文字サイズ: ${s.fontSize}px
• アクセントカラー: ${s.accentColor}
• レイアウト: ${s.density}
• カテゴリ: ${cat}
• 読み上げ: ${ttsLabel}
• 自動更新: ${autoRefresh > 0 ? autoRefresh + '分' : 'OFF'}
• ブックマーク: ${bookmarkCount}件`
      };
    }

    return null;
  }

  return { process };
})();
