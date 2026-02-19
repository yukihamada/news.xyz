/**
 * ab.js — A/B Test Engine with Auto-Optimization
 * 10 design variants. Thompson Sampling (Beta distribution) for automatic traffic allocation.
 * Tracks impressions, article clicks, detail opens, scroll depth, session duration.
 *
 * Auto-optimization:
 *   - Exploration phase: first MIN_SAMPLES impressions per variant → uniform random
 *   - Exploitation phase: Thompson Sampling draws from Beta(clicks+1, impressions-clicks+1)
 *     → higher-performing variants automatically get more traffic
 *   - Convergence: when a variant has >95% win probability with 100+ impressions,
 *     it's declared the winner and gets 100% traffic
 */
'use strict';

const ABTest = (() => {
  const STORAGE_KEY = 'hn_ab';
  const SESSION_KEY = 'hn_ab_session';
  const MIN_SAMPLES = 10; // minimum impressions before optimization kicks in

  /** 10 design variants — each overrides CSS variables */
  const VARIANTS = [
    {
      id: 'blue-pro',
      name: 'Blue Professional',
      desc: 'クール＆プロフェッショナル（デフォルト）',
      light: { bg:'#fafafa', text:'#1a1a1a', surface:'#fff', border:'#e0e0e0', accent:'#0066cc', accentHover:'#004499', muted:'#595959' },
      dark:  { bg:'#1a1a2e', text:'#e0e0e0', surface:'#16213e', border:'#2a2a4a', accent:'#4fc3f7', accentHover:'#81d4fa', muted:'#a0a0a0' },
    },
    {
      id: 'warm-amber',
      name: 'Warm Amber',
      desc: 'ウォーム＆知的（ClaudNews系）',
      light: { bg:'#fdf8f6', text:'#1c1917', surface:'#fff', border:'#e7e5e4', accent:'#c2410c', accentHover:'#9a3412', muted:'#78716c' },
      dark:  { bg:'#1c1917', text:'#e7e5e4', surface:'#292524', border:'#44403c', accent:'#fb923c', accentHover:'#fdba74', muted:'#a8a29e' },
    },
    {
      id: 'purple-cosmos',
      name: 'Purple Cosmos',
      desc: '宇宙的パープル＆ミステリアス',
      light: { bg:'#faf5ff', text:'#1e1b2e', surface:'#fff', border:'#e4d5f7', accent:'#7c3aed', accentHover:'#5b21b6', muted:'#6b6180' },
      dark:  { bg:'#13111c', text:'#e2dff0', surface:'#1e1a2e', border:'#372e54', accent:'#a78bfa', accentHover:'#c4b5fd', muted:'#8b83a8' },
    },
    {
      id: 'forest-green',
      name: 'Forest Green',
      desc: '自然派グリーン＆リラックス',
      light: { bg:'#f0fdf4', text:'#14532d', surface:'#fff', border:'#bbf7d0', accent:'#16a34a', accentHover:'#15803d', muted:'#4d7c5e' },
      dark:  { bg:'#0a1f12', text:'#d1fae5', surface:'#14291e', border:'#1e3a28', accent:'#4ade80', accentHover:'#86efac', muted:'#6ea882' },
    },
    {
      id: 'crimson-bold',
      name: 'Crimson Bold',
      desc: '大胆なレッド＆インパクト',
      light: { bg:'#fff5f5', text:'#1a1a1a', surface:'#fff', border:'#fecaca', accent:'#dc2626', accentHover:'#b91c1c', muted:'#78585e' },
      dark:  { bg:'#1a0a0a', text:'#fecaca', surface:'#291414', border:'#442020', accent:'#f87171', accentHover:'#fca5a5', muted:'#a87878' },
    },
    {
      id: 'ocean-teal',
      name: 'Ocean Teal',
      desc: '海のティール＆爽やか',
      light: { bg:'#f0fdfa', text:'#134e4a', surface:'#fff', border:'#99f6e4', accent:'#0d9488', accentHover:'#0f766e', muted:'#4d8078' },
      dark:  { bg:'#0a1a18', text:'#ccfbf1', surface:'#142422', border:'#1e3836', accent:'#2dd4bf', accentHover:'#5eead4', muted:'#6ea8a0' },
    },
    {
      id: 'midnight-gold',
      name: 'Midnight Gold',
      desc: '高級感のゴールド＆ダーク',
      light: { bg:'#fefce8', text:'#1a1800', surface:'#fff', border:'#fef08a', accent:'#ca8a04', accentHover:'#a16207', muted:'#78716c' },
      dark:  { bg:'#141210', text:'#fef3c7', surface:'#1c1a16', border:'#3d3520', accent:'#fbbf24', accentHover:'#fcd34d', muted:'#a89e7c' },
    },
    {
      id: 'sakura-pink',
      name: 'Sakura Pink',
      desc: '桜ピンク＆やわらか',
      light: { bg:'#fdf2f8', text:'#1a1a1a', surface:'#fff', border:'#fbcfe8', accent:'#db2777', accentHover:'#be185d', muted:'#9d7088' },
      dark:  { bg:'#1a0e14', text:'#fce7f3', surface:'#261520', border:'#44233a', accent:'#f472b6', accentHover:'#f9a8d4', muted:'#a87898' },
    },
    {
      id: 'slate-minimal',
      name: 'Slate Minimal',
      desc: 'モノクロ＆ミニマル',
      light: { bg:'#f8fafc', text:'#0f172a', surface:'#fff', border:'#e2e8f0', accent:'#475569', accentHover:'#334155', muted:'#94a3b8' },
      dark:  { bg:'#0f172a', text:'#e2e8f0', surface:'#1e293b', border:'#334155', accent:'#94a3b8', accentHover:'#cbd5e1', muted:'#64748b' },
    },
    {
      id: 'sunset-gradient',
      name: 'Sunset Gradient',
      desc: 'サンセットオレンジ＆ローズ',
      light: { bg:'#fff7ed', text:'#1a1a1a', surface:'#fff', border:'#fed7aa', accent:'#ea580c', accentHover:'#c2410c', muted:'#78716c' },
      dark:  { bg:'#1a110a', text:'#ffedd5', surface:'#261a10', border:'#44301a', accent:'#fb923c', accentHover:'#fdba74', muted:'#a89078' },
    },
  ];

  let state = {
    enabled: ['blue-pro', 'warm-amber'],
    autoOptimize: true,   // auto-optimization on by default
    winner: null,          // set when a clear winner is found
    sessions: {},
  };

  let currentVariant = null;
  let sessionStart = 0;
  let maxScrollDepth = 0;

  function init() {
    load();
    assign();
    applyVariant();
    trackSession();
  }

  function load() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const saved = JSON.parse(raw);
        state = { ...state, ...saved };
      }
    } catch { /* ignore */ }
    for (const v of VARIANTS) {
      if (!state.sessions[v.id]) {
        state.sessions[v.id] = { impressions: 0, clicks: 0, detailOpens: 0, totalScrollDepth: 0, totalDuration: 0, sessionCount: 0 };
      }
    }
  }

  function save() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch { /* quota */ }
  }

  // ============================
  // Thompson Sampling (Beta)
  // ============================

  /** Sample from Beta(alpha, beta) using Joehnk's method */
  function sampleBeta(alpha, beta) {
    // For small alpha/beta, use inverse CDF approximation
    const a = joehnkGamma(alpha);
    const b = joehnkGamma(beta);
    return a / (a + b);
  }

  /** Sample from Gamma(shape, 1) — Marsaglia & Tsang for shape>=1, shift for shape<1 */
  function joehnkGamma(shape) {
    if (shape < 1) {
      return joehnkGamma(shape + 1) * Math.pow(Math.random(), 1 / shape);
    }
    const d = shape - 1/3;
    const c = 1 / Math.sqrt(9 * d);
    while (true) {
      let x, v;
      do {
        x = randn();
        v = 1 + c * x;
      } while (v <= 0);
      v = v * v * v;
      const u = Math.random();
      if (u < 1 - 0.0331 * (x * x) * (x * x)) return d * v;
      if (Math.log(u) < 0.5 * x * x + d * (1 - v + Math.log(v))) return d * v;
    }
  }

  /** Standard normal via Box-Muller */
  function randn() {
    const u1 = Math.random();
    const u2 = Math.random();
    return Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
  }

  /** Compute engagement score for a variant (composite metric) */
  function engagementScore(s) {
    if (!s || s.impressions === 0) return 0;
    const ctr = s.clicks / s.impressions;
    const detailRate = s.detailOpens / s.impressions;
    const avgScroll = s.sessionCount > 0 ? (s.totalScrollDepth / s.sessionCount) / 100 : 0;
    const avgDur = s.sessionCount > 0 ? Math.min(s.totalDuration / s.sessionCount, 300) / 300 : 0;
    // Weighted composite: CTR 40%, detail 30%, scroll 15%, duration 15%
    return ctr * 0.4 + detailRate * 0.3 + avgScroll * 0.15 + avgDur * 0.15;
  }

  /** Get win probability for each enabled variant via Monte Carlo (1000 simulations) */
  function getWinProbabilities() {
    const pool = getPool();
    const wins = {};
    for (const v of pool) wins[v.id] = 0;
    const N = 1000;
    for (let i = 0; i < N; i++) {
      let bestId = null;
      let bestSample = -1;
      for (const v of pool) {
        const s = state.sessions[v.id];
        // Use engagement "successes" = clicks + detailOpens (capped at impressions)
        const successes = Math.min((s.clicks || 0) + (s.detailOpens || 0), s.impressions || 0);
        const failures = Math.max((s.impressions || 0) - successes, 0);
        const sample = sampleBeta(successes + 1, failures + 1);
        if (sample > bestSample) {
          bestSample = sample;
          bestId = v.id;
        }
      }
      if (bestId) wins[bestId]++;
    }
    const result = {};
    for (const id in wins) result[id] = wins[id] / N;
    return result;
  }

  /** Check if we have a winner (>95% win probability with 100+ impressions) */
  function checkForWinner() {
    const pool = getPool();
    if (pool.length < 2) return;
    const probs = getWinProbabilities();
    for (const id in probs) {
      const s = state.sessions[id];
      if (probs[id] >= 0.95 && s.impressions >= 100) {
        state.winner = id;
        save();
        return;
      }
    }
  }

  function getPool() {
    return state.enabled.length > 0
      ? VARIANTS.filter(v => state.enabled.includes(v.id))
      : VARIANTS;
  }

  /** Pick variant — Thompson Sampling when auto-optimize is on */
  function assign() {
    const existing = sessionStorage.getItem(SESSION_KEY);
    if (existing && VARIANTS.find(v => v.id === existing)) {
      currentVariant = existing;
      return;
    }

    const pool = getPool();

    // If there's a declared winner, use it
    if (state.winner && pool.find(v => v.id === state.winner)) {
      currentVariant = state.winner;
    }
    // Auto-optimize: Thompson Sampling
    else if (state.autoOptimize && pool.every(v => (state.sessions[v.id]?.impressions || 0) >= MIN_SAMPLES)) {
      let bestId = null;
      let bestSample = -1;
      for (const v of pool) {
        const s = state.sessions[v.id];
        const successes = Math.min((s.clicks || 0) + (s.detailOpens || 0), s.impressions || 0);
        const failures = Math.max((s.impressions || 0) - successes, 0);
        const sample = sampleBeta(successes + 1, failures + 1);
        if (sample > bestSample) {
          bestSample = sample;
          bestId = v.id;
        }
      }
      currentVariant = bestId || pool[0].id;
    }
    // Exploration phase or auto-optimize off: uniform random
    else {
      currentVariant = pool[Math.floor(Math.random() * pool.length)].id;
    }

    sessionStorage.setItem(SESSION_KEY, currentVariant);
    state.sessions[currentVariant].impressions++;
    state.sessions[currentVariant].sessionCount++;
    save();

    // Periodically check for winner
    const totalImp = Object.values(state.sessions).reduce((a, s) => a + (s.impressions || 0), 0);
    if (state.autoOptimize && !state.winner && totalImp % 20 === 0 && totalImp >= 100) {
      checkForWinner();
    }
  }

  /** Apply CSS variables for the chosen variant */
  function applyVariant() {
    const v = VARIANTS.find(x => x.id === currentVariant);
    if (!v) return;
    const mode = document.body.dataset.mode || 'light';
    applyColors(v, mode);
    const observer = new MutationObserver(() => {
      const m = document.body.dataset.mode || 'light';
      applyColors(v, m);
    });
    observer.observe(document.body, { attributes: true, attributeFilter: ['data-mode'] });
  }

  function applyColors(variant, mode) {
    const colors = mode === 'dark' ? variant.dark : variant.light;
    const root = document.documentElement;
    root.style.setProperty('--bg', colors.bg);
    root.style.setProperty('--text', colors.text);
    root.style.setProperty('--surface', colors.surface);
    root.style.setProperty('--border', colors.border);
    root.style.setProperty('--accent', colors.accent);
    root.style.setProperty('--accent-hover', colors.accentHover);
    root.style.setProperty('--muted', colors.muted);
  }

  function preview(variantId) {
    const v = VARIANTS.find(x => x.id === variantId);
    if (!v) return;
    currentVariant = variantId;
    sessionStorage.setItem(SESSION_KEY, variantId);
    const mode = document.body.dataset.mode || 'light';
    applyColors(v, mode);
  }

  /** Track session metrics */
  function trackSession() {
    sessionStart = Date.now();
    maxScrollDepth = 0;

    document.addEventListener('click', (e) => {
      if (e.target.closest('.article-title a')) {
        if (currentVariant && state.sessions[currentVariant]) {
          state.sessions[currentVariant].clicks++;
          save();
        }
      }
    });

    const detailPanel = document.getElementById('detail-panel');
    if (detailPanel) {
      const obs = new MutationObserver(() => {
        if (!detailPanel.hidden && detailPanel.classList.contains('open')) {
          if (currentVariant && state.sessions[currentVariant]) {
            state.sessions[currentVariant].detailOpens++;
            save();
          }
        }
      });
      obs.observe(detailPanel, { attributes: true, attributeFilter: ['class'] });
    }

    window.addEventListener('scroll', () => {
      const depth = Math.round((window.scrollY + window.innerHeight) / document.documentElement.scrollHeight * 100);
      if (depth > maxScrollDepth) maxScrollDepth = depth;
    }, { passive: true });

    window.addEventListener('beforeunload', () => {
      if (currentVariant && state.sessions[currentVariant]) {
        const duration = Math.round((Date.now() - sessionStart) / 1000);
        state.sessions[currentVariant].totalDuration += duration;
        state.sessions[currentVariant].totalScrollDepth += maxScrollDepth;
        save();
      }
    });
  }

  // --- Public API ---
  function getVariants() { return VARIANTS; }
  function getEnabled() { return [...state.enabled]; }
  function getCurrent() { return currentVariant; }
  function getStats() { return JSON.parse(JSON.stringify(state.sessions)); }
  function isAutoOptimize() { return state.autoOptimize; }
  function getWinner() { return state.winner; }

  function setAutoOptimize(on) {
    state.autoOptimize = !!on;
    if (!on) state.winner = null;
    save();
  }

  function clearWinner() {
    state.winner = null;
    save();
  }

  function setEnabled(ids) {
    state.enabled = ids.filter(id => VARIANTS.some(v => v.id === id));
    state.winner = null; // reset winner when pool changes
    save();
  }

  function enableVariant(id) {
    if (!state.enabled.includes(id)) {
      state.enabled.push(id);
      state.winner = null;
      save();
    }
  }

  function disableVariant(id) {
    state.enabled = state.enabled.filter(x => x !== id);
    if (state.winner === id) state.winner = null;
    save();
  }

  function resetStats() {
    for (const v of VARIANTS) {
      state.sessions[v.id] = { impressions: 0, clicks: 0, detailOpens: 0, totalScrollDepth: 0, totalDuration: 0, sessionCount: 0 };
    }
    state.winner = null;
    save();
  }

  function exportCSV() {
    const probs = getWinProbabilities();
    const header = 'variant_id,name,enabled,impressions,clicks,detail_opens,ctr,avg_scroll_depth,avg_duration_sec,win_probability,is_winner';
    const rows = VARIANTS.map(v => {
      const s = state.sessions[v.id] || {};
      const imp = s.impressions || 0;
      const ctr = imp > 0 ? ((s.clicks || 0) / imp * 100).toFixed(2) : '0.00';
      const avgScroll = s.sessionCount > 0 ? Math.round((s.totalScrollDepth || 0) / s.sessionCount) : 0;
      const avgDur = s.sessionCount > 0 ? Math.round((s.totalDuration || 0) / s.sessionCount) : 0;
      const wp = ((probs[v.id] || 0) * 100).toFixed(1);
      return `${v.id},${v.name},${state.enabled.includes(v.id)},${imp},${s.clicks||0},${s.detailOpens||0},${ctr}%,${avgScroll}%,${avgDur},${wp}%,${state.winner===v.id}`;
    });
    return header + '\n' + rows.join('\n');
  }

  return {
    init, getVariants, getEnabled, getCurrent, getStats,
    setEnabled, enableVariant, disableVariant, resetStats,
    preview, exportCSV, applyColors,
    isAutoOptimize, setAutoOptimize, getWinner, clearWinner,
    getWinProbabilities,
  };
})();
