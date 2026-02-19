/**
 * Service Worker — Cache strategies for HyperNews PWA
 * v16: Expanded cache, offline improvements, settings/about pages cached
 */

const CACHE_NAME = 'hypernews-v37';
const API_CACHE = 'hypernews-api-v2';
const IMG_CACHE = 'hypernews-img-v2';
const TTS_CACHE = 'hypernews-tts-v1';
const ARTICLE_CACHE = 'hypernews-articles-v1';
const API_MAX_ENTRIES = 1000; // 増量: より多くの記事をキャッシュ
const ARTICLE_MAX_ENTRIES = 200; // 最大200記事をオフライン保存
const IMG_MAX_AGE = 14 * 24 * 60 * 60 * 1000; // 14 days (延長)

const STATIC_ASSETS = [
  '/',
  '/index.html',
  '/offline.html',
  '/settings.html',
  '/about.html',
  '/pro.html',
  '/ab.html',
  '/dao.html',
  '/dao/airdrop.html',
  '/css/base.css?v=36',
  '/css/theme-hacker.css?v=36',
  '/css/theme-card.css?v=36',
  '/css/theme-lite.css?v=36',
  '/css/theme-terminal.css?v=36',
  '/css/theme-magazine.css?v=36',
  '/css/theme-brutalist.css?v=36',
  '/css/theme-pastel.css?v=36',
  '/css/theme-neon.css?v=36',
  '/css/chat.css?v=36',
  '/css/ads.css?v=36',
  '/css/settings.css?v=36',
  '/css/about.css?v=36',
  '/css/pro.css?v=36',
  '/css/assistant.css?v=36',
  '/css/feed-murmur.css?v=36',
  '/css/time-layers.css?v=37',
  '/js/errors.js?v=36',
  '/js/vitals.js?v=36',
  '/js/storage.js?v=36',
  '/js/i18n.js?v=36',
  '/js/site.js?v=36',
  '/js/ab.js?v=36',
  '/js/subscription.js?v=36',
  '/js/google-auth.js?v=36',
  '/js/konami.js?v=36',
  '/js/ads.js?v=36',
  '/js/api.js?v=36',
  '/js/theme.js?v=36',
  '/js/renderer.js?v=36',
  '/js/tts.js?v=36',
  '/js/commands.js?v=36',
  '/js/chat.js?v=36',
  '/js/app.js?v=36',
  '/js/sw-register.js?v=36',
  '/js/offline.js?v=37',
  '/js/settings.js?v=36',
  '/js/feed-murmur.js?v=36',
  '/manifest.json',
];

// Install: precache static assets
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(STATIC_ASSETS))
  );
  self.skipWaiting();
});

// Activate: clean old caches
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(
        keys
          .filter((k) => k !== CACHE_NAME && k !== API_CACHE && k !== IMG_CACHE && k !== TTS_CACHE && k !== ARTICLE_CACHE)
          .map((k) => caches.delete(k))
      )
    )
  );
  self.clients.claim();
});

// Fetch: strategy per resource type
self.addEventListener('fetch', (event) => {
  const { request } = event;
  const url = new URL(request.url);

  // POST requests: pass through (admin commands, AI)
  if (request.method !== 'GET') {
    return;
  }

  // API requests: stale-while-revalidate with dedicated cache
  if (url.pathname.startsWith('/api/')) {
    // 記事詳細APIは長期キャッシュ (オフライン読書用)
    if (url.pathname.startsWith('/api/articles/')) {
      event.respondWith(cacheFirstArticle(request));
    } else {
      event.respondWith(staleWhileRevalidateApi(request));
    }
    return;
  }

  // Images: cache-first with expiry
  if (
    request.destination === 'image' ||
    /\.(png|jpg|jpeg|gif|webp|svg|ico)$/i.test(url.pathname)
  ) {
    event.respondWith(cacheFirstImage(request));
    return;
  }

  // Static assets: cache-first + background update
  event.respondWith(cacheFirstWithUpdate(request));
});

// Message handler: prefetch categories and save articles
self.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'PREFETCH_CATEGORIES') {
    const categories = event.data.categories || [];
    prefetchCategories(categories);
  } else if (event.data && event.data.type === 'SAVE_ARTICLE') {
    // 記事を明示的にオフライン保存
    saveArticleForOffline(event.data.articleId);
  } else if (event.data && event.data.type === 'PREFETCH_FEED') {
    // フィード全体をプリフェッチ (オフライン準備)
    prefetchFeed(event.data.limit || 50);
  }
});

