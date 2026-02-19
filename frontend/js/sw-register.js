/**
 * sw-register.js — Service Worker registration
 */
'use strict';

if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker
      .register('/sw.js', { scope: '/' })
      .catch(() => { /* SW registration failed — app works fine without it */ });
  });
}
