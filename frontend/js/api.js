/**
 * api.js — Fetch wrapper for news API
 */
'use strict';

const Api = (() => {
  // Base URL: empty for same-origin, set for remote API
  const BASE = '';

  // Request deduplication: prevent duplicate concurrent GET requests
  const inflight = new Map();
  function dedup(key, fn) {
    if (inflight.has(key)) return inflight.get(key);
    const p = fn().finally(() => inflight.delete(key));
    inflight.set(key, p);
    return p;
  }

  /** fetch with AbortController timeout (default 30s) */
  function fetchWithTimeout(url, opts = {}, timeoutMs = 30000) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    return fetch(url, { ...opts, signal: controller.signal }).finally(() => clearTimeout(timer));
  }

  function adminHeaders() {
    const secret = Storage.get('adminSecret') || '';
    return secret ? { 'X-Admin-Secret': secret } : {};
  }

  async function fetchArticles(category, limit = 30, cursor = null) {
    const params = new URLSearchParams();
    if (category) params.set('category', category);
    params.set('limit', String(limit));
    if (cursor) params.set('cursor', cursor);

    const url = `${BASE}/api/articles?${params}`;
    return dedup(url, async () => {
      const res = await fetch(url);
      if (!res.ok) throw new Error(`API error: ${res.status}`);
      return res.json();
    });
  }

  async function fetchCategories() {
    const url = `${BASE}/api/categories`;
    return dedup(url, async () => {
      const res = await fetch(url);
      if (!res.ok) throw new Error(`API error: ${res.status}`);
      return res.json();
    });
  }

  async function sendCommand(command) {
    const res = await fetch(`${BASE}/api/admin/command`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...adminHeaders() },
      body: JSON.stringify({ command }),
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function applyChange(changeId) {
    const res = await fetch(`${BASE}/api/admin/changes/${changeId}/apply`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...adminHeaders() },
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function toggleFeature(feature, enabled) {
    const res = await fetch(`${BASE}/api/admin/features`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...adminHeaders() },
      body: JSON.stringify({ feature, enabled }),
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function rejectChange(changeId) {
    const res = await fetch(`${BASE}/api/admin/changes/${changeId}/reject`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...adminHeaders() },
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function handleRateLimit(res, feature) {
    if (res.status === 402) {
      const data = await res.json().catch(() => ({}));
      if (typeof Subscription !== 'undefined') {
        Subscription.showUpgradePrompt(feature, data.limit || '?', data.tier || 'free');
      }
      throw new Error(data.message || 'rate_limit_exceeded');
    }
  }

  const SUMMARY_CACHE_TTL = 15 * 60 * 1000; // 15 minutes

  async function summarizeArticles(minutes) {
    // Check sessionStorage cache first
    const cacheKey = `hn_summary_${minutes}`;
    try {
      const raw = sessionStorage.getItem(cacheKey);
      if (raw) {
        const cached = JSON.parse(raw);
        if (cached.ts && Date.now() - cached.ts < SUMMARY_CACHE_TTL) {
          return cached.data;
        }
        sessionStorage.removeItem(cacheKey);
      }
    } catch { /* ignore */ }

    const auth = typeof Subscription !== 'undefined' ? Subscription.authHeaders() : {};
    const res = await fetchWithTimeout(`${BASE}/api/articles/summarize`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...auth },
      body: JSON.stringify({ minutes }),
    }, 30000);
    if (res.status === 402) { await handleRateLimit(res, 'AI要約'); return; }
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    const data = await res.json();

    // Save to sessionStorage cache
    try {
      sessionStorage.setItem(cacheKey, JSON.stringify({ data, ts: Date.now() }));
    } catch { /* quota */ }

    return data;
  }

  async function getArticleQuestions(title, description, source, url) {
    const auth = typeof Subscription !== 'undefined' ? Subscription.authHeaders() : {};
    const body = { title, description: description || '', source: source || '' };
    if (url) body.url = url;
    const customPrompt = typeof Storage !== 'undefined' ? Storage.get('aiQuestionPrompt') : '';
    if (customPrompt) body.custom_prompt = customPrompt;
    const res = await fetchWithTimeout(`${BASE}/api/articles/questions`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...auth },
      body: JSON.stringify(body),
    }, 30000);
    if (res.status === 402) { await handleRateLimit(res, 'AI質問'); return; }
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function askArticleQuestion(title, description, source, question, url) {
    const auth = typeof Subscription !== 'undefined' ? Subscription.authHeaders() : {};
    const body = { title, description: description || '', source: source || '', question };
    if (url) body.url = url;
    const customPrompt = typeof Storage !== 'undefined' ? Storage.get('aiAnswerPrompt') : '';
    if (customPrompt) body.custom_prompt = customPrompt;
    const res = await fetchWithTimeout(`${BASE}/api/articles/ask`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...auth },
      body: JSON.stringify(body),
    }, 30000);
    if (res.status === 402) { await handleRateLimit(res, 'AI回答'); return; }
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function manageCategory(action, id, labelJa, order) {
    const body = { action };
    if (id) body.id = id;
    if (labelJa) body.label_ja = labelJa;
    if (order) body.order = order;
    const res = await fetch(`${BASE}/api/admin/categories`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...adminHeaders() },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function toReading(text) {
    const auth = typeof Subscription !== 'undefined' ? Subscription.authHeaders() : {};
    const res = await fetch(`${BASE}/api/tts/to-reading`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...auth },
      body: JSON.stringify({ text }),
    });
    if (res.status === 402) { await handleRateLimit(res, '読み変換'); return; }
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function listFeeds() {
    const res = await fetch(`${BASE}/api/admin/feeds`, {
      headers: { ...adminHeaders() },
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function addFeed(url, source, category) {
    const res = await fetch(`${BASE}/api/admin/feeds`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...adminHeaders() },
      body: JSON.stringify({ url, source, category }),
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function deleteFeed(feedId) {
    const res = await fetch(`${BASE}/api/admin/feeds/${feedId}`, {
      method: 'DELETE',
      headers: { ...adminHeaders() },
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function toggleFeed(feedId, enabled) {
    const res = await fetch(`${BASE}/api/admin/feeds/${feedId}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json', ...adminHeaders() },
      body: JSON.stringify({ enabled }),
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function getArticleById(id) {
    const res = await fetch(`${BASE}/api/articles/${encodeURIComponent(id)}`);
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  async function searchArticles(query, limit = 20) {
    const params = new URLSearchParams();
    params.set('q', query);
    params.set('limit', String(limit));
    const res = await fetch(`${BASE}/api/search?${params}`);
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  }

  return { fetchArticles, fetchCategories, searchArticles, getArticleById, sendCommand, applyChange, rejectChange, toggleFeature, summarizeArticles, getArticleQuestions, askArticleQuestion, manageCategory, toReading, listFeeds, addFeed, deleteFeed, toggleFeed };
})();
