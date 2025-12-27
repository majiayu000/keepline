import { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { exit } from '@tauri-apps/plugin-process';
import QuotaCard from './components/QuotaCard';
import StatusHeader from './components/StatusHeader';
import ActionButtons from './components/ActionButtons';
import ThemeSelector, { ThemeName } from './components/ThemeSelector';
import './styles.css';

interface UsageInfo {
  used: number;
  limit: number;
  percentage: number;
  resetTime?: string;
}

interface QuotaData {
  connected: boolean;
  session?: UsageInfo;
  weeklyTotal?: UsageInfo;
  weeklyOpus?: UsageInfo;
  weeklySonnet?: UsageInfo;
  error?: string;
}

const THEME_STORAGE_KEY = 'claude-quota-theme';

function getSavedTheme(): ThemeName {
  try {
    const saved = localStorage.getItem(THEME_STORAGE_KEY);
    if (saved && ['light', 'dark', 'claude', 'claude-dark', 'minimal', 'minimal-dark', 'ocean'].includes(saved)) {
      return saved as ThemeName;
    }
  } catch {}
  return 'light';
}

function formatResetTime(resetTime?: string): string {
  if (!resetTime) return 'Unknown';
  try {
    const reset = new Date(resetTime);
    const now = new Date();
    const diff = reset.getTime() - now.getTime();
    if (diff <= 0) return 'Soon';

    const hours = Math.floor(diff / (1000 * 60 * 60));
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));

    if (hours > 24) {
      const days = Math.floor(hours / 24);
      return `${days}d ${hours % 24}h`;
    }
    return `${hours}h ${minutes}m`;
  } catch {
    return 'Unknown';
  }
}

export default function App() {
  const [quota, setQuota] = useState<QuotaData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [theme, setTheme] = useState<ThemeName>(getSavedTheme);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);

  const fetchQuota = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await invoke<QuotaData>('get_quota');

      if (data.error) {
        setError(data.error);
        setQuota(null);
      } else {
        setQuota(data);
        setLastUpdated(new Date());

        // Update tray icon with session percentage
        const percentage = data.session?.percentage ?? 0;
        try {
          await invoke('update_tray_icon', { percentage: Math.round(percentage) });
        } catch (iconErr) {
          console.error('Failed to update tray icon:', iconErr);
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchQuota();

    // Auto-refresh every 60 seconds
    const interval = setInterval(fetchQuota, 60000);
    return () => clearInterval(interval);
  }, [fetchQuota]);

  const handleThemeChange = useCallback((newTheme: ThemeName) => {
    setTheme(newTheme);
    try {
      localStorage.setItem(THEME_STORAGE_KEY, newTheme);
    } catch {}
  }, []);

  const handleRefresh = () => {
    fetchQuota();
  };

  const handleOpenDashboard = async () => {
    try {
      await openUrl('http://localhost:3377');
    } catch (err) {
      console.error('Failed to open dashboard:', err);
    }
  };

  const handleQuit = async () => {
    try {
      await exit(0);
    } catch (err) {
      console.error('Failed to quit:', err);
    }
  };

  return (
    <div className={`app theme-${theme}`}>
      <div className="container">
        <StatusHeader
          connected={quota?.connected ?? false}
          loading={loading}
          lastUpdated={lastUpdated ?? undefined}
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
            <div className="section">
              <div className="section-title">CURRENT SESSION</div>
              {quota.session ? (
                <QuotaCard
                  label="5-Hour Usage"
                  percentage={Math.round(quota.session.percentage)}
                  resetsIn={formatResetTime(quota.session.resetTime)}
                />
              ) : (
                <div className="no-data">No session data</div>
              )}
            </div>

            <div className="section">
              <div className="section-title">WEEKLY LIMITS</div>

              {quota.weeklyTotal && (
                <QuotaCard
                  label="7-Day Usage"
                  percentage={Math.round(quota.weeklyTotal.percentage)}
                  resetsIn={formatResetTime(quota.weeklyTotal.resetTime)}
                />
              )}

              {quota.weeklyOpus && (
                <QuotaCard
                  label="Opus (7-Day)"
                  percentage={Math.round(quota.weeklyOpus.percentage)}
                  resetsIn={formatResetTime(quota.weeklyOpus.resetTime)}
                />
              )}

              {quota.weeklySonnet && (
                <QuotaCard
                  label="Sonnet (7-Day)"
                  percentage={Math.round(quota.weeklySonnet.percentage)}
                  resetsIn={formatResetTime(quota.weeklySonnet.resetTime)}
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
