/**
 * settings.js — Settings page logic (v20 — modern redesign)
 */
'use strict';

(function() {
  const BASE = '';

  function adminHeaders() {
    const secret = Storage.get('adminSecret') || '';
    return secret ? { 'X-Admin-Secret': secret } : {};
  }

  // --- Dark mode from storage ---
  document.body.dataset.mode = Storage.get('mode') || 'light';

  // --- Tab switching (auto-select Display or from hash) ---
  const tabs = document.querySelectorAll('.tab-btn');
  const panels = document.querySelectorAll('.tab-panel');

  function activateTab(name) {
    tabs.forEach(t => t.classList.toggle('active', t.dataset.tab === name));
    panels.forEach(p => p.classList.toggle('active', p.id === 'tab-' + name));
    history.replaceState(null, '', '#' + name);
  }

  tabs.forEach(btn => {
    btn.addEventListener('click', () => activateTab(btn.dataset.tab));
  });

  // Auto-select tab from URL hash or default to 'display'
  const hashTab = location.hash.replace('#', '');
  activateTab(hashTab && document.getElementById('tab-' + hashTab) ? hashTab : 'display');

  // --- Toast ---
  function toast(msg) {
    document.querySelectorAll('.toast').forEach(t => t.remove());
    const el = document.createElement('div');
    el.className = 'toast';
    el.textContent = msg;
    document.body.appendChild(el);
    setTimeout(() => el.remove(), 2200);
  }

  // --- Admin secret ---
  const adminInput = document.getElementById('admin-secret');
  adminInput.value = Storage.get('adminSecret') || '';
  document.getElementById('admin-save-btn').addEventListener('click', () => {
    Storage.set('adminSecret', adminInput.value);
    toast('Admin secret saved');
    loadFeeds();
  });

  // --- Helpers ---
  function setActive(groupId, value) {
    const el = document.getElementById(groupId);
    if (!el) return;
    el.querySelectorAll('.opt-btn').forEach(btn => {
      btn.classList.toggle('active', btn.dataset.value === value);
    });
  }

  function initToggle(id, key) {
    const btn = document.getElementById(id);
    if (!btn) return;
    btn.setAttribute('aria-checked', String(!!Storage.get(key)));
    btn.addEventListener('click', () => {
      const next = btn.getAttribute('aria-checked') !== 'true';
      btn.setAttribute('aria-checked', String(next));
      Storage.set(key, next);
      toast(`${key}: ${next ? 'ON' : 'OFF'}`);
    });
  }

  function initSlider(rangeId, valId, key, suffix, transform) {
    const range = document.getElementById(rangeId);
    const valEl = document.getElementById(valId);
    if (!range || !valEl) return;
    const stored = Storage.get(key);
    range.value = stored;
    valEl.textContent = (transform ? transform(stored) : stored) + (suffix || '');
    range.addEventListener('input', e => {
      const raw = parseFloat(e.target.value);
      const display = transform ? transform(raw) : raw;
      valEl.textContent = display + (suffix || '');
      Storage.set(key, raw);
      updatePreview();
    });
  }

  function initBtnGroup(groupId, key, opts) {
    const val = String(Storage.get(key) ?? ((opts && opts.default) || ''));
    setActive(groupId, val);
    const el = document.getElementById(groupId);
    if (!el) return;
    el.addEventListener('click', e => {
      const btn = e.target.closest('.opt-btn');
      if (!btn) return;
      const v = btn.dataset.value;
      const parsed = opts && opts.int ? parseInt(v, 10) : v;
      Storage.set(key, parsed);
      setActive(groupId, v);
      toast((opts && opts.label || key) + ': ' + btn.textContent.trim());
      if (opts && opts.onChange) opts.onChange(parsed);
      updatePreview();
    });
  }

  // === RSS Feeds ===
  async function loadFeeds() {
    const list = document.getElementById('feeds-list');
    try {
      const res = await fetch(`${BASE}/api/admin/feeds`, { headers: { ...adminHeaders() } });
      if (!res.ok) { list.innerHTML = '<div class="loading-text">Failed to load feeds</div>'; return; }
      const data = await res.json();
      if (!data.feeds || data.feeds.length === 0) {
        list.innerHTML = '<div class="loading-text">No feeds configured</div>';
        return;
      }
      list.innerHTML = '';
      for (const feed of data.feeds) {
        const item = document.createElement('div');
        item.className = 'feed-item';
        item.innerHTML = `
          <div class="feed-info">
            <div class="feed-source">${esc(feed.source)}</div>
            <div class="feed-url">${esc(feed.url)}</div>
          </div>
          <span class="feed-category">${esc(feed.category)}</span>
          <button class="feed-toggle ${feed.enabled ? 'on' : 'off'}" data-id="${esc(feed.feed_id)}"></button>
          <button class="feed-delete" data-id="${esc(feed.feed_id)}">&times;</button>
        `;
        list.appendChild(item);
      }
      list.querySelectorAll('.feed-toggle').forEach(btn => {
        btn.addEventListener('click', async () => {
          const id = btn.dataset.id;
          const newEnabled = btn.classList.contains('off');
          try {
            const res = await fetch(`${BASE}/api/admin/feeds/${id}`, {
              method: 'PUT',
              headers: { 'Content-Type': 'application/json', ...adminHeaders() },
              body: JSON.stringify({ enabled: newEnabled }),
            });
            if (res.ok) {
              btn.classList.toggle('on', newEnabled);
              btn.classList.toggle('off', !newEnabled);
              toast(newEnabled ? 'Feed enabled' : 'Feed disabled');
            }
          } catch(e) { toast('Error: ' + e.message); }
        });
      });
      list.querySelectorAll('.feed-delete').forEach(btn => {
        btn.addEventListener('click', async () => {
          if (!confirm('Delete this feed?')) return;
          try {
            const res = await fetch(`${BASE}/api/admin/feeds/${btn.dataset.id}`, {
              method: 'DELETE', headers: { ...adminHeaders() },
            });
            if (res.ok) { btn.closest('.feed-item').remove(); toast('Feed deleted'); }
          } catch(e) { toast('Error: ' + e.message); }
        });
      });
    } catch { list.innerHTML = '<div class="loading-text">Error loading feeds</div>'; }
  }

  document.getElementById('feed-add-btn').addEventListener('click', async () => {
    const url = document.getElementById('feed-url').value.trim();
    const source = document.getElementById('feed-source').value.trim();
    const category = document.getElementById('feed-category').value;
    if (!url || !source) { toast('URL and Source are required'); return; }
    try {
      const res = await fetch(`${BASE}/api/admin/feeds`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...adminHeaders() },
        body: JSON.stringify({ url, source, category }),
      });
      if (res.ok) {
        document.getElementById('feed-url').value = '';
        document.getElementById('feed-source').value = '';
        toast('Feed added');
        loadFeeds();
      } else {
        const data = await res.json().catch(() => ({}));
        toast(data.error || 'Failed to add feed');
      }
    } catch(e) { toast('Error: ' + e.message); }
  });

  // === Local Data ===
  function updateDataCounts() {
    ReadHistory.init();
    Bookmarks.init();
    document.getElementById('history-count').textContent = ReadHistory.getCount() + ' articles';
    document.getElementById('bookmark-count').textContent = Bookmarks.getCount() + ' items';
    try {
      const eco = JSON.parse(localStorage.getItem('hn_eco') || '{}');
      document.getElementById('cache-count').textContent = (eco.queryCache ? Object.keys(eco.queryCache).length : 0) + ' entries';
    } catch { document.getElementById('cache-count').textContent = '0 entries'; }
    let total = 0;
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key && key.startsWith('hn_')) total += (localStorage.getItem(key) || '').length;
    }
    document.getElementById('storage-size').textContent = (total / 1024).toFixed(1) + ' KB';
  }

  document.getElementById('clear-history').addEventListener('click', () => { localStorage.removeItem('hn_readHistory'); toast('Read history cleared'); updateDataCounts(); });
  document.getElementById('clear-bookmarks').addEventListener('click', () => { localStorage.removeItem('hn_bookmarks'); toast('Bookmarks cleared'); updateDataCounts(); });
  document.getElementById('clear-cache').addEventListener('click', () => {
    try { const eco = JSON.parse(localStorage.getItem('hn_eco') || '{}'); eco.queryCache = {}; eco.cacheHits = 0; localStorage.setItem('hn_eco', JSON.stringify(eco)); } catch {}
    toast('AI cache cleared'); updateDataCounts();
  });
  document.getElementById('clear-all').addEventListener('click', () => {
    if (!confirm('Reset all settings to defaults?')) return;
    const keys = [];
    for (let i = 0; i < localStorage.length; i++) { const k = localStorage.key(i); if (k && k.startsWith('hn_')) keys.push(k); }
    keys.forEach(k => localStorage.removeItem(k));
    toast('All settings reset');
    setTimeout(() => location.reload(), 400);
  });

  // === Import / Export ===
  document.getElementById('export-btn').addEventListener('click', () => {
    const data = {};
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key && key.startsWith('hn_')) data[key] = localStorage.getItem(key);
    }
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = 'hypernews-settings.json';
    a.click();
    URL.revokeObjectURL(a.href);
    toast('Settings exported');
  });

  document.getElementById('import-btn').addEventListener('click', () => {
    document.getElementById('import-file').click();
  });
  document.getElementById('import-file').addEventListener('change', e => {
    const file = e.target.files[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const data = JSON.parse(reader.result);
        let count = 0;
        for (const [key, val] of Object.entries(data)) {
          if (key.startsWith('hn_')) { localStorage.setItem(key, val); count++; }
        }
        toast(`Imported ${count} settings`);
        setTimeout(() => location.reload(), 400);
      } catch { toast('Invalid settings file'); }
    };
    reader.readAsText(file);
    e.target.value = '';
  });

  // === Display Tab ===
  function initDisplay() {
    initBtnGroup('theme-btns', 'theme', { label: 'Theme' });
    initBtnGroup('mode-btns', 'mode', { label: 'Mode', onChange: v => { document.body.dataset.mode = v; } });
    initBtnGroup('density-btns', 'density', { label: 'Density' });
    initBtnGroup('refresh-btns', 'autoRefresh', { label: 'Auto refresh', int: true });

    initSlider('font-size-range', 'font-size-val', 'fontSize', 'px', v => Math.round(v));
    initSlider('line-height-range', 'line-height-val', 'lineHeight', '', v => v.toFixed(1));

    // Colors
    const accentColor = Storage.get('accentColor') || 'default';
    document.querySelectorAll('.color-btn').forEach(btn => {
      btn.classList.toggle('active', btn.dataset.value === accentColor);
    });
    document.getElementById('color-btns').addEventListener('click', e => {
      const btn = e.target.closest('.color-btn');
      if (!btn) return;
      Storage.set('accentColor', btn.dataset.value);
      document.querySelectorAll('.color-btn').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      toast('Accent: ' + btn.title);
    });
  }

  function initDisplayToggles() {
    initToggle('toggle-showImages', 'showImages');
    initToggle('toggle-showDescriptions', 'showDescriptions');
    initToggle('toggle-hideReadArticles', 'hideReadArticles');
    initToggle('toggle-enableAnimations', 'enableAnimations');
    initToggle('toggle-showSource', 'showSource');
    initToggle('toggle-showTime', 'showTime');
    initToggle('toggle-showTtsButton', 'showTtsButton');
    initToggle('toggle-showBookmarkButton', 'showBookmarkButton');
  }

  // === Layout Tab ===
  function initLayout() {
    initSlider('article-gap-range', 'article-gap-val', 'articleGap', 'px', v => Math.round(v));
    initSlider('article-padding-range', 'article-padding-val', 'articlePadding', 'px', v => Math.round(v));
    initSlider('border-radius-range', 'border-radius-val', 'borderRadius', 'px', v => Math.round(v));

    initBtnGroup('image-size-btns', 'imageSize', { label: 'Image size' });
    initBtnGroup('desc-lines-btns', 'descLines', { label: 'Desc lines', int: true });
    initBtnGroup('max-width-btns', 'contentMaxWidth', { label: 'Max width', int: true });

    updatePreview();
  }

  // === Live Preview ===
  function updatePreview() {
    const container = document.getElementById('preview-articles');
    if (!container) return;
    const gap = Storage.get('articleGap');
    const pad = Storage.get('articlePadding');
    const radius = Storage.get('borderRadius');
    const imgSize = Storage.get('imageSize');

    container.style.gap = gap + 'px';
    container.querySelectorAll('.preview-article').forEach(a => {
      a.style.padding = pad + 'px';
      a.style.borderRadius = radius + 'px';
      a.style.gap = Math.max(4, pad * 0.5) + 'px';
    });
    const imgW = imgSize === 'none' ? 0 : imgSize === 'small' ? 40 : imgSize === 'large' ? 90 : 60;
    const imgH = Math.round(imgW * 0.67);
    container.querySelectorAll('.preview-img').forEach(img => {
      img.style.display = imgSize === 'none' ? 'none' : 'block';
      img.style.width = imgW + 'px';
      img.style.height = imgH + 'px';
      img.style.borderRadius = Math.max(0, radius - 3) + 'px';
    });
  }

  // === Reading Tab ===
  function initReading() {
    initBtnGroup('articles-per-page-btns', 'articlesPerPage', { label: 'Per page', int: true });
    initBtnGroup('click-action-btns', 'articleClickAction', { label: 'Click action' });
    initBtnGroup('read-mark-btns', 'readMarkDelay', { label: 'Read mark', int: true });
    initToggle('toggle-infiniteScroll', 'infiniteScroll');
  }

  // === AI & Voice Tab ===
  function initAI() {
    document.getElementById('ai-question-prompt').value = Storage.get('aiQuestionPrompt') || '';
    document.getElementById('ai-answer-prompt').value = Storage.get('aiAnswerPrompt') || '';

    document.getElementById('ai-save-btn').addEventListener('click', () => {
      Storage.set('aiQuestionPrompt', document.getElementById('ai-question-prompt').value);
      Storage.set('aiAnswerPrompt', document.getElementById('ai-answer-prompt').value);
      toast('AI prompts saved');
    });
    document.getElementById('ai-clear-btn').addEventListener('click', () => {
      document.getElementById('ai-question-prompt').value = '';
      document.getElementById('ai-answer-prompt').value = '';
      Storage.set('aiQuestionPrompt', '');
      Storage.set('aiAnswerPrompt', '');
      toast('AI prompts cleared');
    });

    initBtnGroup('typewriter-btns', 'typewriterSpeed', { label: 'Typewriter', int: true });
    loadVoicePicker();

    // Murmur toggle
    initToggle('toggle-murmur', 'feed_murmur');

    // EcoSystem Cache Rate
    initSlider('cache-rate-range', 'cache-rate-val', 'ecoCacheRate', '%', v => Math.round(v));

    // Voice Clone
    CloneVoices.init();
    initVoiceClone();
  }

  async function loadVoicePicker() {
    const container = document.getElementById('tts-voice-btns');
    const hint = document.getElementById('tts-voice-hint');
    const currentVoice = Storage.get('ttsVoice') || 'off';
    try {
      const res = await fetch(`${BASE}/api/tts/voices`);
      if (!res.ok) throw new Error('fail');
      const data = await res.json();
      const voices = data.voices || [];
      container.innerHTML = '<button class="opt-btn" data-value="off">OFF</button>';
      for (const v of voices) {
        const btn = document.createElement('button');
        btn.className = 'opt-btn';
        btn.dataset.value = v.id || v.name;
        btn.textContent = v.label || v.name || v.id;
        container.appendChild(btn);
      }
      setActive('tts-voice-btns', currentVoice);
      hint.textContent = voices.length > 0 ? '' : 'No voices available';
    } catch {
      hint.textContent = 'Could not load voices';
      setActive('tts-voice-btns', currentVoice);
    }
    container.addEventListener('click', e => {
      const btn = e.target.closest('.opt-btn');
      if (!btn) return;
      Storage.set('ttsVoice', btn.dataset.value);
      setActive('tts-voice-btns', btn.dataset.value);
      toast('TTS Voice: ' + (btn.dataset.value === 'off' ? 'OFF' : btn.textContent));
    });
  }

  function initVoiceClone() {
    const list = document.getElementById('clone-list');
    const form = document.getElementById('clone-form');
    const newBtn = document.getElementById('clone-new-btn');
    const recBtn = document.getElementById('clone-rec-btn');
    const recStatus = document.getElementById('clone-rec-status');
    const recTimeEl = document.getElementById('clone-rec-time');
    const recStopBtn = document.getElementById('clone-rec-stop');
    const fileInput = document.getElementById('clone-file');
    const audioPreview = document.getElementById('clone-audio-preview');
    const saveBtn = document.getElementById('clone-save-btn');
    const cancelBtn = document.getElementById('clone-cancel-btn');
    const countEl = document.getElementById('clone-count');

    let mediaRecorder = null;
    let audioChunks = [];
    let recTimer = null;
    let recStartTime = 0;
    let currentAudioBase64 = null;

    renderCloneList();

    newBtn.addEventListener('click', () => {
      if (CloneVoices.getCount() >= CloneVoices.MAX_CLONES) {
        toast('クローン上限(' + CloneVoices.MAX_CLONES + '件)に達しています');
        return;
      }
      form.hidden = !form.hidden;
      if (!form.hidden) resetForm();
    });

    recBtn.addEventListener('click', async () => {
      if (mediaRecorder && mediaRecorder.state === 'recording') return;
      try {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        audioChunks = [];
        mediaRecorder = new MediaRecorder(stream, { mimeType: MediaRecorder.isTypeSupported('audio/webm') ? 'audio/webm' : '' });
        mediaRecorder.ondataavailable = e => audioChunks.push(e.data);
        mediaRecorder.onstop = () => {
          stream.getTracks().forEach(t => t.stop());
          const blob = new Blob(audioChunks, { type: mediaRecorder.mimeType || 'audio/webm' });
          processAudioBlob(blob);
          recBtn.classList.remove('recording');
          recStatus.hidden = true;
          clearInterval(recTimer);
        };
        mediaRecorder.start();
        recBtn.classList.add('recording');
        recStatus.hidden = false;
        recStartTime = Date.now();
        recTimer = setInterval(() => {
          const elapsed = Math.floor((Date.now() - recStartTime) / 1000);
          recTimeEl.textContent = Math.floor(elapsed / 60) + ':' + String(elapsed % 60).padStart(2, '0');
        }, 200);
      } catch {
        toast('マイクのアクセスが許可されていません');
      }
    });

    recStopBtn.addEventListener('click', () => {
      if (mediaRecorder && mediaRecorder.state === 'recording') mediaRecorder.stop();
    });

    fileInput.addEventListener('change', e => {
      const file = e.target.files[0];
      if (!file) return;
      if (file.size > 5 * 1024 * 1024) { toast('ファイルが大きすぎます (5MB以下)'); return; }
      processAudioBlob(file);
      e.target.value = '';
    });

    function processAudioBlob(blob) {
      const reader = new FileReader();
      reader.onload = () => {
        const dataUrl = reader.result;
        // Strip data URL prefix to get raw base64
        currentAudioBase64 = dataUrl.split(',')[1] || dataUrl;
        audioPreview.src = dataUrl;
        audioPreview.hidden = false;
        updateSaveBtn();
      };
      reader.readAsDataURL(blob);
    }

    function updateSaveBtn() {
      const name = document.getElementById('clone-name').value.trim();
      const refText = document.getElementById('clone-ref-text').value.trim();
      saveBtn.disabled = !(name && currentAudioBase64 && refText);
    }

    document.getElementById('clone-name').addEventListener('input', updateSaveBtn);
    document.getElementById('clone-ref-text').addEventListener('input', updateSaveBtn);

    saveBtn.addEventListener('click', () => {
      const name = document.getElementById('clone-name').value.trim();
      const refText = document.getElementById('clone-ref-text').value.trim();
      if (!name || !currentAudioBase64 || !refText) { toast('全項目を入力してください'); return; }
      const id = CloneVoices.add(name, currentAudioBase64, refText);
      if (!id) { toast('クローン上限(' + CloneVoices.MAX_CLONES + '件)に達しています'); return; }
      renderCloneList();
      form.hidden = true;
      toast('ボイス「' + name + '」を保存しました');
    });

    cancelBtn.addEventListener('click', () => {
      form.hidden = true;
      if (mediaRecorder && mediaRecorder.state === 'recording') mediaRecorder.stop();
    });

    function resetForm() {
      document.getElementById('clone-name').value = '';
      document.getElementById('clone-ref-text').value = '';
      currentAudioBase64 = null;
      audioPreview.hidden = true;
      audioPreview.src = '';
      saveBtn.disabled = true;
      recBtn.classList.remove('recording');
      recStatus.hidden = true;
      clearInterval(recTimer);
      countEl.textContent = CloneVoices.getCount();
    }

    function renderCloneList() {
      const clones = CloneVoices.getAll();
      if (clones.length === 0) {
        list.innerHTML = '<p class="clone-empty">クローンボイスはまだありません</p>';
      } else {
        list.innerHTML = clones.map(c => `
          <div class="clone-card" data-id="${esc(c.id)}">
            <span class="clone-card-name">${esc(c.name)}</span>
            <div class="clone-card-actions">
              <button class="btn-secondary clone-test-btn" title="テスト再生">▶</button>
              <button class="btn-secondary clone-use-btn" title="このボイスを使用">使用</button>
              <button class="btn-secondary clone-del-btn" title="削除">✕</button>
            </div>
          </div>
        `).join('');
      }
      countEl.textContent = CloneVoices.getCount();

      // Hide new button when at limit
      newBtn.style.display = CloneVoices.getCount() >= CloneVoices.MAX_CLONES ? 'none' : '';

      // Event delegation for clone cards
      list.querySelectorAll('.clone-test-btn').forEach(btn => {
        btn.addEventListener('click', async () => {
          const card = btn.closest('.clone-card');
          const id = card.dataset.id;
          const clone = CloneVoices.get(id);
          if (!clone) return;
          btn.disabled = true;
          btn.textContent = '…';
          try {
            const auth = typeof Subscription !== 'undefined' ? Subscription.authHeaders() : {};
            const res = await fetch('/api/tts/clone', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json', ...auth },
              body: JSON.stringify({ text: 'これはテスト再生です。ボイスクローンの音声をお楽しみください。', ref_audio: clone.refAudio, ref_text: clone.refText, language: 'Japanese' }),
            });
            if (!res.ok) throw new Error('TTS error');
            const blob = await res.blob();
            const url = URL.createObjectURL(blob);
            const audio = new Audio(url);
            audio.onended = () => URL.revokeObjectURL(url);
            audio.play();
          } catch {
            toast('テスト再生に失敗しました');
          } finally {
            btn.disabled = false;
            btn.textContent = '▶';
          }
        });
      });

      list.querySelectorAll('.clone-use-btn').forEach(btn => {
        btn.addEventListener('click', () => {
          const card = btn.closest('.clone-card');
          const id = card.dataset.id;
          const clone = CloneVoices.get(id);
          if (!clone) return;
          Storage.set('ttsVoice', 'clone:' + id);
          setActive('tts-voice-btns', 'clone:' + id);
          toast('ボイス「' + clone.name + '」を使用中');
        });
      });

      list.querySelectorAll('.clone-del-btn').forEach(btn => {
        btn.addEventListener('click', () => {
          const card = btn.closest('.clone-card');
          const id = card.dataset.id;
          const clone = CloneVoices.get(id);
          if (!clone) return;
          if (!confirm('「' + clone.name + '」を削除しますか？')) return;
          // If currently using this voice, reset to off
          if (Storage.get('ttsVoice') === 'clone:' + id) {
            Storage.set('ttsVoice', 'off');
            setActive('tts-voice-btns', 'off');
          }
          CloneVoices.remove(id);
          renderCloneList();
          toast('ボイスを削除しました');
        });
      });
    }
  }

  function esc(str) {
    const div = document.createElement('div');
    div.textContent = str || '';
    return div.innerHTML;
  }

  // --- Init ---
  loadFeeds();
  updateDataCounts();
  initDisplay();
  initDisplayToggles();
  initLayout();
  initReading();
  initAI();
})();
