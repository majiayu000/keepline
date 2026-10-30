import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type CostSource = 'claude' | 'codex';

interface CostTokenBreakdown {
  inputTokens: number;
  outputTokens: number;
  reasoningTokens: number;
  cacheCreationTokens: number;
  cacheReadTokens: number;
  totalTokens: number;
}

interface CostModelSummary {
  model: string;
  cost?: number | null;
  costUsd?: number | null;
  tokens: CostTokenBreakdown;
}

interface CostRangeSummary {
  range: string;
  label: string;
  since?: string | null;
  until?: string | null;
  currency: string;
  cost?: number | null;
  costUsd?: number | null;
  tokens: CostTokenBreakdown;
  models: CostModelSummary[];
  validEntries: number;
  skippedEntries: number;
  elapsedMs: number;
}

interface CostOverview {
  source: string;
  displayName: string;
  currency: string;
  generatedAt: string;
  cached: boolean;
  ranges: CostRangeSummary[];
}

interface CostSummarySectionProps {
  source: CostSource;
  refreshKey?: number;
}

function formatMoney(value: number | null | undefined, currency: string): string {
  if (value == null || !Number.isFinite(value)) return 'n/a';

  const maximumFractionDigits = Math.abs(value) < 1 ? 4 : 2;
  try {
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency,
      minimumFractionDigits: 2,
      maximumFractionDigits,
    }).format(value);
  } catch {
    return `${currency} ${value.toFixed(maximumFractionDigits)}`;
  }
}

function formatCompactNumber(value: number): string {
  if (!Number.isFinite(value)) return '0';
  return new Intl.NumberFormat('en-US', {
    notation: 'compact',
    maximumFractionDigits: 1,
  }).format(value);
}

function formatUpdatedAt(value: string): string {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return '';
  return date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
  });
}

function pickPrimaryRange(overview: CostOverview | null): CostRangeSummary | null {
  if (!overview) return null;
  return (
    overview.ranges.find((range) => range.range === 'today' && range.validEntries > 0) ??
    overview.ranges.find((range) => range.validEntries > 0) ??
    overview.ranges[0] ??
    null
  );
}

function finiteOrNull(value: number | null | undefined): number | null {
  return value != null && Number.isFinite(value) ? value : null;
}

function safeCount(value: number | null | undefined): number {
  return value != null && Number.isFinite(value) && value > 0 ? value : 0;
}

/** Parse a "YYYY-MM-DD" date as a calendar day (UTC midnight) so DST cannot skew the count. */
function parseDay(value?: string | null): number | null {
  if (!value) return null;
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(value);
  if (!match) return null;
  const day = Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  return Number.isFinite(day) ? day : null;
}

/** Inclusive number of calendar days covered by a range, or null when `since` is unknown. */
function rangeDays(range: CostRangeSummary): number | null {
  const since = parseDay(range.since);
  if (since == null) return null;
  const now = new Date();
  const until = parseDay(range.until) ?? Date.UTC(now.getFullYear(), now.getMonth(), now.getDate());
  const days = Math.round((until - since) / 86_400_000) + 1;
  return Number.isFinite(days) && days >= 1 ? days : null;
}

function formatPercent(share: number): string {
  if (!Number.isFinite(share)) return '0%';
  const pct = share * 100;
  return `${pct > 0 && pct < 10 ? pct.toFixed(1) : Math.round(pct)}%`;
}

const TOKEN_KINDS = [
  { key: 'cacheReadTokens', label: 'Cache read', color: 'var(--chart-ramp-4)' },
  { key: 'cacheCreationTokens', label: 'Cache write', color: 'var(--chart-ramp-3)' },
  { key: 'inputTokens', label: 'Input', color: 'var(--chart-ramp-2)' },
  { key: 'outputTokens', label: 'Output', color: 'var(--chart-ramp-1)' },
  { key: 'reasoningTokens', label: 'Reasoning', color: 'var(--chart-ramp-0)' },
] as const;

function CostFooter({ left, updatedAt }: { left: string; updatedAt: string }) {
  const updated = formatUpdatedAt(updatedAt);
  return (
    <div className="cost-footer">
      <span title={left}>{left}</span>
      {updated && <span>Updated {updated}</span>}
    </div>
  );
}

function RangeTile({
  range,
  selected,
  onSelect,
}: {
  range: CostRangeSummary;
  selected: boolean;
  onSelect: () => void;
}) {
  const cost = finiteOrNull(range.cost);
  const days = range.range === 'today' ? null : rangeDays(range);
  let detail: string | null = null;
  if (range.range === 'today') {
    detail = `${formatCompactNumber(safeCount(range.tokens.totalTokens))} tokens`;
  } else if (days != null && cost != null) {
    detail = `${formatMoney(cost / days, range.currency)}/day`;
  }

  return (
    <button
      type="button"
      className="cost-range"
      aria-pressed={selected}
      onClick={onSelect}
    >
      <span className="cost-range-label">{range.label}</span>
      <span className="cost-range-value">{formatMoney(cost, range.currency)}</span>
      {detail && <span className="cost-range-detail">{detail}</span>}
    </button>
  );
}

