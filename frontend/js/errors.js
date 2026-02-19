'use strict';
const ErrorTracker = (() => {
  const MAX_ERRORS = 50;
  const STORAGE_KEY = 'hn_errors';
  let pending = [];

  function init() {
    window.addEventListener('error', (e) => {
      record({
        type: 'error',
        message: e.message,
        filename: e.filename,
        line: e.lineno,
        col: e.colno,
        ts: Date.now(),
      });
    });

    window.addEventListener('unhandledrejection', (e) => {
      record({
        type: 'promise',
        message: String(e.reason),
        ts: Date.now(),
      });
    });

    // Send pending errors on page hide
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'hidden') flush();
    });
  }

  function record(err) {
    try {
      const errors = JSON.parse(sessionStorage.getItem(STORAGE_KEY) || '[]');
      errors.push(err);
      if (errors.length > MAX_ERRORS) errors.splice(0, errors.length - MAX_ERRORS);
      sessionStorage.setItem(STORAGE_KEY, JSON.stringify(errors));
    } catch {}
    pending.push(err);
    if (location.hostname === 'localhost') {
      console.warn('[ErrorTracker]', err);
    }
  }

  function flush() {
    if (pending.length === 0) return;
    const payload = JSON.stringify({
      type: 'errors',
      url: location.pathname,
      errors: pending,
      ts: Date.now(),
    });
    try {
      navigator.sendBeacon('/api/telemetry', new Blob([payload], { type: 'application/json' }));
    } catch {}
    pending = [];
  }

  function getAll() {
    try {
      return JSON.parse(sessionStorage.getItem(STORAGE_KEY) || '[]');
    } catch { return []; }
  }

  function clear() {
    sessionStorage.removeItem(STORAGE_KEY);
    pending = [];
  }

  return { init, getAll, clear };
})();
