/**
 * FeedMurmur — AI murmur mode for the vertical swipe feed.
 * Generates a short casual AI comment + TTS audio for each article.
 */
const FeedMurmur = (function () {
  'use strict';

  let enabled = false;
  let subtitleEl = null;
  let currentAudio = null;
  let abortCtrl = null;
  let dwellTimer = null;
  const cache = new Map(); // title -> { text, audio_base64 }

  function init() {
    enabled = localStorage.getItem('feed_murmur') === 'true';
    // Create subtitle element
    subtitleEl = document.createElement('div');
    subtitleEl.className = 'murmur-subtitle';
    subtitleEl.innerHTML = '<span class="murmur-text"></span>';
    document.body.appendChild(subtitleEl);
  }

  function isEnabled() {
    return enabled;
  }

  function setEnabled(val) {
    enabled = val;
    localStorage.setItem('feed_murmur', val ? 'true' : 'false');
    if (!val) {
      cancel();
      hideSubtitle();
    }
  }

  function toggle() {
    setEnabled(!enabled);
    return enabled;
  }

  /** Called when a feed item is deactivated (scrolled away). */
  function onArticleDeactivated() {
    cancel();
    hideSubtitle();
  }

  /** Called when a feed item becomes active (visible). */
  function onArticleActivated(item) {
    if (!enabled) return;

    // Cancel any previous request/playback
    cancel();
    hideSubtitle();

    // Don't murmur if podcast is playing
    if (item.classList.contains('playing')) return;

    const title = item.dataset.title || '';
    const description = item.dataset.description || '';
    const source = item.dataset.source || '';
    if (!title) return;

    // Dwell timer — only request after 1.5s to avoid wasting API calls on fast swipes
    dwellTimer = setTimeout(() => {
      dwellTimer = null;
      triggerMurmur(title, description, source);
    }, 1500);
  }

  function cancel() {
    if (dwellTimer) {
      clearTimeout(dwellTimer);
      dwellTimer = null;
    }
    if (abortCtrl) {
      abortCtrl.abort();
      abortCtrl = null;
    }
    if (currentAudio) {
      currentAudio.pause();
      currentAudio = null;
    }
    if (window.speechSynthesis) {
      window.speechSynthesis.cancel();
    }
  }

  async function triggerMurmur(title, description, source) {
    const cacheKey = title + '|' + source;

    // Check local cache
    if (cache.has(cacheKey)) {
      const cached = cache.get(cacheKey);
      playMurmur(cached.text, cached.audio_base64);
      return;
    }

    // Show loading state
    showSubtitle('', true);

    abortCtrl = new AbortController();
    try {
      const resp = await fetch('/api/murmur/generate', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Device-Id': getDeviceId(),
        },
        body: JSON.stringify({ title, description, source }),
        signal: abortCtrl.signal,
      });

      if (!resp.ok) {
        if (resp.status === 402) {
          // Rate limit — auto-disable
          setEnabled(false);
          hideSubtitle();
          return;
        }
        hideSubtitle();
        return;
      }

      const data = await resp.json();
      cache.set(cacheKey, data);
      playMurmur(data.text, data.audio_base64);
    } catch (e) {
      if (e.name !== 'AbortError') {
        hideSubtitle();
      }
    } finally {
      abortCtrl = null;
    }
  }

  function playMurmur(text, audioBase64) {
    if (!text) {
      hideSubtitle();
      return;
    }

    showSubtitle(text, false);

    if (audioBase64) {
      // Play base64 audio
      const audio = new Audio('data:audio/wav;base64,' + audioBase64);
      currentAudio = audio;
      audio.volume = 0.8;
      audio.play().then(() => {
        audio.addEventListener('ended', () => {
          currentAudio = null;
          setTimeout(() => fadeOutSubtitle(), 1000);
        }, { once: true });
      }).catch(() => {
        // Autoplay blocked — fallback to Web Speech API
        currentAudio = null;
        speakFallback(text);
      });
    } else {
      // No audio — use Web Speech API
      speakFallback(text);
    }
  }

  function speakFallback(text) {
    if (!window.speechSynthesis) {
      setTimeout(() => fadeOutSubtitle(), 3000);
      return;
    }
    const utterance = new SpeechSynthesisUtterance(text);
    utterance.lang = 'ja-JP';
    utterance.rate = 1.0;
    utterance.volume = 0.7;
    utterance.onend = () => {
      setTimeout(() => fadeOutSubtitle(), 1000);
    };
    window.speechSynthesis.speak(utterance);
  }

  function showSubtitle(text, loading) {
    if (!subtitleEl) return;
    const textEl = subtitleEl.querySelector('.murmur-text');
    if (textEl) textEl.textContent = text;
    subtitleEl.classList.remove('fade-out');
    subtitleEl.classList.toggle('loading', !!loading);
    subtitleEl.classList.add('visible');
  }

  function hideSubtitle() {
    if (!subtitleEl) return;
    subtitleEl.classList.remove('visible', 'fade-out', 'loading');
  }

  function fadeOutSubtitle() {
    if (!subtitleEl) return;
    subtitleEl.classList.add('fade-out');
    subtitleEl.classList.remove('loading');
    setTimeout(() => {
      subtitleEl.classList.remove('visible', 'fade-out');
    }, 600);
  }

  function getDeviceId() {
    if (typeof HNStorage !== 'undefined' && HNStorage.getDeviceId) {
      return HNStorage.getDeviceId();
    }
    let id = localStorage.getItem('device_id');
    if (!id) {
      id = crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2);
      localStorage.setItem('device_id', id);
    }
    return id;
  }

  return { init, isEnabled, setEnabled, toggle, onArticleActivated, onArticleDeactivated };
})();
