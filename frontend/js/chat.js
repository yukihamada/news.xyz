/**
 * chat.js — Chat panel UI with server-side command support
 */
'use strict';

const Chat = (() => {
  let panel, messages, form, input, toggle, closeBtn, suggestionsEl;
  let pendingChangeId = null;
  let currentStep = 0;

  const SUGGESTIONS = [
    // Step 0: テーマ選択 — 8テーマ + ランダム
    [
      { label: 'Hacker', command: 'ハッカー風にして' },
      { label: 'Card', command: 'カード表示にして' },
      { label: 'Lite', command: 'ライト表示にして' },
      { label: 'Terminal', command: 'ターミナルにして' },
      { label: 'Magazine', command: 'マガジンにして' },
      { label: 'Brutalist', command: 'ブルータリストにして' },
      { label: 'Pastel', command: 'パステルにして' },
      { label: 'Neon', command: 'ネオンにして' },
      { label: 'Random!', action: 'random_theme' },
    ],
    // Step 1: カスタマイズ — モード, カラー, フォントサイズ, 次へ
    [
      { label: 'ダーク/ライト切替', command: 'モード切替にして' },
      { label: 'アクセントカラー', action: 'color_picker' },
      { label: '文字サイズ調整', action: 'font_size_picker' },
      { label: '次へ →', action: 'skip_step' },
    ],
    // Step 2: AI機能
    [
      { label: '3分ニュース', action: 'summarize', minutes: 3 },
      { label: 'ボイスを選ぶ', action: 'voice_picker' },
      { label: '利用状況', action: 'show_usage' },
      { label: 'Googleログイン', action: 'google_login' },
    ],
    // Step 3: 管理
    [
      { label: '5分自動更新', action: 'auto_refresh', minutes: 5 },
      { label: 'カテゴリ管理', action: 'category_list' },
      { label: 'ブックマーク一覧', action: 'bookmark_list' },
      { label: '完了', command: null },
    ],
  ];

  function init() {
    panel = document.getElementById('chat-panel');
    messages = document.getElementById('chat-messages');
    form = document.getElementById('chat-form');
    input = document.getElementById('chat-input');
    toggle = document.getElementById('chat-toggle');
    closeBtn = document.getElementById('chat-close');
    suggestionsEl = document.getElementById('chat-suggestions');

    toggle.addEventListener('click', () => togglePanel());
    closeBtn.addEventListener('click', () => closePanel());
    form.addEventListener('submit', onSubmit);

    // Close on Escape
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && !panel.hidden) {
        closePanel();
      }
    });

    showSuggestions(0);
  }

  function togglePanel() {
    if (panel.hidden) {
      openPanel();
    } else {
      closePanel();
    }
  }

  function openPanel() {
    panel.hidden = false;
    panel.classList.add('open');
    input.focus();
    showSuggestions(currentStep);
  }

  function closePanel() {
    panel.hidden = true;
    panel.classList.remove('open');
  }

  async function onSubmit(e) {
    e.preventDefault();
    const text = input.value.trim();
    if (!text) return;

    addMessage(text, 'user');
    input.value = '';

    // Handle pending change confirmation
    if (pendingChangeId) {
      await handleChangeConfirmation(text);
      return;
    }

    // Try local commands first
    const result = Commands.process(text);
    if (result) {
      if (result.action === 'summarize') {
        await handleSummarize(result.minutes);
        return;
      }
      if (result.action === 'voice_picker') {
        addMessage(result.response, 'bot');
        showVoicePicker();
        return;
      }
      if (result.action === 'random_theme') {
        const r = Theme.randomize();
        addMessage(`ランダム設定: テーマ=${r.theme}, モード=${r.mode}, アクセント=${r.accent}`, 'bot');
        return;
      }
      if (result.action === 'color_picker') {
        addMessage(result.response, 'bot');
        showColorPicker();
        return;
      }
      if (result.action === 'bookmark_list') {
        addMessage(result.response, 'bot');
        showBookmarkList();
        return;
      }
      if (result.action === 'settings_reset') {
        handleSettingsReset();
        return;
      }
      if (result.action === 'google_login') {
        if (typeof GoogleAuth !== 'undefined' && GoogleAuth.isAuthenticated()) {
          addMessage('既にGoogleでログイン済みです。', 'bot');
        } else if (typeof GoogleAuth !== 'undefined') {
          addMessage('Googleログイン画面を表示します...', 'bot');
          const container = document.createElement('div');
          container.style.padding = '8px 0';
          suggestionsEl.innerHTML = '';
          suggestionsEl.appendChild(container);
          GoogleAuth.renderButton(container);
          GoogleAuth.showOneTap();
        }
        return;
      }
      if (result.action === 'google_logout') {
        if (typeof GoogleAuth !== 'undefined') {
          GoogleAuth.signOut();
          addMessage('Googleアカウントからログアウトしました。', 'bot');
        }
        return;
      }
      if (result.action === 'subscribe') {
        addMessage(result.response, 'bot');
        handleSubscribe();
        return;
      }
      if (result.action === 'billing_portal') {
        addMessage(result.response, 'bot');
        handleBillingPortal();
        return;
      }
      if (result.action === 'show_usage') {
        await handleShowUsage();
        return;
      }
      if (result.action?.startsWith('category_')) {
        await handleCategoryAction(result);
        return;
      }
      if (result.action === 'feed_list') {
        await handleFeedList();
        return;
      }
      if (result.action === 'feed_add') {
        await handleFeedAdd(result.url, result.source, result.category);
        return;
      }
      if (result.action === 'feed_delete') {
        await handleFeedDelete(result.feed_id);
        return;
      }
      if (result.action === 'open_settings') {
        addMessage(result.response, 'bot');
        window.location.href = '/settings.html';
        return;
      }
      addMessage(result.response, 'bot');
      return;
    }

    // Send to server for AI interpretation
    addMessage('考え中...', 'bot thinking');
    try {
      const response = await Api.sendCommand(text);
      removeThinking();

      if (response.type === 'error') {
        addMessage(response.message, 'bot');
      } else if (response.type === 'info') {
        addMessage(response.message, 'bot');
      } else if (response.type === 'preview') {
        pendingChangeId = response.change_id;
        const actionsDesc = response.actions
          .map((a) => formatAction(a))
          .join('\n');
        addMessage(
          `${response.interpretation}\n\n変更内容:\n${actionsDesc}\n\nこの変更を適用しますか？（「はい」または「キャンセル」）`,
          'bot'
        );
      }
    } catch (err) {
      removeThinking();
      addMessage(`エラーが発生しました: ${err.message}`, 'bot');
    }
  }

  async function handleSummarize(minutes) {
    addMessage(`${minutes}分のニュース要約を生成中...`, 'bot thinking');
    try {
      const data = await Api.summarizeArticles(minutes);
      removeThinking();
      if (data.error) {
        addMessage(`エラー: ${data.error}`, 'bot');
        return;
      }
      addMessage(data.summary, 'bot');
      const spoke = Tts.speakText(data.summary_reading || data.summary);
      if (!spoke) {
        addMessage('読み上げるにはボイススタイルを設定してください（「ニュースキャスター」など）', 'bot');
      }
    } catch (err) {
      removeThinking();
      addMessage(`要約の取得に失敗しました: ${err.message}`, 'bot');
    }
  }

  async function handleChangeConfirmation(text) {
    const msg = text.toLowerCase();
    const changeId = pendingChangeId;
    pendingChangeId = null;

    if (/はい|yes|ok|適用|apply|承認/.test(msg)) {
      addMessage('変更を適用中...', 'bot thinking');
      try {
        const result = await Api.applyChange(changeId);
        removeThinking();
        if (result.errors && result.errors.length > 0) {
          addMessage(
            `変更を適用しました（${result.applied}件成功、${result.errors.length}件エラー）`,
            'bot'
          );
        } else {
          addMessage('変更を適用しました。次回のフィード取得から反映されます。', 'bot');
        }
      } catch (err) {
        removeThinking();
        addMessage(`変更の適用に失敗しました: ${err.message}`, 'bot');
      }
    } else {
      try {
        await Api.rejectChange(changeId);
      } catch {
        // ignore reject errors
      }
      addMessage('変更をキャンセルしました。', 'bot');
    }
  }

  function formatAction(action) {
    switch (action.type) {
      case 'add_feed':
        return `+ フィード追加: ${action.source} (${action.category})`;
      case 'remove_feed':
        return `- フィード削除: ${action.feed_id}`;
      case 'enable_feed':
        return `○ フィード有効化: ${action.feed_id}`;
      case 'disable_feed':
        return `× フィード無効化: ${action.feed_id}`;
      case 'toggle_feature':
        return `${action.enabled ? '○' : '×'} 機能${action.enabled ? '有効' : '無効'}化: ${action.feature}`;
      case 'set_grouping_threshold':
        return `~ グルーピングしきい値: ${action.threshold}`;
      case 'add_category':
        return `+ カテゴリ追加: ${action.label_ja} (${action.id})`;
      case 'remove_category':
        return `- カテゴリ削除: ${action.id}`;
      case 'rename_category':
        return `~ カテゴリ名変更: ${action.id} → ${action.label_ja}`;
      case 'reorder_categories':
        return `~ カテゴリ並び替え: ${action.order?.join(', ')}`;
      default:
        return `? ${JSON.stringify(action)}`;
    }
  }

  function showSuggestions(step) {
    currentStep = step;
    suggestionsEl.innerHTML = '';
    const items = SUGGESTIONS[step];
    if (!items) return;
    items.forEach((item) => {
      const btn = document.createElement('button');
      btn.className = 'chat-chip';
      btn.type = 'button';
      btn.textContent = item.label;
      btn.addEventListener('click', () => onChipClick(item));
      suggestionsEl.appendChild(btn);
    });
  }

  async function onChipClick(item) {
    // "これでOK" — complete and reset
    if (item.command === null) {
      addMessage(item.label, 'user');
      addMessage('カスタマイズ完了！またいつでもどうぞ。', 'bot');
      showSuggestions(0);
      return;
    }

    addMessage(item.label, 'user');

    // Random theme chip — stay on step 0 for re-roll
    if (item.action === 'random_theme') {
      const result = Theme.randomize();
      addMessage(`ランダム設定: テーマ=${result.theme}, モード=${result.mode}, アクセント=${result.accent}`, 'bot');
      showSuggestions(0);
      return;
    }

    // Font size picker chip
    if (item.action === 'font_size_picker') {
      addMessage('文字サイズを調整してください。', 'bot');
      showFontSizePicker();
      return;
    }

    // Skip to next step
    if (item.action === 'skip_step') {
      const nextStep = currentStep + 1;
      if (nextStep < SUGGESTIONS.length) {
        showSuggestions(nextStep);
      }
      return;
    }

    // Voice picker chip
    if (item.action === 'voice_picker') {
      showVoicePicker();
      return;
    }

    // Color picker chip
    if (item.action === 'color_picker') {
      addMessage('アクセントカラーを選んでください。', 'bot');
      showColorPicker();
      return;
    }

    // Category list chip
    if (item.action === 'category_list') {
      addMessage('カテゴリ一覧を表示します。', 'bot');
      showCategoryManager();
      return;
    }

    // Bookmark list chip
    if (item.action === 'bookmark_list') {
      addMessage('ブックマーク一覧を表示します。', 'bot');
      showBookmarkList();
      const nextStep = currentStep + 1;
      if (nextStep < SUGGESTIONS.length) showSuggestions(nextStep);
      return;
    }

    // Settings reset chip
    if (item.action === 'settings_reset') {
      handleSettingsReset();
      const nextStep = currentStep + 1;
      if (nextStep < SUGGESTIONS.length) showSuggestions(nextStep);
      return;
    }

    // Auto-refresh chip
    if (item.action === 'auto_refresh') {
      App.setAutoRefresh(item.minutes);
      addMessage(`${item.minutes}分ごとに自動更新します。`, 'bot');
      const nextStep = currentStep + 1;
      if (nextStep < SUGGESTIONS.length) showSuggestions(nextStep);
      return;
    }

    // Google login chip
    if (item.action === 'google_login') {
      if (typeof GoogleAuth !== 'undefined' && GoogleAuth.isAuthenticated()) {
        addMessage('既にGoogleでログイン済みです。', 'bot');
      } else if (typeof GoogleAuth !== 'undefined') {
        addMessage('Googleログイン画面を表示します...', 'bot');
        const container = document.createElement('div');
        container.style.padding = '8px 0';
        suggestionsEl.innerHTML = '';
        suggestionsEl.appendChild(container);
        GoogleAuth.renderButton(container);
        GoogleAuth.showOneTap();
      } else {
        addMessage('Google認証機能は現在利用できません。', 'bot');
      }
      return;
    }

    // Google logout chip
    if (item.action === 'google_logout') {
      if (typeof GoogleAuth !== 'undefined') {
        GoogleAuth.signOut();
        addMessage('Googleアカウントからログアウトしました。', 'bot');
      }
      const nextStep = currentStep + 1;
      if (nextStep < SUGGESTIONS.length) showSuggestions(nextStep);
      return;
    }

    // Subscribe chip
    if (item.action === 'subscribe') {
      addMessage('Proプランのチェックアウトを開きます...', 'bot');
      handleSubscribe();
      return;
    }

    // Billing portal chip
    if (item.action === 'billing_portal') {
      addMessage('課金管理ポータルを開きます...', 'bot');
      handleBillingPortal();
      return;
    }

    // Show usage chip
    if (item.action === 'show_usage') {
      await handleShowUsage();
      const nextStep = currentStep + 1;
      if (nextStep < SUGGESTIONS.length) showSuggestions(nextStep);
      return;
    }

    // Summarize chip
    if (item.action === 'summarize') {
      await handleSummarize(item.minutes);
      const nextStep = currentStep + 1;
      if (nextStep < SUGGESTIONS.length) showSuggestions(nextStep);
      return;
    }

    // Voice style chip
    if (item.voice !== undefined) {
      if (item.voice === 'off') {
        Tts.setStyle('off');
        Tts.stop();
        addMessage('読み上げをOFFにしました。', 'bot');
      } else {
        Tts.setStyle(item.voice);
        const styleName = Tts.STYLES[item.voice]?.label || item.voice;
        addMessage(`ボイスを「${styleName}」に設定しました。記事の🔊ボタンで読み上げできます。`, 'bot');
      }
      const nextStep = currentStep + 1;
      if (nextStep < SUGGESTIONS.length) {
        showSuggestions(nextStep);
      }
      return;
    }

    if (item.server) {
      // Server-side command (Claude API)
      addMessage('考え中...', 'bot thinking');
      try {
        const response = await Api.sendCommand(item.command);
        removeThinking();
        if (response.type === 'preview') {
          pendingChangeId = response.change_id;
          const actionsDesc = response.actions.map((a) => formatAction(a)).join('\n');
          addMessage(
            `${response.interpretation}\n\n変更内容:\n${actionsDesc}\n\nこの変更を適用しますか？（「はい」または「キャンセル」）`,
            'bot'
          );
        } else {
          addMessage(response.message || '実行しました。', 'bot');
        }
      } catch (err) {
        removeThinking();
        addMessage(`エラー: ${err.message}`, 'bot');
      }
    } else {
      // Local command
      const result = Commands.process(item.command);
      if (result) {
        addMessage(result.response, 'bot');
      }
    }

    // Advance to next step
    const nextStep = currentStep + 1;
    if (nextStep < SUGGESTIONS.length) {
      showSuggestions(nextStep);
    }
  }

  async function handleCategoryAction(result) {
    if (result.action === 'category_list') {
      addMessage(result.response, 'bot');
      showCategoryManager();
      return;
    }
    addMessage(result.response, 'bot thinking');
    try {
      let data;
      if (result.action === 'category_add') {
        data = await Api.manageCategory('add', result.id, result.label_ja);
      } else if (result.action === 'category_remove') {
        data = await Api.manageCategory('remove', result.id);
      } else if (result.action === 'category_rename') {
        data = await Api.manageCategory('rename', result.id, result.label_ja);
      }
      removeThinking();
      addMessage(data?.message || '完了しました。', 'bot');
      App.refresh();
    } catch (err) {
      removeThinking();
      addMessage(`エラー: ${err.message}`, 'bot');
    }
  }

  async function handleFeedList() {
    addMessage('フィード一覧を取得中...', 'bot thinking');
    try {
      const data = await Api.listFeeds();
      removeThinking();
      if (!data.feeds || data.feeds.length === 0) {
        addMessage('登録されているフィードはありません。', 'bot');
        return;
      }
      let msg = `登録フィード（${data.feeds.length}件）:\n`;
      for (const f of data.feeds) {
        const status = f.enabled ? '○' : '×';
        msg += `${status} ${f.source} [${f.category}] — ${f.feed_id}\n`;
      }
      addMessage(msg.trim(), 'bot');
    } catch (err) {
      removeThinking();
      addMessage(`フィード一覧の取得に失敗: ${err.message}`, 'bot');
    }
  }

  async function handleFeedAdd(url, source, category) {
    addMessage(`フィード「${source}」を追加中...`, 'bot thinking');
    try {
      const data = await Api.addFeed(url, source, category);
      removeThinking();
      addMessage(data.message || 'フィードを追加しました。', 'bot');
    } catch (err) {
      removeThinking();
      addMessage(`フィード追加に失敗: ${err.message}`, 'bot');
    }
  }

  async function handleFeedDelete(feedId) {
    addMessage(`フィード「${feedId}」を削除中...`, 'bot thinking');
    try {
      const data = await Api.deleteFeed(feedId);
      removeThinking();
      addMessage(data.message || 'フィードを削除しました。', 'bot');
    } catch (err) {
      removeThinking();
      addMessage(`フィード削除に失敗: ${err.message}`, 'bot');
    }
  }

  async function handleShowUsage() {
    addMessage('利用状況を確認中...', 'bot thinking');
    const data = await Subscription.fetchUsage();
    removeThinking();
    if (!data) {
      addMessage('利用状況の取得に失敗しました。', 'bot');
      return;
    }
    if (data.tier === 'pro') {
      addMessage('Proプラン: すべてのAI機能が無制限でご利用いただけます。', 'bot');
      return;
    }
    if (data.tier === 'authenticated') {
      addMessage('Googleログイン済み: 制限が2倍に拡大されています。', 'bot');
    }
    const features = { summarize: 'AI要約', questions: 'AI質問', ask: 'AI回答', tts: 'TTS', to_reading: '読み変換' };
    let msg = '本日の利用状況:\n';
    for (const [key, label] of Object.entries(features)) {
      const used = data.usage?.[key] || 0;
      const limit = data.limits?.[key] || '?';
      msg += `\u2022 ${label}: ${used}/${limit}\u56de\n`;
    }
    addMessage(msg.trim(), 'bot');
  }

  function showCategoryManager() {
    const cats = App.getCategories();
    suggestionsEl.innerHTML = '';
    // Show current categories as chips with delete option
    for (const cat of cats) {
      const btn = document.createElement('button');
      btn.className = 'chat-chip';
      btn.type = 'button';
      btn.textContent = `× ${cat.label_ja}`;
      btn.addEventListener('click', async () => {
        addMessage(`${cat.label_ja}を削除`, 'user');
        try {
          const data = await Api.manageCategory('remove', cat.id);
          addMessage(data.message, 'bot');
          App.refresh();
          showCategoryManager();
        } catch (err) {
          addMessage(`エラー: ${err.message}`, 'bot');
        }
      });
      suggestionsEl.appendChild(btn);
    }
    // Add new category chip
    const addBtn = document.createElement('button');
    addBtn.className = 'chat-chip chip-accent';
    addBtn.type = 'button';
    addBtn.textContent = '+ 追加';
    addBtn.addEventListener('click', () => {
      addMessage('新しいカテゴリ名を入力してください:', 'bot');
      input.placeholder = '例: ライフスタイル';
      input.focus();
    });
    suggestionsEl.appendChild(addBtn);
    // Close chip
    const doneBtn = document.createElement('button');
    doneBtn.className = 'chat-chip';
    doneBtn.type = 'button';
    doneBtn.textContent = '完了';
    doneBtn.addEventListener('click', () => {
      addMessage('完了', 'user');
      addMessage('カテゴリの管理を終了しました。', 'bot');
      input.placeholder = '例: ダークモードにして';
      showSuggestions(0);
    });
    suggestionsEl.appendChild(doneBtn);
  }

  function showVoicePicker() {
    const voices = Tts.getVoices();
    if (voices.length === 0) {
      addMessage('利用可能なボイスがありません。', 'bot');
      return;
    }
    addMessage('ボイスを選んでください:', 'bot');
    suggestionsEl.innerHTML = '';
    for (const voice of voices) {
      const btn = document.createElement('button');
      btn.className = 'chat-chip';
      btn.type = 'button';
      btn.textContent = voice.label;
      if (voice.category === 'cloned' || voice.recommended) btn.classList.add('chip-accent');
      btn.addEventListener('click', () => {
        addMessage(voice.label, 'user');
        Tts.setStyle(voice.id);
        addMessage(`ボイスを「${voice.label}」に設定しました。`, 'bot');
        const nextStep = currentStep + 1;
        if (nextStep < SUGGESTIONS.length) showSuggestions(nextStep);
      });
      suggestionsEl.appendChild(btn);
    }
    // OFF option
    const offBtn = document.createElement('button');
    offBtn.className = 'chat-chip';
    offBtn.type = 'button';
    offBtn.textContent = '読み上げOFF';
    offBtn.addEventListener('click', () => {
      addMessage('読み上げOFF', 'user');
      Tts.setStyle('off');
      Tts.stop();
      addMessage('読み上げをOFFにしました。', 'bot');
      const nextStep = currentStep + 1;
      if (nextStep < SUGGESTIONS.length) showSuggestions(nextStep);
    });
    suggestionsEl.appendChild(offBtn);
  }

  function showColorPicker() {
    const presets = Theme.ACCENT_PRESETS;
    const labels = {
      default: 'デフォルト', blue: 'ブルー', green: 'グリーン', purple: 'パープル',
      red: 'レッド', orange: 'オレンジ', pink: 'ピンク',
    };
    suggestionsEl.innerHTML = '';
    for (const [key, preset] of Object.entries(presets)) {
      const btn = document.createElement('button');
      btn.className = 'chat-chip';
      btn.type = 'button';
      const dot = key !== 'default' ? `<span style="display:inline-block;width:12px;height:12px;border-radius:50%;background:${preset.light};vertical-align:middle;margin-right:4px"></span>` : '';
      btn.innerHTML = `${dot}${labels[key] || key}`;
      btn.addEventListener('click', () => {
        addMessage(labels[key] || key, 'user');
        Theme.setAccentColor(key);
        addMessage(`アクセントカラーを「${labels[key] || key}」に変更しました。`, 'bot');
        const nextStep = currentStep + 1;
        if (nextStep < SUGGESTIONS.length) showSuggestions(nextStep);
      });
      suggestionsEl.appendChild(btn);
    }
  }

  function showFontSizePicker() {
    suggestionsEl.innerHTML = '';
    const current = Storage.get('fontSize');
    const items = [
      { label: '最小 (10px)', size: 10 },
      { label: '小さく', delta: -2 },
      { label: `現在: ${current}px`, disabled: true },
      { label: '大きく', delta: 2 },
      { label: '最大 (32px)', size: 32 },
      { label: '完了', done: true },
    ];
    for (const item of items) {
      const btn = document.createElement('button');
      btn.className = 'chat-chip';
      btn.type = 'button';
      btn.textContent = item.label;
      if (item.disabled) {
        btn.disabled = true;
      } else if (item.done) {
        btn.classList.add('chip-accent');
        btn.addEventListener('click', () => {
          addMessage('完了', 'user');
          addMessage(`文字サイズを${Storage.get('fontSize')}pxに設定しました。`, 'bot');
          const nextStep = currentStep + 1;
          if (nextStep < SUGGESTIONS.length) showSuggestions(nextStep);
        });
      } else {
        btn.addEventListener('click', () => {
          if (item.size !== undefined) {
            Theme.setFontSize(item.size);
          } else {
            Theme.adjustFontSize(item.delta);
          }
          addMessage(`${item.label} → ${Storage.get('fontSize')}px`, 'user');
          showFontSizePicker();
        });
      }
      suggestionsEl.appendChild(btn);
    }
  }

  function showBookmarkList() {
    const items = Bookmarks.getAll();
    if (items.length === 0) {
      addMessage('ブックマークはまだありません。記事カードの🔖ボタンで保存できます。', 'bot');
      return;
    }
    let msg = `ブックマーク (${items.length}件):\n`;
    for (const item of items.slice(0, 20)) {
      msg += `\u2022 ${item.title} (${item.source || ''})\n`;
    }
    if (items.length > 20) msg += `...他${items.length - 20}件`;
    addMessage(msg.trim(), 'bot');
  }

  function handleSettingsReset() {
    addMessage('設定リセット', 'user');
    const defaults = Storage.DEFAULTS;
    for (const key of Object.keys(defaults)) {
      Storage.set(key, defaults[key]);
    }
    Theme.apply();
    App.setAutoRefresh(0);
    addMessage('設定を初期値にリセットしました。', 'bot');
  }

  function handleSubscribe() {
    try {
      Subscription.subscribe();
    } catch {
      addMessage('サブスクリプション機能は現在準備中です。もうしばらくお待ちください。', 'bot');
    }
  }

  function handleBillingPortal() {
    try {
      Subscription.openBillingPortal();
    } catch {
      addMessage('課金管理機能は現在準備中です。もうしばらくお待ちください。', 'bot');
    }
  }

  function removeThinking() {
    const thinking = messages.querySelector('.chat-msg.thinking');
    if (thinking) thinking.remove();
  }

  function addMessage(text, sender) {
    const div = document.createElement('div');
    div.className = `chat-msg ${sender}`;
    div.textContent = text;
    messages.appendChild(div);
    div.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }

  return { init, openPanel, closePanel, addMessage };
})();
