/**
 * storage.js — localStorage abstraction with defaults
 */
'use strict';

const Storage = (() => {
  const PREFIX = 'hn_';
  const DEFAULTS = {
    theme: 'card',
    mode: 'light',
    fontSize: 16,
    category: '',
    ttsVoice: 'off',
    accentColor: 'default',
    density: 'normal',
    autoRefresh: 0,
    showImages: true,
    showDescriptions: true,
    hideReadArticles: false,
    enableAnimations: true,
    articlesPerPage: 30,
    readMarkDelay: 1500,
    articleClickAction: 'detail',
    infiniteScroll: true,
    typewriterSpeed: 20,
    ecoCacheRate: 20,
    aiQuestionPrompt: '',
    aiAnswerPrompt: '',
    articleGap: 8,
    articlePadding: 12,
    borderRadius: 12,
    showSource: true,
    showTime: true,
    showTtsButton: true,
    showBookmarkButton: true,
    imageSize: 'medium',
    lineHeight: 1.6,
    descLines: 2,
    contentMaxWidth: 1200,
    lang: '',
  };

  function get(key) {
    try {
      const val = localStorage.getItem(PREFIX + key);
      return val !== null ? JSON.parse(val) : DEFAULTS[key];
    } catch {
      return DEFAULTS[key];
    }
  }

  function set(key, value) {
    try {
      localStorage.setItem(PREFIX + key, JSON.stringify(value));
    } catch { /* quota exceeded — silently ignore */ }
  }

  function getAll() {
    const result = {};
    for (const key of Object.keys(DEFAULTS)) {
      result[key] = get(key);
    }
    return result;
  }

  return { get, set, getAll, DEFAULTS };
})();

/**
 * Bookmarks — Save bookmarked articles in localStorage
 */
const Bookmarks = (() => {
  const STORAGE_KEY = 'hn_bookmarks';
  let items = {};

  function init() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      items = raw ? JSON.parse(raw) : {};
    } catch {
      items = {};
    }
  }

  function save() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(items));
    } catch { /* quota exceeded */ }
  }

  function toggle(articleId, data) {
    if (!articleId) return false;
    if (items[articleId]) {
      delete items[articleId];
      save();
      return false;
    }
    items[articleId] = { ...data, ts: Date.now() };
    save();
    return true;
  }

  function isBookmarked(articleId) {
    return !!items[articleId];
  }

  function getAll() {
    return Object.entries(items)
      .map(([id, data]) => ({ id, ...data }))
      .sort((a, b) => b.ts - a.ts);
  }

  function getCount() {
    return Object.keys(items).length;
  }

  function clear() {
    items = {};
    try { localStorage.removeItem(STORAGE_KEY); } catch {}
  }

  return { init, toggle, isBookmarked, getAll, getCount, clear };
})();

/**
 * ReadHistory — Track read articles in localStorage
 * Stores article IDs with timestamps, auto-prunes entries older than 30 days.
 */
const ReadHistory = (() => {
  const STORAGE_KEY = 'hn_readHistory';
  const MAX_AGE_MS = 30 * 24 * 60 * 60 * 1000; // 30 days
  let cache = {};

  function init() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      cache = raw ? JSON.parse(raw) : {};
      prune();
    } catch {
      cache = {};
    }
  }

  function prune() {
    const cutoff = Date.now() - MAX_AGE_MS;
    let changed = false;
    for (const id in cache) {
      if (cache[id] < cutoff) {
        delete cache[id];
        changed = true;
      }
    }
    if (changed) save();
  }

  function save() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(cache));
    } catch { /* quota exceeded */ }
  }

  function markRead(articleId) {
    if (!articleId) return;
    cache[articleId] = Date.now();
    save();
  }

  function isRead(articleId) {
    return !!cache[articleId];
  }

  function getCount() {
    return Object.keys(cache).length;
  }

  function clear() {
    cache = {};
    try { localStorage.removeItem(STORAGE_KEY); } catch {}
  }

  return { init, markRead, isRead, getCount, clear };
})();

/**
 * CloneVoices — Manage cloned voice profiles in localStorage
 * Stores ref_audio (base64), ref_text, and name. Max 3 clones.
 */
const CloneVoices = (() => {
  const STORAGE_KEY = 'hn_cloneVoices';
  const MAX_CLONES = 3;
  let items = {};

  function init() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      items = raw ? JSON.parse(raw) : {};
    } catch {
      items = {};
    }
  }

  function save() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(items));
    } catch { /* quota exceeded */ }
  }

  function add(name, refAudio, refText) {
    if (Object.keys(items).length >= MAX_CLONES) return null;
    const id = 'cv_' + Date.now().toString(36);
    items[id] = { name, refAudio, refText, created: Date.now() };
    save();
    return id;
  }

  function remove(id) {
    delete items[id];
    save();
  }

  function get(id) {
    return items[id] || null;
  }

  function getAll() {
    return Object.entries(items)
      .map(([id, data]) => ({ id, ...data }))
      .sort((a, b) => b.created - a.created);
  }

  function getCount() {
    return Object.keys(items).length;
  }

  function clear() {
    items = {};
    try { localStorage.removeItem(STORAGE_KEY); } catch {}
  }

  return { init, add, remove, get, getAll, getCount, clear, MAX_CLONES };
})();
