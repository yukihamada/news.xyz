/**
 * theme.js — Theme, dark mode, and font size control
 */
'use strict';

const Theme = (() => {
  const THEMES = ['hacker', 'card', 'lite', 'terminal', 'magazine', 'brutalist', 'pastel', 'neon'];
  const FONT_SIZES = [12, 14, 16, 18, 20, 24];
  const DENSITIES = ['compact', 'normal', 'spacious'];

  const ACCENT_PRESETS = {
    default: { light: '#0066cc', hover: '#004499', dark: '#4fc3f7', darkHover: '#81d4fa' },
    blue:    { light: '#2563eb', hover: '#1d4ed8', dark: '#60a5fa', darkHover: '#93bbfd' },
    green:   { light: '#16a34a', hover: '#15803d', dark: '#4ade80', darkHover: '#86efac' },
    purple:  { light: '#9333ea', hover: '#7e22ce', dark: '#c084fc', darkHover: '#d8b4fe' },
    red:     { light: '#dc2626', hover: '#b91c1c', dark: '#f87171', darkHover: '#fca5a5' },
    orange:  { light: '#ea580c', hover: '#c2410c', dark: '#fb923c', darkHover: '#fdba74' },
    pink:    { light: '#db2777', hover: '#be185d', dark: '#f472b6', darkHover: '#f9a8d4' },
  };

  function apply() {
    const theme = Storage.get('theme');
    const mode = Storage.get('mode');
    const fontSize = Storage.get('fontSize');
    const accentColor = Storage.get('accentColor');
    const density = Storage.get('density');

    document.body.setAttribute('data-theme', theme);
    document.body.setAttribute('data-mode', mode);
    document.body.setAttribute('data-density', density);
    document.body.setAttribute('data-show-images', String(Storage.get('showImages')));
    document.body.setAttribute('data-show-descriptions', String(Storage.get('showDescriptions')));
    document.body.setAttribute('data-animations', String(Storage.get('enableAnimations')));
    document.documentElement.style.setProperty('--font-size', fontSize + 'px');

    // Custom spacing via CSS custom properties
    document.documentElement.style.setProperty('--article-gap', Storage.get('articleGap') + 'px');
    document.documentElement.style.setProperty('--article-padding', Storage.get('articlePadding') + 'px');
    document.documentElement.style.setProperty('--radius', Storage.get('borderRadius') + 'px');

    // Element visibility via data attributes
    document.body.setAttribute('data-show-source', String(Storage.get('showSource')));
    document.body.setAttribute('data-show-time', String(Storage.get('showTime')));
    document.body.setAttribute('data-show-tts-btn', String(Storage.get('showTtsButton')));
    document.body.setAttribute('data-show-bookmark-btn', String(Storage.get('showBookmarkButton')));
    document.body.setAttribute('data-image-size', Storage.get('imageSize'));

    // Line height & description lines & content max width
    document.documentElement.style.setProperty('--line-height', String(Storage.get('lineHeight')));
    document.documentElement.style.setProperty('--desc-lines', String(Storage.get('descLines')));
    document.documentElement.style.setProperty('--content-max-width', Storage.get('contentMaxWidth') + 'px');

    // Apply accent color
    const preset = ACCENT_PRESETS[accentColor] || ACCENT_PRESETS.default;
    if (accentColor !== 'default') {
      const colors = mode === 'dark'
        ? { accent: preset.dark, hover: preset.darkHover }
        : { accent: preset.light, hover: preset.hover };
      document.documentElement.style.setProperty('--accent', colors.accent);
      document.documentElement.style.setProperty('--accent-hover', colors.hover);
    } else {
      document.documentElement.style.removeProperty('--accent');
      document.documentElement.style.removeProperty('--accent-hover');
    }
  }

  function setTheme(name) {
    if (!THEMES.includes(name)) return false;
    Storage.set('theme', name);
    apply();
    return true;
  }

  function setMode(mode) {
    if (mode !== 'dark' && mode !== 'light') return false;
    Storage.set('mode', mode);
    apply();
    return true;
  }

  function toggleMode() {
    const current = Storage.get('mode');
    return setMode(current === 'dark' ? 'light' : 'dark');
  }

  function setFontSize(size) {
    const n = parseInt(size, 10);
    if (n < 10 || n > 32) return false;
    Storage.set('fontSize', n);
    apply();
    return true;
  }

  function adjustFontSize(delta) {
    const current = Storage.get('fontSize');
    const next = Math.max(10, Math.min(32, current + delta));
    return setFontSize(next);
  }

  function setAccentColor(color) {
    if (!ACCENT_PRESETS[color]) return false;
    Storage.set('accentColor', color);
    apply();
    return true;
  }

  function setDensity(d) {
    if (!DENSITIES.includes(d)) return false;
    Storage.set('density', d);
    apply();
    return true;
  }

  function getState() {
    return {
      theme: Storage.get('theme'),
      mode: Storage.get('mode'),
      fontSize: Storage.get('fontSize'),
      accentColor: Storage.get('accentColor'),
      density: Storage.get('density'),
    };
  }

  function randomize() {
    const theme = THEMES[Math.floor(Math.random() * THEMES.length)];
    const mode = Math.random() > 0.5 ? 'dark' : 'light';
    const accentKeys = Object.keys(ACCENT_PRESETS);
    const accent = accentKeys[Math.floor(Math.random() * accentKeys.length)];
    Storage.set('theme', theme);
    Storage.set('mode', mode);
    Storage.set('accentColor', accent);
    apply();
    return { theme, mode, accent };
  }

  return { apply, setTheme, setMode, toggleMode, setFontSize, adjustFontSize, setAccentColor, setDensity, getState, randomize, THEMES, ACCENT_PRESETS, DENSITIES };
})();
