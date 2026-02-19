/**
 * google-auth.js — Google Identity Services integration
 */
'use strict';

const GoogleAuth = (() => {
  const AUTH_TOKEN_KEY = 'hn_google_auth_token';
  const USER_KEY = 'hn_google_user';
  let clientId = '';
  let gisLoaded = false;

  /** Fetch Google Client ID from backend config */
  async function fetchClientId() {
    try {
      const res = await fetch('/api/config');
      if (!res.ok) return '';
      const data = await res.json();
      return data.google_client_id || '';
    } catch {
      return '';
    }
  }

  /** Load Google Identity Services script */
  function loadGIS() {
    return new Promise((resolve) => {
      if (gisLoaded || typeof google !== 'undefined' && google.accounts) {
        gisLoaded = true;
        resolve();
        return;
      }
      const script = document.createElement('script');
      script.src = 'https://accounts.google.com/gsi/client';
      script.async = true;
      script.defer = true;
      script.onload = () => { gisLoaded = true; resolve(); };
      script.onerror = () => resolve(); // fail silently
      document.head.appendChild(script);
    });
  }

  /** Initialize GIS and render button if needed */
  async function init() {
    clientId = await fetchClientId();
    if (!clientId) return;
    await loadGIS();
    if (!gisLoaded || typeof google === 'undefined') return;

    google.accounts.id.initialize({
      client_id: clientId,
      callback: handleCredentialResponse,
      auto_select: false,
    });

    updateUI();
  }

  /** Render a Google Sign-In button into a container */
  function renderButton(container) {
    if (!gisLoaded || typeof google === 'undefined' || !clientId) return;
    google.accounts.id.renderButton(container, {
      type: 'standard',
      shape: 'pill',
      theme: 'outline',
      size: 'medium',
      text: 'signin_with',
      locale: 'ja',
    });
  }

  /** Show One Tap prompt */
  function showOneTap() {
    if (!gisLoaded || typeof google === 'undefined' || !clientId) return;
    if (isAuthenticated()) return;
    google.accounts.id.prompt();
  }

  /** Handle credential response from Google */
  async function handleCredentialResponse(response) {
    if (!response.credential) return;

    try {
      const deviceId = typeof Subscription !== 'undefined' ? Subscription.getDeviceId() : null;
      const res = await fetch('/api/auth/google', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          id_token: response.credential,
          device_id: deviceId,
        }),
      });

      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        console.error('Google auth failed:', err.error);
        if (typeof Chat !== 'undefined') {
          Chat.addMessage('Googleログインに失敗しました: ' + (err.error || ''), 'bot');
        }
        return;
      }

      const data = await res.json();
      localStorage.setItem(AUTH_TOKEN_KEY, data.auth_token);
      localStorage.setItem(USER_KEY, JSON.stringify(data.user));

      updateUI();

      if (typeof Chat !== 'undefined') {
        const name = data.user.name || data.user.email;
        Chat.addMessage(
          data.is_new
            ? `${name} さん、ようこそ！Googleログインで AI制限が2倍になりました。`
            : `${name} さん、おかえりなさい！`,
          'bot'
        );
      }

      // Re-render eco status
      if (typeof EcoSystem !== 'undefined') EcoSystem.renderEcoStatus();
    } catch (e) {
      console.error('Google auth error:', e);
    }
  }

  /** Sign out */
  function signOut() {
    localStorage.removeItem(AUTH_TOKEN_KEY);
    localStorage.removeItem(USER_KEY);
    if (gisLoaded && typeof google !== 'undefined') {
      google.accounts.id.disableAutoSelect();
    }
    updateUI();
    if (typeof EcoSystem !== 'undefined') EcoSystem.renderEcoStatus();
  }

  /** Check if user is authenticated via Google */
  function isAuthenticated() {
    return !!localStorage.getItem(AUTH_TOKEN_KEY);
  }

  /** Get stored auth token */
  function getAuthToken() {
    return localStorage.getItem(AUTH_TOKEN_KEY);
  }

  /** Get stored user info */
  function getUser() {
    try {
      return JSON.parse(localStorage.getItem(USER_KEY) || 'null');
    } catch {
      return null;
    }
  }

  /** Update UI: show user avatar next to logo, or login hint */
  function updateUI() {
    const logo = document.querySelector('.logo');
    if (!logo) return;

    // Remove existing google-user element
    const existing = logo.querySelector('.google-user');
    if (existing) existing.remove();

    const user = getUser();
    if (user && isAuthenticated()) {
      const el = document.createElement('span');
      el.className = 'google-user';
      el.title = user.email || '';
      if (user.picture) {
        el.innerHTML = `<img src="${user.picture}" alt="" class="google-avatar" referrerpolicy="no-referrer">`;
      } else {
        el.textContent = (user.name || user.email || '?').charAt(0).toUpperCase();
      }
      logo.appendChild(el);
    }
  }

  return {
    init,
    renderButton,
    showOneTap,
    signOut,
    isAuthenticated,
    getAuthToken,
    getUser,
    updateUI,
  };
})();
