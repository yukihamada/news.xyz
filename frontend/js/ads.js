/**
 * ads.js — Ad manager module
 * Shows Google AdSense ads for Free users, upgrade CTA as fallback.
 * Pro users see no ads at all.
 */
'use strict';

const Ads = (function() {
  // Google AdSense publisher ID — empty = not yet approved, show self-promo CTA instead
  const PUB_ID = '';
  const FEED_AD_INTERVAL = 5;  // Show ad every N articles
  let adCounter = 0;
  let initialized = false;

  function init() {
    if (typeof Subscription !== 'undefined' && Subscription.isPro()) return;
    if (PUB_ID) loadAdSenseScript();
    initialized = true;
  }

  function loadAdSenseScript() {
    if (document.querySelector('script[src*="adsbygoogle"]')) return;
    const s = document.createElement('script');
    s.async = true;
    s.crossOrigin = 'anonymous';
    s.src = 'https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=ca-' + PUB_ID;
    document.head.appendChild(s);
  }

  /**
   * Call after each article is appended to the feed.
   * Returns an ad element every FEED_AD_INTERVAL articles, or null.
   */
  function maybeFeedAd() {
    if (typeof Subscription !== 'undefined' && Subscription.isPro()) return null;
    adCounter++;
    if (adCounter % FEED_AD_INTERVAL !== 0) return null;
    return PUB_ID ? createAdSenseUnit() : createUpgradeCTA();
  }

  function createAdSenseUnit() {
    const wrap = document.createElement('div');
    wrap.className = 'feed-ad-unit';
    wrap.innerHTML =
      '<span class="feed-ad-label">AD</span>' +
      '<ins class="adsbygoogle" style="display:block" ' +
      'data-ad-format="fluid" data-ad-layout-key="-gu-18+5g-2f-83" ' +
      'data-ad-client="ca-' + PUB_ID + '" data-ad-slot=""></ins>';
    // Push ad
    try {
      (window.adsbygoogle = window.adsbygoogle || []).push({});
    } catch (e) { /* AdSense not loaded yet */ }
    return wrap;
  }

  function createUpgradeCTA() {
    const wrap = document.createElement('div');
    wrap.className = 'feed-ad-cta';
    wrap.innerHTML =
      '<div class="feed-ad-cta__icon">\u26A1</div>' +
      '<div class="feed-ad-cta__text">\u5E83\u544A\u306A\u3057\u3067\u5FEB\u9069\u306B</div>' +
      '<div class="feed-ad-cta__sub">Pro\u30D7\u30E9\u30F3 \u00A5500/\u6708 \u2014 \u7121\u5236\u9650AI + \u5E83\u544A\u975E\u8868\u793A</div>' +
      '<a href="/pro.html" class="feed-ad-cta__btn">Pro\u306B\u30A2\u30C3\u30D7\u30B0\u30EC\u30FC\u30C9</a>';
    return wrap;
  }

  /**
   * Show a banner ad in a container (for standard news.xyz site).
   * @param {HTMLElement} container - element to append the banner to
   */
  function showBannerAd(container) {
    if (!container) return;
    if (typeof Subscription !== 'undefined' && Subscription.isPro()) return;

    if (PUB_ID) {
      const wrap = document.createElement('div');
      wrap.className = 'banner-ad';
      wrap.innerHTML =
        '<ins class="adsbygoogle" style="display:block" ' +
        'data-ad-client="ca-' + PUB_ID + '" data-ad-slot="" ' +
        'data-ad-format="auto" data-full-width-responsive="true"></ins>';
      container.appendChild(wrap);
      try {
        (window.adsbygoogle = window.adsbygoogle || []).push({});
      } catch (e) { /* */ }
    } else {
      const wrap = document.createElement('div');
      wrap.className = 'banner-ad-cta';
      wrap.innerHTML =
        '<span class="banner-ad-cta__text">\u2728 Pro\u30D7\u30E9\u30F3\u3067\u5E83\u544A\u975E\u8868\u793A + \u7121\u5236\u9650AI</span>' +
        '<a href="/pro.html" class="banner-ad-cta__btn">Pro\u3078</a>';
      container.appendChild(wrap);
    }
  }

  /** Reset counter (e.g. on category switch) */
  function resetCounter() {
    adCounter = 0;
  }

  return { init, maybeFeedAd, showBannerAd, resetCounter };
})();
