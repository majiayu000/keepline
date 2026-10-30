import { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import CostSummarySection from './CostSummarySection';
import QuotaCard, { QuotaLegend, toResetDate } from './QuotaCard';

interface CodexData {
  connected: boolean;
  planType?: string;
  accountId?: string;
  subscriptionUntil?: string;
  email?: string;
  error?: string;
}

interface CodexStats {
  totalSessions: number;
  todaySessions: number;
  lastActivity?: string;
  error?: string;
}

interface CodexRateLimitWindow {
  usedPercent: number;
  windowMinutes?: number;
  resetsAt?: number;
}

interface CodexCredits {
  hasCredits: boolean;
  unlimited: boolean;
  balance?: string;
}

interface CodexRateLimits {
  connected: boolean;
  planType?: string;
  primary?: CodexRateLimitWindow;
  secondary?: CodexRateLimitWindow;
  credits?: CodexCredits;
  error?: string;
}

interface CodexPanelProps {
  onConnectionChange?: (connected: boolean) => void;
  onUsageChange?: (usedPercent: number | null) => void;
  refreshKey?: number;
}

function formatPlanType(planType?: string): string {
  if (!planType) return 'Unknown';
  return planType.charAt(0).toUpperCase() + planType.slice(1);
}

function formatSubscriptionDate(dateStr?: string): string {
  if (!dateStr) return 'Unknown';
  try {
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  } catch {
    return dateStr;
  }
}

function formatWindowLabel(minutes?: number): string | undefined {
  if (!minutes || minutes <= 0) return undefined;
  if (minutes >= 1440 && minutes % 1440 === 0) return `${minutes / 1440}-day`;
  if (minutes >= 60 && minutes % 60 === 0) return `${minutes / 60}-hour`;
  if (minutes >= 1440) return `${Math.round(minutes / 1440)}-day`;
  if (minutes >= 60) return `${Math.round(minutes / 60)}-hour`;
  return `${minutes}-minute`;
}

function CodexQuotaCard({ name, limit }: { name: string; limit: CodexRateLimitWindow }) {
  return (
    <QuotaCard
      name={name}
      windowLabel={formatWindowLabel(limit.windowMinutes)}
      percentage={limit.usedPercent}
      resetsAt={toResetDate(limit.resetsAt)}
      windowMs={limit.windowMinutes ? limit.windowMinutes * 60_000 : null}
    />
  );
}

export default function CodexPanel({
  onConnectionChange,
  onUsageChange,
  refreshKey = 0,
}: CodexPanelProps) {
  const [codexData, setCodexData] = useState<CodexData | null>(null);
  const [codexStats, setCodexStats] = useState<CodexStats | null>(null);
  const [rateLimits, setRateLimits] = useState<CodexRateLimits | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      const [info, stats, limits] = await Promise.all([
        invoke<CodexData>('get_codex_info'),
        invoke<CodexStats>('get_codex_stats'),
        invoke<CodexRateLimits>('get_codex_rate_limits'),
      ]);

      setCodexData(info);
      setCodexStats(stats);
      setRateLimits(limits);

      if (limits.error) {
        setError(limits.error);
      } else if (info.error) {
        setError(info.error);
      } else if (stats.error) {
        setError(stats.error);
      }

      // Notify parent about connection status change
      const isConnected = limits.connected || info.connected;
      onConnectionChange?.(isConnected);

      // Notify parent about used percentage for tray icon.
      // Prefer weekly (secondary) since it's the binding constraint; fall back to 5h (primary).
      const trayPercent = limits.secondary?.usedPercent ?? limits.primary?.usedPercent;
      const usedPercent = trayPercent != null
        ? Math.round(Math.max(0, Math.min(100, trayPercent)))
        : null;
      onUsageChange?.(usedPercent);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch Codex data');
      onConnectionChange?.(false);
      onUsageChange?.(null);
    } finally {
      setLoading(false);
    }
  }, [onConnectionChange, onUsageChange]);

  useEffect(() => {
    fetchData();
    // Refresh every 30 seconds
    const interval = setInterval(fetchData, 30000);
    return () => clearInterval(interval);
  }, [fetchData, refreshKey]);

  const handleOpenChatGPT = async () => {
    try {
      await invoke('open_chatgpt_quota');
    } catch (err) {
      console.error('Failed to open ChatGPT:', err);
    }
  };

  if (loading && !codexData && !rateLimits) {
    return (
      <div className="codex-panel">
        <div className="loading-state">Loading Codex info...</div>
      </div>
    );
  }

  const hasRateLimits = rateLimits?.primary || rateLimits?.secondary;
  const connected = rateLimits?.connected || codexData?.connected;
  const planType = rateLimits?.planType || codexData?.planType;

  return (
    <div className="codex-panel">
      {error && (
        <div className="error-banner">
          <span className="error-icon">!</span>
          <span className="error-text">{error}</span>
        </div>
      )}

      {connected && (
        <div className="codex-content">
          {/* Rate Limits Section */}
          {hasRateLimits && (
            <div className="section quota-section">
              <div className="section-title">
                USAGE LIMITS
                <span className="plan-tag">{formatPlanType(planType)}</span>
              </div>

              {rateLimits?.primary && (
                <CodexQuotaCard name="Primary" limit={rateLimits.primary} />
              )}

              {rateLimits?.secondary && (
                <CodexQuotaCard name="Secondary" limit={rateLimits.secondary} />
              )}

              <QuotaLegend />

              {rateLimits?.credits?.hasCredits && (
                <div className="quota-card credits-card">
                  <div className="quota-header">
                    <span className="quota-label">Credits</span>
                    <span className="quota-value">
                      {rateLimits.credits.unlimited
                        ? 'Unlimited'
                        : rateLimits.credits.balance || '0'}
                    </span>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Subscription Section (only if no rate limits) */}
          {!hasRateLimits && codexData && (
            <div className="section">
              <div className="section-title">SUBSCRIPTION</div>
              <div className="codex-card">
                <div className="codex-row">
                  <span className="codex-label">Plan</span>
                  <span className="codex-value plan-badge">
                    {formatPlanType(planType)}
                  </span>
                </div>
                <div className="codex-row">
                  <span className="codex-label">Valid Until</span>
                  <span className="codex-value">
                    {formatSubscriptionDate(codexData.subscriptionUntil)}
                  </span>
                </div>
                {codexData.email && (
                  <div className="codex-row">
                    <span className="codex-label">Account</span>
                    <span className="codex-value email">{codexData.email}</span>
                  </div>
                )}
              </div>
            </div>
          )}

          <CostSummarySection source="codex" refreshKey={refreshKey} />

          {/* ChatGPT Link */}
          <button className="open-chatgpt-btn" onClick={handleOpenChatGPT}>
            Open ChatGPT
          </button>

          {/* Local Stats Section */}
          {codexStats && (codexStats.totalSessions > 0 || codexStats.todaySessions > 0) && (
            <div className="section">
              <div className="section-title">LOCAL STATS</div>
              <div className="codex-card">
                <div className="codex-row">
                  <span className="codex-label">Today</span>
                  <span className="codex-value">{codexStats.todaySessions} sessions</span>
                </div>
                <div className="codex-row">
                  <span className="codex-label">Total</span>
                  <span className="codex-value">{codexStats.totalSessions} sessions</span>
                </div>
              </div>
            </div>
          )}
        </div>
      )}

      {!connected && !error && (
        <div className="empty-state">
          <p>Codex not connected</p>
          <p className="hint">Run 'codex' in terminal to login</p>
        </div>
      )}
    </div>
  );
}
