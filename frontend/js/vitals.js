'use strict';
const Vitals = (() => {
  const metrics = {};

  function init() {
    if (typeof PerformanceObserver === 'undefined') return;

    // Track LCP (Largest Contentful Paint)
    try {
      new PerformanceObserver((list) => {
        const entries = list.getEntries();
        const last = entries[entries.length - 1];
        if (last) record('LCP', Math.round(last.startTime));
      }).observe({ type: 'largest-contentful-paint', buffered: true });
    } catch {}

    // Track INP (Interaction to Next Paint) — replaces deprecated FID
    try {
      let worstInp = 0;
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          // INP = max duration of event-timing entries
          if (entry.duration > worstInp) {
            worstInp = entry.duration;
            record('INP', Math.round(worstInp));
          }
        }
      }).observe({ type: 'event', buffered: true, durationThreshold: 16 });
    } catch {}

    // Track CLS (Cumulative Layout Shift)
    try {
      let cls = 0;
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          if (!entry.hadRecentInput) cls += entry.value;
        }
        record('CLS', Math.round(cls * 1000) / 1000);
      }).observe({ type: 'layout-shift', buffered: true });
    } catch {}

    // Track page load timing
    window.addEventListener('load', () => {
      setTimeout(() => {
        const nav = performance.getEntriesByType('navigation')[0];
        if (nav) {
          record('TTFB', Math.round(nav.responseStart));
          record('Load', Math.round(nav.loadEventEnd - nav.startTime));
        }
      }, 0);
    });

    // Send metrics on page hide (beacon)
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'hidden') report();
    });
  }

  function record(metric, value) {
    metrics[metric] = value;
    sessionStorage.setItem('vitals_' + metric, String(value));
    if (location.hostname === 'localhost') {
      console.log(`[Vitals] ${metric}: ${value}`);
    }
  }

  function getAll() {
    const result = {};
    for (const key of ['LCP', 'INP', 'CLS', 'TTFB', 'Load']) {
      const v = sessionStorage.getItem('vitals_' + key);
      if (v) result[key] = v;
    }
    return result;
  }

  function report() {
    if (Object.keys(metrics).length === 0) return;
    const payload = JSON.stringify({
      type: 'vitals',
      url: location.pathname,
      metrics,
      ts: Date.now(),
    });
    try {
      navigator.sendBeacon('/api/telemetry', new Blob([payload], { type: 'application/json' }));
    } catch {}
  }

  return { init, getAll };
})();
