/**
 * Offline Support — Prefetch articles and show connectivity status
 */

// Connection status tracking
let isOnline = navigator.onLine;
let offlineIndicator = null;

// Initialize offline features
function initOffline() {
  createOfflineIndicator();
  updateOnlineStatus();

  // Listen for online/offline events
  window.addEventListener('online', handleOnline);
  window.addEventListener('offline', handleOffline);

  // Prefetch articles for offline reading when online
  if (isOnline && 'serviceWorker' in navigator && navigator.serviceWorker.controller) {
    // Wait a bit before prefetching (don't block initial load)
    setTimeout(() => {
      prefetchArticlesForOffline();
    }, 3000);
  }
}

// Create offline indicator element
function createOfflineIndicator() {
  if (offlineIndicator) return;

  offlineIndicator = document.createElement('div');
  offlineIndicator.id = 'offline-indicator';
  offlineIndicator.innerHTML = `
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <line x1="1" y1="1" x2="23" y2="23"/>
      <path d="M16.72 11.06A10.94 10.94 0 0 1 19 12.55"/>
      <path d="M5 12.55a10.94 10.94 0 0 1 5.17-2.39"/>
      <path d="M10.71 5.05A16 16 0 0 1 22.58 9"/>
      <path d="M1.42 9a15.91 15.91 0 0 1 4.7-2.88"/>
      <path d="M8.53 16.11a6 6 0 0 1 6.95 0"/>
      <line x1="12" y1="20" x2="12.01" y2="20"/>
    </svg>
    <span>Offline Mode</span>
  `;

  // Add styles
  const style = document.createElement('style');
  style.textContent = `
    #offline-indicator {
      position: fixed;
      top: 70px;
      right: 20px;
      background: rgba(239, 68, 68, 0.95);
      color: white;
      padding: 0.75rem 1.25rem;
      border-radius: 8px;
      display: none;
      align-items: center;
      gap: 0.5rem;
      font-size: 0.875rem;
      font-weight: 600;
      z-index: 9999;
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
      backdrop-filter: blur(10px);
      animation: slideInFromRight 0.3s ease-out;
    }

    #offline-indicator.show {
      display: flex;
    }

    #offline-indicator.online {
      background: rgba(16, 185, 129, 0.95);
    }

    @keyframes slideInFromRight {
      from {
        transform: translateX(100%);
        opacity: 0;
      }
      to {
        transform: translateX(0);
        opacity: 1;
      }
    }

    @media (max-width: 768px) {
      #offline-indicator {
        top: 60px;
        right: 10px;
        left: 10px;
        justify-content: center;
      }
    }
  `;

  document.head.appendChild(style);
  document.body.appendChild(offlineIndicator);
}

// Update online status indicator
function updateOnlineStatus() {
  if (!offlineIndicator) return;

  if (!isOnline) {
    offlineIndicator.classList.add('show');
    offlineIndicator.classList.remove('online');
    offlineIndicator.querySelector('span').textContent = 'Offline Mode';
  } else {
    offlineIndicator.classList.remove('show');
  }
}

// Handle online event
function handleOnline() {
  isOnline = true;

  // Show brief "Back Online" message
  if (offlineIndicator) {
    offlineIndicator.classList.add('show', 'online');
    offlineIndicator.innerHTML = `
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polyline points="20 6 9 17 4 12"/>
      </svg>
      <span>Back Online</span>
    `;

    // Hide after 3 seconds
    setTimeout(() => {
      offlineIndicator.classList.remove('show');

      // Reset to offline indicator HTML
      offlineIndicator.innerHTML = `
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="1" y1="1" x2="23" y2="23"/>
          <path d="M16.72 11.06A10.94 10.94 0 0 1 19 12.55"/>
          <path d="M5 12.55a10.94 10.94 0 0 1 5.17-2.39"/>
          <path d="M10.71 5.05A16 16 0 0 1 22.58 9"/>
          <path d="M1.42 9a15.91 15.91 0 0 1 4.7-2.88"/>
          <path d="M8.53 16.11a6 6 0 0 1 6.95 0"/>
          <line x1="12" y1="20" x2="12.01" y2="20"/>
        </svg>
        <span>Offline Mode</span>
      `;
      offlineIndicator.classList.remove('online');

      // Prefetch fresh content
      setTimeout(() => {
        prefetchArticlesForOffline();
      }, 1000);
    }, 3000);
  }
}

// Handle offline event
function handleOffline() {
  isOnline = false;
  updateOnlineStatus();
}

// Prefetch articles for offline reading
function prefetchArticlesForOffline() {
  if (!navigator.serviceWorker || !navigator.serviceWorker.controller) {
    return;
  }

  // Send message to service worker to prefetch feed
  navigator.serviceWorker.controller.postMessage({
    type: 'PREFETCH_FEED',
    limit: 100 // Prefetch first 100 articles
  });

  console.log('[Offline] Prefetching 100 articles for offline reading...');
}

// Save specific article for offline
function saveArticleForOffline(articleId) {
  if (!navigator.serviceWorker || !navigator.serviceWorker.controller) {
    return;
  }

  navigator.serviceWorker.controller.postMessage({
    type: 'SAVE_ARTICLE',
    articleId: articleId
  });

  console.log(`[Offline] Saved article ${articleId} for offline reading`);
}

// Export functions
window.offlineSupport = {
  init: initOffline,
  saveArticle: saveArticleForOffline,
  prefetch: prefetchArticlesForOffline,
  isOnline: () => isOnline
};

// Auto-initialize on DOMContentLoaded
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initOffline);
} else {
  initOffline();
}
