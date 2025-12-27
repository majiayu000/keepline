import React, { useEffect, useState, useCallback } from 'react';
import QuotaCard from './components/QuotaCard';
import StatusHeader from './components/StatusHeader';
import ActionButtons from './components/ActionButtons';
import ThemeSelector, { ThemeName } from './components/ThemeSelector';

interface QuotaLimit {
  used: number;
  limit: number;
  remaining: number;
  percentage: number;
  resetsAt: Date;
  resetsIn: string;
}

interface QuotaData {
  connected: boolean;
  session?: QuotaLimit;
  weeklyTotal?: QuotaLimit;
  weeklyOpus?: QuotaLimit;
  weeklySonnet?: QuotaLimit;
  lastUpdated: Date;
  error?: string;
}

// Storage key for theme preference
const THEME_STORAGE_KEY = 'claude-quota-theme';

// Get saved theme or default based on system preference
function getSavedTheme(): ThemeName {
  try {
    const saved = localStorage.getItem(THEME_STORAGE_KEY);
    if (saved && ['light', 'dark', 'claude', 'claude-dark', 'minimal', 'minimal-dark', 'ocean'].includes(saved)) {
      return saved as ThemeName;
    }
  } catch {}
  return 'light';
}

export default function App() {
  const [quota, setQuota] = useState<QuotaData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [theme, setTheme] = useState<ThemeName>(getSavedTheme);

  const fetchQuota = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await window.electronAPI.getQuota();

      if ('error' in data && typeof data.error === 'string') {
        setError(data.error);
        setQuota(null);
      } else {
        setQuota(data as QuotaData);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    // Initial fetch
    fetchQuota();

    // Listen for refresh events
    const unsubscribe = window.electronAPI.onRefreshQuota(fetchQuota);

    return () => {
      unsubscribe();
    };
  }, [fetchQuota]);

  // Handle theme change
  const handleThemeChange = useCallback((newTheme: ThemeName) => {
    setTheme(newTheme);
    try {
      localStorage.setItem(THEME_STORAGE_KEY, newTheme);
    } catch {}
  }, []);

  const handleRefresh = () => {
    fetchQuota();
  };

  const handleOpenDashboard = () => {
    window.electronAPI.openExternal('http://localhost:3377');
  };

  const handleQuit = () => {
    window.electronAPI.quit();
  };

  return (
    <div className={`app theme-${theme}`}>
      <div className="container">
        <StatusHeader
          connected={quota?.connected ?? false}
          loading={loading}
          lastUpdated={quota?.lastUpdated ? new Date(quota.lastUpdated) : undefined}
        />

        <ThemeSelector currentTheme={theme} onThemeChange={handleThemeChange} />

        {error && (
          <div className="error-banner">
            <span className="error-icon">!</span>
            <span className="error-text">{error}</span>
          </div>
        )}

        {!error && quota && (
          <div className="quota-list">
            {/* 5-Hour Session */}
            <div className="section">
              <div className="section-title">CURRENT SESSION</div>
              {quota.session ? (
                <QuotaCard
                  label="5-Hour Usage"
                  percentage={quota.session.percentage}
                  resetsIn={quota.session.resetsIn}
                />
              ) : (
                <div className="no-data">No session data</div>
              )}
            </div>

            {/* Weekly Limits */}
            <div className="section">
              <div className="section-title">WEEKLY LIMITS</div>

              {quota.weeklyTotal && (
                <QuotaCard
                  label="7-Day Usage"
                  percentage={quota.weeklyTotal.percentage}
                  resetsIn={quota.weeklyTotal.resetsIn}
                />
              )}

              {quota.weeklyOpus && (
                <QuotaCard
                  label="Opus (7-Day)"
                  percentage={quota.weeklyOpus.percentage}
                  resetsIn={quota.weeklyOpus.resetsIn}
                />
              )}

              {quota.weeklySonnet && (
                <QuotaCard
                  label="Sonnet (7-Day)"
                  percentage={quota.weeklySonnet.percentage}
                  resetsIn={quota.weeklySonnet.resetsIn}
                />
              )}

              {!quota.weeklyTotal && !quota.weeklyOpus && !quota.weeklySonnet && (
                <div className="no-data">No weekly data</div>
              )}
            </div>
          </div>
        )}

        {!error && !quota && !loading && (
          <div className="empty-state">
            <p>Unable to load quota data</p>
            <button onClick={handleRefresh} className="retry-btn">
              Try Again
            </button>
          </div>
        )}

        <ActionButtons
          onRefresh={handleRefresh}
          onDashboard={handleOpenDashboard}
          onQuit={handleQuit}
          loading={loading}
        />
      </div>
    </div>
  );
}
