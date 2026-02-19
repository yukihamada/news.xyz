/**
 * assistant.js - AI Assistant feature for news.xyz
 * Provides interactive Q&A with caster mode
 */
'use strict';

const Assistant = (() => {
  // ===== Constants =====
  const MODE_CASTER = 'caster';
  const MODE_FRIEND = 'friend';
  const MODE_SCHOLAR = 'scholar';
  const MODE_ENTERTAINER = 'entertainer';
  const MAX_HISTORY_LENGTH = 3;
  const API_QUESTIONS_ENDPOINT = '/api/articles/questions';
  const API_ASK_ENDPOINT = '/api/articles/ask';

  // ===== State Management =====
  const state = new Map(); // articleId -> AssistantState

  class AssistantState {
    constructor(articleId, articleData) {
      this.articleId = articleId;
      this.articleData = articleData;
      this.mode = MODE_CASTER;
      this.conversationHistory = [];
      this.currentSuggestions = [];
      this.isGenerating = false;
      this.isSpeaking = false;
      this.uiElements = {
        container: null,
        modeToggle: null,
        suggestionsContainer: null,
        answerDrawer: null,
        audioControl: null,
      };
    }
  }

  // ===== HTML Utilities =====
  function escHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }

  // ===== API Calls =====
  async function fetchSuggestions(articleData, retryCount = 0) {
    const headers = { 'Content-Type': 'application/json' };

    // Add device ID if Subscription module is available
    if (typeof Subscription !== 'undefined' && Subscription.getDeviceId) {
      headers['X-Device-Id'] = Subscription.getDeviceId();
    }

    try {
      const response = await fetch(API_QUESTIONS_ENDPOINT, {
        method: 'POST',
        headers,
        body: JSON.stringify({
          title: articleData.title,
          description: articleData.description,
          source: articleData.source,
          url: articleData.url || ''
        })
      });

      if (!response.ok) {
        const errorText = await response.text().catch(() => 'Unknown error');
        console.error(`Failed to fetch suggestions (${response.status}):`, errorText);
        throw new Error(`Failed to fetch suggestions: ${response.status}`);
      }

      const data = await response.json();
      return data.questions || [];
    } catch (error) {
      // Retry up to 3 times with exponential backoff
      if (retryCount < 3) {
        const delay = Math.pow(2, retryCount) * 1000; // 1s, 2s, 4s
        console.log(`Retrying in ${delay}ms... (attempt ${retryCount + 1}/3)`);
        await new Promise(resolve => setTimeout(resolve, delay));
        return fetchSuggestions(articleData, retryCount + 1);
      }
      throw error;
    }
  }

  async function fetchAnswer(articleData, question, mode) {
    const modePrompts = {
      [MODE_CASTER]: `${question}

【回答スタイル指示】
あなたはプロのニュースキャスターです。以下の条件で回答してください：
- 客観的で正確な情報提供
- 専門用語は簡潔に解説
- 300-600文字
- 背景・影響・今後の見通しを含める`,

      [MODE_FRIEND]: `${question}

【回答スタイル指示】
あなたは親しい友人として話しています。以下の条件で回答してください：
- カジュアルでフレンドリーな口調（「〜だよ」「〜だね」「〜なんだ」）
- 難しい話題も分かりやすく、身近な例えを使って説明
- 300-600文字
- 個人的な感想や「これってさ、」といった語りかけを交えてOK`,

      [MODE_SCHOLAR]: `${question}

【回答スタイル指示】
あなたは専門知識を持つ学者として話しています。以下の条件で回答してください：
- 学術的で詳細な分析と解説
- データ、統計、歴史的背景などの具体的な情報を含める
- 専門用語も使いつつ、丁寧に説明
- 400-700文字（通常より詳しく）
- 「〜である」「〜と考えられる」などの論理的な口調`,

      [MODE_ENTERTAINER]: `${question}

【回答スタイル指示】
あなたはエンタメ系解説者として話しています。以下の条件で回答してください：
- 面白おかしく、ユーモアを交えた解説
- 「マジか！」「すごくない？」などの感嘆符を使用
- 比喩や大げさな表現で興味を引く
- 300-600文字
- 笑いや驚きを誘いつつも、正確な情報を提供`
    };

    const enhancedQuestion = modePrompts[mode] || question;

    const headers = { 'Content-Type': 'application/json' };

    // Add device ID if Subscription module is available
    if (typeof Subscription !== 'undefined' && Subscription.getDeviceId) {
      headers['X-Device-Id'] = Subscription.getDeviceId();
    }

    const response = await fetch(API_ASK_ENDPOINT, {
      method: 'POST',
      headers,
      body: JSON.stringify({
        title: articleData.title,
        description: articleData.description,
        source: articleData.source,
        question: enhancedQuestion,
        url: articleData.url || ''
      })
    });

    if (!response.ok) {
      throw new Error(`Failed to fetch answer: ${response.status}`);
    }

    const data = await response.json();
    return data.answer || '';
  }

  // ===== UI Creation =====
  function createAssistantUI(articleId, assistantState) {
    const container = document.createElement('div');
    container.className = 'assistant-container';
    container.dataset.articleId = articleId;

    // Suggestions container
    const suggestionsContainer = document.createElement('div');
    suggestionsContainer.className = 'assistant-suggestions';
    container.appendChild(suggestionsContainer);

    // Answer drawer (initially hidden)
    const answerDrawer = document.createElement('div');
    answerDrawer.className = 'assistant-answer-drawer';
    answerDrawer.hidden = true;
    container.appendChild(answerDrawer);

    // Mode selector (bottom-left, play button style)
    const modeSelector = document.createElement('div');
    modeSelector.className = 'assistant-mode-selector';
    modeSelector.hidden = true; // Initially hidden, shown after first TTS playback
    modeSelector.innerHTML = `
      <button class="assistant-mode-trigger" aria-label="AIモード選択">
        <svg class="mode-trigger-icon" viewBox="0 0 24 24" fill="currentColor">
          <path d="M8 5v14l11-7z"/>
        </svg>
      </button>
      <div class="assistant-mode-menu" hidden>
        <button class="assistant-mode-option assistant-mode-option--active" data-mode="${MODE_CASTER}">
          <span class="mode-icon">📺</span>
          <span class="mode-name">キャスター</span>
          <span class="mode-desc">プロの解説</span>
        </button>
        <button class="assistant-mode-option" data-mode="${MODE_FRIEND}">
          <span class="mode-icon">💬</span>
          <span class="mode-name">友達</span>
          <span class="mode-desc">カジュアル</span>
        </button>
        <button class="assistant-mode-option" data-mode="${MODE_SCHOLAR}">
          <span class="mode-icon">🎓</span>
          <span class="mode-name">学者</span>
          <span class="mode-desc">詳しく分析</span>
        </button>
        <button class="assistant-mode-option" data-mode="${MODE_ENTERTAINER}">
          <span class="mode-icon">🎭</span>
          <span class="mode-name">エンタメ</span>
          <span class="mode-desc">楽しく解説</span>
        </button>
      </div>
    `;
    container.appendChild(modeSelector);

    // Mode selector event listeners
    const modeTrigger = modeSelector.querySelector('.assistant-mode-trigger');
    const modeMenu = modeSelector.querySelector('.assistant-mode-menu');
    const modeOptions = modeSelector.querySelectorAll('.assistant-mode-option');

    modeTrigger.addEventListener('click', (e) => {
      e.stopPropagation();
      modeMenu.hidden = !modeMenu.hidden;
    });

    modeOptions.forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const newMode = btn.dataset.mode;
        handleModeToggle(assistantState, newMode, modeSelector);
        modeMenu.hidden = true;
      });
    });

    // Close menu when clicking outside
    document.addEventListener('click', () => {
      modeMenu.hidden = true;
    });

    // Store UI elements
    assistantState.uiElements = {
      container,
      modeSelector,
      suggestionsContainer,
      answerDrawer,
      audioControl: null,
    };

    return container;
  }

  function renderSuggestions(assistantState, suggestions) {
    const container = assistantState.uiElements.suggestionsContainer;
    container.innerHTML = '';

    if (suggestions.length === 0) {
      container.innerHTML = '<div class="assistant-suggestions-empty">質問を生成中...</div>';
      return;
    }

    suggestions.forEach((suggestion, index) => {
      const chip = document.createElement('button');
      chip.className = 'assistant-suggestion-chip';
      chip.type = 'button';
      chip.textContent = suggestion;
      chip.dataset.suggestion = suggestion;
      chip.addEventListener('click', () => handleSuggestionClick(assistantState.articleId, suggestion));
      container.appendChild(chip);
    });
  }

  function showAnswerDrawer(assistantState, question, answer) {
    const drawer = assistantState.uiElements.answerDrawer;
    drawer.innerHTML = `
      <div class="assistant-answer-header">
        <div class="assistant-answer-question">
          <strong>Q:</strong> ${escHtml(question)}
        </div>
        <button class="assistant-answer-close" aria-label="Close">×</button>
      </div>
      <div class="assistant-answer-body">
        <div class="assistant-answer-text">${escHtml(answer)}</div>
      </div>
      <div class="assistant-audio-control">
        <button class="assistant-audio-btn assistant-audio-btn--pause" aria-label="Pause">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <rect x="6" y="4" width="4" height="16"/>
            <rect x="14" y="4" width="4" height="16"/>
          </svg>
        </button>
        <div class="assistant-audio-wave">
          <div class="assistant-audio-wave-bar"></div>
          <div class="assistant-audio-wave-bar"></div>
          <div class="assistant-audio-wave-bar"></div>
          <div class="assistant-audio-wave-bar"></div>
          <div class="assistant-audio-wave-bar"></div>
        </div>
      </div>
    `;
    drawer.hidden = false;

    // Add close button handler
    const closeBtn = drawer.querySelector('.assistant-answer-close');
    closeBtn.addEventListener('click', () => {
      drawer.hidden = true;
      stopAudio(assistantState);
    });

    // Add pause/resume button handler
    const audioBtn = drawer.querySelector('.assistant-audio-btn');
    assistantState.uiElements.audioControl = audioBtn;
    audioBtn.addEventListener('click', () => toggleAudio(assistantState));
  }

  function showLoadingState(assistantState) {
    const container = assistantState.uiElements.suggestionsContainer;
    container.innerHTML = '<div class="assistant-loading">回答を生成中...</div>';
  }

  function showErrorState(assistantState, message) {
    const container = assistantState.uiElements.suggestionsContainer;
    container.innerHTML = `<div class="assistant-error">${escHtml(message)}</div>`;
  }

  function updateAudioUI(assistantState, status) {
    const audioBtn = assistantState.uiElements.audioControl;
    const wave = assistantState.uiElements.answerDrawer?.querySelector('.assistant-audio-wave');

    if (!audioBtn || !wave) return;

    switch (status) {
      case 'playing':
        audioBtn.className = 'assistant-audio-btn assistant-audio-btn--pause';
        audioBtn.innerHTML = `
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <rect x="6" y="4" width="4" height="16"/>
            <rect x="14" y="4" width="4" height="16"/>
          </svg>
        `;
        wave.classList.add('assistant-audio-wave--playing');
        break;
      case 'paused':
        audioBtn.className = 'assistant-audio-btn assistant-audio-btn--play';
        audioBtn.innerHTML = `
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <polygon points="5 3 19 12 5 21"/>
          </svg>
        `;
        wave.classList.remove('assistant-audio-wave--playing');
        break;
      case 'ended':
      case 'error':
        wave.classList.remove('assistant-audio-wave--playing');
        break;
    }
  }

  // ===== Event Handlers =====
  function handleModeToggle(assistantState, newMode, modeSelector) {
    if (assistantState.isGenerating || assistantState.isSpeaking) {
      console.log('Cannot change mode while generating or speaking');
      return;
    }

    assistantState.mode = newMode;

    // Update UI
    const modeLabels = {
      [MODE_CASTER]: 'キャスター',
      [MODE_FRIEND]: '友達',
      [MODE_SCHOLAR]: '学者',
      [MODE_ENTERTAINER]: 'エンタメ'
    };

    const trigger = modeSelector.querySelector('.assistant-mode-trigger .mode-trigger-label');
    trigger.textContent = modeLabels[newMode];

    // Update active state
    const options = modeSelector.querySelectorAll('.assistant-mode-option');
    options.forEach(opt => {
      if (opt.dataset.mode === newMode) {
        opt.classList.add('assistant-mode-option--active');
      } else {
        opt.classList.remove('assistant-mode-option--active');
      }
    });

    console.log(`Mode changed to: ${newMode}`);
  }

  async function handleSuggestionClick(articleId, question) {
    const assistantState = state.get(articleId);
    if (!assistantState) return;

    if (assistantState.isGenerating || assistantState.isSpeaking) {
      console.log('Already generating or speaking, ignoring click');
      return;
    }

    assistantState.isGenerating = true;
    showLoadingState(assistantState);

    try {
      const answer = await fetchAnswer(
        assistantState.articleData,
        question,
        assistantState.mode
      );

      assistantState.conversationHistory.push({ question, answer });
      if (assistantState.conversationHistory.length > MAX_HISTORY_LENGTH) {
        assistantState.conversationHistory.shift();
      }

      showAnswerDrawer(assistantState, question, answer);
      await playAnswer(assistantState, answer);

      // Generate action plan after first answer
      if (assistantState.conversationHistory.length === 1) {
        showActionPlan(assistantState.articleId);
      }

      // After audio completes, generate next suggestions
      await generateNextSuggestions(assistantState);
    } catch (error) {
      console.error('Failed to process question:', error);
      showErrorState(assistantState, '回答の生成に失敗しました');
    } finally {
      assistantState.isGenerating = false;
    }
  }

  async function playAnswer(assistantState, text) {
    if (typeof Tts === 'undefined') {
      console.warn('Tts module not available');
      return;
    }

    // Save current voice setting
    const originalVoice = Tts.getStyle();

    // Get mode-specific voice preference (if set)
    const modeVoiceKey = `assistant_voice_${assistantState.mode}`;
    const modeVoice = localStorage.getItem(modeVoiceKey);

    // Switch to mode-specific voice if configured
    if (modeVoice && modeVoice !== 'default') {
      Tts.setStyle(modeVoice);
      console.log(`Using ${assistantState.mode} mode voice:`, modeVoice);
    }

    assistantState.isSpeaking = true;
    updateAudioUI(assistantState, 'playing');

    try {
      await new Promise((resolve, reject) => {
        const success = Tts.speakText(text, () => {
          assistantState.isSpeaking = false;
          updateAudioUI(assistantState, 'ended');

          // Show mode selector after first playback
          if (assistantState.uiElements?.modeSelector) {
            assistantState.uiElements.modeSelector.hidden = false;
          }

          // Restore original voice
          if (modeVoice && modeVoice !== 'default') {
            Tts.setStyle(originalVoice);
          }

          resolve();
        });

        if (!success) {
          assistantState.isSpeaking = false;
          updateAudioUI(assistantState, 'error');

          // Restore original voice
          if (modeVoice && modeVoice !== 'default') {
            Tts.setStyle(originalVoice);
          }

          reject(new Error('TTS not configured'));
        }
      });
    } catch (error) {
      console.error('Failed to play audio:', error);
      assistantState.isSpeaking = false;
      updateAudioUI(assistantState, 'error');

      // Restore original voice on error
      if (modeVoice && modeVoice !== 'default') {
        Tts.setStyle(originalVoice);
      }
    }
  }

  function toggleAudio(assistantState) {
    if (typeof Tts === 'undefined') return;

    if (assistantState.isSpeaking) {
      Tts.stop();
      assistantState.isSpeaking = false;
      updateAudioUI(assistantState, 'paused');
    }
  }

  function stopAudio(assistantState) {
    if (typeof Tts === 'undefined') return;
    Tts.stop();
    assistantState.isSpeaking = false;
  }

  async function generateNextSuggestions(assistantState) {
    // Show loading state
    const container = assistantState.uiElements.suggestionsContainer;
    container.innerHTML = '<div class="assistant-loading">次の質問を生成中...</div>';

    try {
      const suggestions = await fetchSuggestions(assistantState.articleData);
      assistantState.currentSuggestions = suggestions;
      renderSuggestions(assistantState, suggestions);
    } catch (error) {
      console.error('Failed to generate next suggestions:', error);

      // Show error with retry button
      container.innerHTML = `
        <div class="assistant-error-with-retry">
          <div class="assistant-error">次の質問の生成に失敗しました</div>
          <button class="assistant-retry-btn" type="button">再試行</button>
        </div>
      `;

      const retryBtn = container.querySelector('.assistant-retry-btn');
      retryBtn.addEventListener('click', () => generateNextSuggestions(assistantState));
    }
  }

  // ===== Article Initialization =====
  function getArticleData(articleEl) {
    const titleLink = articleEl.querySelector('.article-title a');
    return {
      id: articleEl.dataset.articleId || '',
      title: titleLink?.textContent?.trim() || '',
      description: articleEl.querySelector('.article-desc')?.textContent?.trim() || '',
      source: articleEl.querySelector('.article-source')?.textContent?.trim() || '',
      url: titleLink?.href || '',
    };
  }

  async function initializeArticle(articleEl) {
    const articleId = articleEl.dataset.articleId;
    if (!articleId) {
      console.warn('Article element missing data-article-id');
      return;
    }

    if (state.has(articleId)) {
      console.log('Article already initialized:', articleId);
      return;
    }

    const articleData = getArticleData(articleEl);
    const assistantState = new AssistantState(articleId, articleData);
    state.set(articleId, assistantState);

    // Create UI (but don't append yet)
    const assistantUI = createAssistantUI(articleId, assistantState);

    // Generate initial suggestions in background (silently)
    fetchSuggestions(articleData).then(suggestions => {
      if (suggestions && suggestions.length > 0) {
        assistantState.currentSuggestions = suggestions;
        // Only show UI if we successfully got suggestions
        articleEl.appendChild(assistantUI);
        renderSuggestions(assistantState, suggestions);
      }
    }).catch(error => {
      console.error('Failed to generate suggestions (silent):', error);
      // Don't show any error to user
    });
  }

  // ===== Initialization =====
  function init() {
    // Only run on news.xyz
    if (document.documentElement.dataset.site !== 'xyz') {
      console.log('Assistant: Not on news.xyz, skipping initialization');
      return;
    }

    console.log('Assistant: Initializing for news.xyz');

    // Initialize existing articles
    const existingArticles = document.querySelectorAll('.article');
    existingArticles.forEach(initializeArticle);

    // Watch for new articles
    const observer = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        mutation.addedNodes.forEach((node) => {
          if (node.nodeType === 1 && node.classList && node.classList.contains('article')) {
            initializeArticle(node);
          }
        });
      });
    });

    observer.observe(document.body, {
      childList: true,
      subtree: true
    });
  }

  // ===== Voice Settings Helper =====
  function setModeVoice(mode, voiceId) {
    const modeVoiceKey = `assistant_voice_${mode}`;
    if (voiceId === 'default' || voiceId === null) {
      localStorage.removeItem(modeVoiceKey);
      console.log(`${mode} mode: voice reset to default`);
    } else {
      localStorage.setItem(modeVoiceKey, voiceId);
      console.log(`${mode} mode: voice set to ${voiceId}`);
    }
  }

  function getModeVoice(mode) {
    const modeVoiceKey = `assistant_voice_${mode}`;
    return localStorage.getItem(modeVoiceKey) || 'default';
  }

  function listModeVoices() {
    const modes = [MODE_CASTER, MODE_FRIEND, MODE_SCHOLAR, MODE_ENTERTAINER];
    const settings = {};
    modes.forEach(mode => {
      settings[mode] = getModeVoice(mode);
    });
    console.table(settings);
    return settings;
  }

  // ===== Smart News Features =====
  async function showActionPlan(articleId) {
    const assistantState = state.get(articleId);
    if (!assistantState) return;

    const articleData = assistantState.articleData;

    try {
      const headers = { 'Content-Type': 'application/json' };

      // Add device ID if Subscription module is available
      if (typeof Subscription !== 'undefined' && Subscription.getDeviceId) {
        headers['X-Device-Id'] = Subscription.getDeviceId();
      }

      const response = await fetch('/api/articles/action-plan', {
        method: 'POST',
        headers,
        body: JSON.stringify({
          title: articleData.title,
          description: articleData.description,
          url: articleData.url
        })
      });

      if (!response.ok) throw new Error('Failed to fetch action plan');

      const data = await response.json();
      displayActionPlan(assistantState, data);
    } catch (error) {
      console.error('Failed to generate action plan:', error);
    }
  }

  function displayActionPlan(assistantState, plan) {
    const container = assistantState.uiElements.container;
    let actionPlanEl = container.querySelector('.assistant-action-plan');

    if (!actionPlanEl) {
      actionPlanEl = document.createElement('div');
      actionPlanEl.className = 'assistant-action-plan';
      container.appendChild(actionPlanEl);
    }

    const stepsHTML = plan.steps.map(step => `<li>${escHtml(step)}</li>`).join('');
    const toolsHTML = plan.tools_or_templates.map(tool => `<li>${escHtml(tool)}</li>`).join('');

    actionPlanEl.innerHTML = `
      <div class="action-plan-header">
        <span class="action-plan-icon">💡</span>
        <h3 class="action-plan-title">で、どうすればいい？</h3>
      </div>
      <div class="action-plan-summary">${escHtml(plan.summary)}</div>
      <div class="action-plan-steps">
        <h4>具体的なアクション</h4>
        <ol>${stepsHTML}</ol>
      </div>
      ${plan.tools_or_templates.length > 0 ? `
        <div class="action-plan-tools">
          <h4>使えるツール・テンプレート</h4>
          <ul>${toolsHTML}</ul>
        </div>
      ` : ''}
    `;
  }

  // ===== Public API =====
  return {
    init,
    // Voice configuration helpers (for console/settings UI)
    setModeVoice,
    getModeVoice,
    listModeVoices,
    // Smart news features
    showActionPlan,
  };
})();

// Auto-initialize
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => Assistant.init());
} else {
  Assistant.init();
}
