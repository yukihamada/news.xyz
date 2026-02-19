/**
 * site.js — Site branding for news.xyz
 */
'use strict';

const Site = (() => {
  const siteId = 'xyz';

  const c = {
    name: 'news.xyz',
    title: 'news.xyz — AI News, Blazing Fast | Built in Rust',
    description: 'The $56,000 domain running the fastest AI news aggregator. 146+ feeds, AI summaries, voice news, 8 themes. Rust-powered. Ad-free.',
    url: 'https://news.xyz/',
    image: 'https://news.xyz/icons/og-xyz.png',
    manifest: 'manifest.json',
    themeColor: '#1a1a2e',
  };

  function apply() {
    // Logo
    const logo = document.querySelector('.logo');
    if (logo) {
      logo.textContent = c.name;
    }

    // Title
    document.title = c.title;

    // Theme color
    const tc = document.querySelector('meta[name="theme-color"]');
    if (tc) tc.setAttribute('content', c.themeColor);

    // Canonical
    const canon = document.querySelector('link[rel="canonical"]');
    if (canon) canon.setAttribute('href', c.url);

    // Manifest
    const mf = document.querySelector('link[rel="manifest"]');
    if (mf) mf.setAttribute('href', c.manifest);

    // OGP
    setMeta('og:title', c.title);
    setMeta('og:url', c.url);
    setMeta('og:site_name', c.name);
    setMeta('og:image', c.image);
    setMeta('og:description', c.description);

    // Twitter
    setMeta('twitter:title', c.title);
    setMeta('twitter:description', c.description);
    setMeta('twitter:image', c.image);
  }

  function setMeta(nameOrProp, content) {
    const el = document.querySelector(`meta[name="${nameOrProp}"]`)
            || document.querySelector(`meta[property="${nameOrProp}"]`);
    if (el) el.setAttribute('content', content);
  }

  // Apply immediately on parse
  document.addEventListener('DOMContentLoaded', apply);

  return {
    id: siteId,
    name: c.name,
    title: c.title,
    url: c.url,
    themeColor: c.themeColor,
  };
})();