async function prefetchCategories(categories) {
  const cache = await caches.open(API_CACHE);
  for (const cat of categories) {
    try {
      const params = new URLSearchParams({ limit: '30' });
      if (cat) params.set('category', cat);
      const url = `/api/articles?${params}`;
      const response = await fetch(url);
      if (response.ok) {
        await cache.put(new Request(url), response);
      }
    } catch {
      // Network error — skip
    }
  }
  // Also prefetch categories list
  try {
    const catRes = await fetch('/api/categories');
    if (catRes.ok) {
      await cache.put(new Request('/api/categories'), catRes);
    }
  } catch { /* skip */ }

  // Trim API cache
  await trimCache(API_CACHE, API_MAX_ENTRIES);
}

async function cacheFirstWithUpdate(request) {
  const cache = await caches.open(CACHE_NAME);
  const cached = await cache.match(request);

  // Background update
  const fetchPromise = fetch(request)
    .then((response) => {
      if (response.ok) {
        cache.put(request, response.clone());
      }
      return response;
    })
    .catch(() => null);

  if (cached) return cached;

  const response = await fetchPromise;
  if (response) return response;

  // Offline fallback for navigation
  if (request.mode === 'navigate') {
    const offline = await cache.match('/offline.html');
    if (offline) return offline;
  }

  return new Response('Offline', { status: 503 });
}

async function staleWhileRevalidateApi(request) {
  const cache = await caches.open(API_CACHE);
  const cached = await cache.match(request);

  const fetchPromise = fetch(request)
    .then(async (response) => {
      if (response.ok) {
        await cache.put(request, response.clone());
        // Trim after adding
        trimCache(API_CACHE, API_MAX_ENTRIES);
      }
      return response;
    })
    .catch(() => cached);

  return cached || fetchPromise;
}

async function cacheFirstImage(request) {
  const cache = await caches.open(IMG_CACHE);
  const cached = await cache.match(request);
  if (cached) return cached;

  try {
    const response = await fetch(request);
    if (response.ok) {
      cache.put(request, response.clone());
    }
    return response;
  } catch {
    return new Response('', { status: 404 });
  }
}

/** Trim a cache to maxEntries by removing oldest entries */
async function trimCache(cacheName, maxEntries) {
  const cache = await caches.open(cacheName);
  const keys = await cache.keys();
  if (keys.length <= maxEntries) return;
  // Remove oldest entries (first in list = oldest)
  const toDelete = keys.slice(0, keys.length - maxEntries);
  await Promise.all(toDelete.map((k) => cache.delete(k)));
}

/** Cache-first strategy for article details (offline reading) */
async function cacheFirstArticle(request) {
  const cache = await caches.open(ARTICLE_CACHE);
  const cached = await cache.match(request);

  if (cached) {
    // Background revalidate (but return cached immediately)
    fetch(request)
      .then((response) => {
        if (response.ok) {
          cache.put(request, response.clone());
        }
      })
      .catch(() => {});
    return cached;
  }

  try {
    const response = await fetch(request);
    if (response.ok) {
      await cache.put(request, response.clone());
      await trimCache(ARTICLE_CACHE, ARTICLE_MAX_ENTRIES);
    }
    return response;
  } catch {
    // Offline and no cache
    return new Response(
      JSON.stringify({ error: 'Offline', offline: true }),
      { status: 503, headers: { 'Content-Type': 'application/json' } }
    );
  }
}

/** Save a specific article for offline reading */
async function saveArticleForOffline(articleId) {
  if (!articleId) return;

  const cache = await caches.open(ARTICLE_CACHE);
  const url = `/api/articles/${articleId}`;

  try {
    const response = await fetch(url);
    if (response.ok) {
      await cache.put(new Request(url), response);
      await trimCache(ARTICLE_CACHE, ARTICLE_MAX_ENTRIES);
    }
  } catch {
    // Network error - skip
  }
}

/** Prefetch feed for offline reading (first N articles) */
async function prefetchFeed(limit = 50) {
  try {
    const feedUrl = `/api/feed?limit=${limit}`;
    const response = await fetch(feedUrl);

    if (!response.ok) return;

    const articles = await response.json();
    const cache = await caches.open(ARTICLE_CACHE);

    // Cache each article's full content
    const prefetchPromises = articles.slice(0, limit).map(async (article) => {
      if (!article.id) return;

      try {
        const articleUrl = `/api/articles/${article.id}`;
        const articleResponse = await fetch(articleUrl);

        if (articleResponse.ok) {
          await cache.put(new Request(articleUrl), articleResponse);
        }
      } catch {
        // Skip failed articles
      }
    });

    await Promise.allSettled(prefetchPromises);
    await trimCache(ARTICLE_CACHE, ARTICLE_MAX_ENTRIES);

    // Also cache the feed list itself
    const apiCache = await caches.open(API_CACHE);
    const feedResponse = await fetch(feedUrl);
    if (feedResponse.ok) {
      await apiCache.put(new Request(feedUrl), feedResponse);
    }
  } catch {
    // Network error - skip
  }
}
