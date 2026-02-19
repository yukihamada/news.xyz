/**
 * konami.js — Konami code easter egg (up up down down left right left right B A)
 * Only active when chat panel is open and user is Google-authenticated.
 */
'use strict';

const Konami = (() => {
  const SEQUENCE = [
    'ArrowUp', 'ArrowUp', 'ArrowDown', 'ArrowDown',
    'ArrowLeft', 'ArrowRight', 'ArrowLeft', 'ArrowRight',
    'b', 'a',
  ];
  const CLAIMED_KEY = 'hn_konami_claimed';
  let buffer = [];

  function init() {
    document.addEventListener('keydown', onKeyDown);
  }

  function onKeyDown(e) {
    // Only active when chat panel is visible
    const panel = document.getElementById('chat-panel');
    if (!panel || panel.hidden) return;

    // Already claimed locally
    if (localStorage.getItem(CLAIMED_KEY) === '1') return;

    // Must be Google-authenticated
    if (typeof GoogleAuth === 'undefined' || !GoogleAuth.isAuthenticated()) return;

    buffer.push(e.key.length === 1 ? e.key.toLowerCase() : e.key);
    if (buffer.length > SEQUENCE.length) {
      buffer = buffer.slice(-SEQUENCE.length);
    }

    if (buffer.length === SEQUENCE.length &&
        buffer.every((k, i) => k === SEQUENCE[i])) {
      buffer = [];
      activate();
    }
  }

  async function activate() {
    // Call backend
    const token = GoogleAuth.getAuthToken();
    if (!token) return;

    try {
      const res = await fetch('/api/auth/konami', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`,
        },
      });

      const data = await res.json();

      if (data.success) {
        localStorage.setItem(CLAIMED_KEY, '1');
        // Award tokens in EcoSystem
        if (typeof EcoSystem !== 'undefined') {
          EcoSystem.awardKonami(1000);
        }
        if (typeof Chat !== 'undefined') {
          Chat.addMessage('🎮 コナミコマンド発動！ 1000トークンを獲得！ 上限が10,000に拡大されました！', 'bot');
        }
      } else {
        if (typeof Chat !== 'undefined') {
          Chat.addMessage(data.message || 'コナミコマンドは既に使用済みです。', 'bot');
        }
        localStorage.setItem(CLAIMED_KEY, '1');
      }
    } catch (e) {
      console.error('Konami claim error:', e);
    }
  }

  return { init };
})();