function RangeDetail({ range, updatedAt }: { range: CostRangeSummary; updatedAt: string }) {
  const kinds = TOKEN_KINDS.map((kind) => ({
    ...kind,
    value: safeCount(range.tokens[kind.key]),
  })).filter((kind) => kind.value > 0);
  const kindTotal = kinds.reduce((sum, kind) => sum + kind.value, 0);
  const totalTokens = safeCount(range.tokens.totalTokens) || kindTotal;

  const promptSide =
    safeCount(range.tokens.inputTokens) +
    safeCount(range.tokens.cacheCreationTokens) +
    safeCount(range.tokens.cacheReadTokens);
  const cacheHit = promptSide > 0 ? safeCount(range.tokens.cacheReadTokens) / promptSide : null;

  const models = [...range.models]
    .sort(
      (a, b) =>
        (finiteOrNull(b.cost) ?? 0) - (finiteOrNull(a.cost) ?? 0) ||
        safeCount(b.tokens.totalTokens) - safeCount(a.tokens.totalTokens),
    )
    .slice(0, 3);
  const topModel = models[0];
  const topCost = finiteOrNull(topModel?.cost) ?? 0;
  const topTokens = safeCount(topModel?.tokens.totalTokens);
  const shareOf = (model: CostModelSummary): number => {
    if (topCost > 0) return Math.max(0, finiteOrNull(model.cost) ?? 0) / topCost;
    if (topTokens > 0) return safeCount(model.tokens.totalTokens) / topTokens;
    return 0;
  };

  const rangeCost = finiteOrNull(range.cost);
  const topShare =
    topModel && rangeCost != null && rangeCost > 0 && topCost > 0
      ? Math.min(1, topCost / rangeCost)
      : null;

  return (
    <div className="cost-body">
      <div className="cost-tokens">
        <div className="cost-sub-head">
          <span>
            <b>{formatCompactNumber(totalTokens)}</b> tokens ·{' '}
            {safeCount(range.validEntries).toLocaleString('en-US')} entries
          </span>
          {cacheHit != null && <span>Cache hit {Math.round(cacheHit * 100)}%</span>}
        </div>

        {kinds.length > 0 && (
          <>
            <div
              className="cost-stack"
              role="img"
              aria-label={`Token composition: ${kinds
                .map((kind) => `${kind.label} ${formatPercent(kind.value / kindTotal)}`)
                .join(', ')}`}
            >
              {kinds.map((kind) => (
                <span
                  key={kind.key}
                  style={{ flex: `${kind.value} 0 0`, background: kind.color }}
                  title={`${kind.label}: ${formatCompactNumber(kind.value)} tokens`}
                />
              ))}
            </div>
            <div className="cost-token-legend">
              {kinds.map((kind) => (
                <div key={kind.key}>
                  <span className="cost-swatch" style={{ background: kind.color }} aria-hidden="true" />
                  <span className="cost-token-name">{kind.label}</span>
                  <span className="cost-token-figures">
                    <span className="cost-token-value">{formatCompactNumber(kind.value)}</span>
                    <span className="cost-token-share">{formatPercent(kind.value / kindTotal)}</span>
                  </span>
                </div>
              ))}
            </div>
          </>
        )}
      </div>

      {models.length > 0 && (
        <div className="cost-models">
          {models.map((model) => (
            <div className="cost-model-row" key={model.model}>
              <div className="cost-model-name">
                <span title={model.model}>{model.model}</span>
                <div className="cost-model-bar" aria-hidden="true">
                  <i style={{ width: `${(shareOf(model) * 100).toFixed(1)}%` }} />
                </div>
              </div>
              <span className="cost-model-tokens">
                {formatCompactNumber(safeCount(model.tokens.totalTokens))}
              </span>
              <span className="cost-model-cost">{formatMoney(model.cost, range.currency)}</span>
            </div>
          ))}
        </div>
      )}

      <CostFooter
        left={
          topShare != null && topModel
            ? `${Math.round(topShare * 100)}% of spend on ${topModel.model}`
            : range.label
        }
        updatedAt={updatedAt}
      />
    </div>
  );
}

export default function CostSummarySection({ source, refreshKey = 0 }: CostSummarySectionProps) {
  const [overview, setOverview] = useState<CostOverview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    const loadCost = async (force: boolean) => {
      try {
        setLoading(true);
        setError(null);
        const data = await invoke<CostOverview>('get_cost_overview', {
          source,
          currency: 'USD',
          timezone: null,
          force,
        });
        if (!cancelled) {
          setOverview(data);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Failed to load cost summary');
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    loadCost(refreshKey > 0);
    const interval = setInterval(() => loadCost(false), 300_000);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [source, refreshKey]);

  const [selectedRange, setSelectedRange] = useState<string | null>(null);
  const primaryRange = useMemo(() => pickPrimaryRange(overview), [overview]);
  const activeRange =
    overview?.ranges.find((range) => range.range === selectedRange) ?? primaryRange;
  const currency = overview?.currency ?? 'USD';

  return (
    <div className="section cost-section">
      <div className="section-title">
        LOCAL COST
        {overview && (
          <span className="section-aside">
            {currency} · {overview.cached ? 'cached' : 'estimated from local logs'}
          </span>
        )}
      </div>

      {loading && !overview && (
        <div className="cost-loading">Loading cost...</div>
      )}

      {error && !overview && (
        <div className="cost-inline-error">{error}</div>
      )}

      {overview && (
        <div className="cost-card">
          {overview.ranges.length > 0 && (
            <div className="cost-range-grid" role="group" aria-label="Cost range">
              {overview.ranges.map((range) => (
                <RangeTile
                  key={range.range}
                  range={range}
                  selected={range.range === activeRange?.range}
                  onSelect={() => setSelectedRange(range.range)}
                />
              ))}
            </div>
          )}

          {activeRange ? (
            <RangeDetail range={activeRange} updatedAt={overview.generatedAt} />
          ) : (
            <div className="cost-body">
              <CostFooter left={overview.displayName} updatedAt={overview.generatedAt} />
            </div>
          )}

          {error && <div className="cost-inline-error compact">{error}</div>}
        </div>
      )}
    </div>
  );
}
