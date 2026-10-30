const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
// Clamp the elapsed share so a window that just started does not divide by ~0.
const MIN_ELAPSED_FRACTION = 0.02;

export const FIVE_HOURS_MS = 5 * HOUR_MS;
export const SEVEN_DAYS_MS = 7 * 24 * HOUR_MS;

type PaceLevel = 'ok' | 'warn' | 'critical';

interface Pace {
  remainingMs: number;
  elapsedFraction: number;
  projected: number;
  fullInMs: number | null;
  level: PaceLevel;
}

export interface QuotaCardProps {
  /** Short name, e.g. "Session", "Weekly", "Opus". */
  name: string;
  /** Window caption, e.g. "5-hour", "7-day". Optional when unknown. */
  windowLabel?: string;
  /** Used percentage, 0-100 (may exceed 100 on overage). */
  percentage: number;
  /** When the window resets. Missing or invalid disables tick and projection. */
  resetsAt?: Date | null;
  /** Full window length in ms. Missing disables tick and projection. */
  windowMs?: number | null;
}

/** Parse an ISO string or unix seconds into a valid Date, else null. */
export function toResetDate(value?: string | number | null): Date | null {
  if (value == null || value === '') return null;
  const date = typeof value === 'number' ? new Date(value * 1000) : new Date(value);
  return Number.isFinite(date.getTime()) ? date : null;
}

export function formatDuration(ms: number): string {
  const totalMinutes = Math.max(0, Math.round(ms / MINUTE_MS));
  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

function formatClock(date: Date, remainingMs: number): string {
  return date.toLocaleString('en-US', {
    weekday: remainingMs > 20 * HOUR_MS ? 'short' : undefined,
    hour: 'numeric',
    minute: '2-digit',
  });
}

function computePace(used: number, resetsAt: Date, windowMs: number, now: number): Pace | null {
  const remainingMs = Math.max(0, resetsAt.getTime() - now);
  // A reset further away than one window means the inputs disagree; do not guess.
  if (remainingMs > windowMs * 1.01) return null;
  const elapsedFraction = Math.min(1, Math.max(MIN_ELAPSED_FRACTION, 1 - remainingMs / windowMs));
  const projected = used / elapsedFraction;
  let fullInMs: number | null = null;
  if (used > 0 && used < 100) {
    const ratePerMs = used / (elapsedFraction * windowMs);
    fullInMs = (100 - used) / ratePerMs;
  } else if (used >= 100) {
    fullInMs = 0;
  }
  const level: PaceLevel = projected >= 100 ? 'critical' : projected >= 90 ? 'warn' : 'ok';
  return { remainingMs, elapsedFraction, projected, fullInMs, level };
}

function CriticalIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <path d="M8 2.5 14.5 13.5h-13z" />
      <path d="M8 7v2.6M8 11.6v.1" />
    </svg>
  );
}

function WarnIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <circle cx="8" cy="8" r="6" />
      <path d="M8 5v3.5M8 10.8v.1" />
    </svg>
  );
}

function PaceLabel({ pace, used }: { pace: Pace; used: number }) {
  if (pace.level === 'critical') {
    const text =
      used >= 100 || pace.fullInMs === 0
        ? 'Limit reached'
        : `Full in ~${formatDuration(pace.fullInMs ?? 0)}`;
    return (
      <span className="quota-pace critical">
        <CriticalIcon />
        {text}
      </span>
    );
  }
  if (pace.level === 'warn') {
    return (
      <span className="quota-pace warn">
        <WarnIcon />
        Near limit at reset
      </span>
    );
  }
  return <span className="quota-pace ok">On pace for ~{Math.round(pace.projected)}%</span>;
}

export default function QuotaCard({
  name,
  windowLabel,
  percentage,
  resetsAt,
  windowMs,
}: QuotaCardProps) {
  const used = Number.isFinite(percentage) ? Math.max(0, percentage) : 0;
  const usedRounded = Math.round(used);
  const usedWidth = Math.min(100, used);
  const now = Date.now();
  const validReset = resetsAt && Number.isFinite(resetsAt.getTime()) ? resetsAt : null;
  const validWindow = windowMs != null && Number.isFinite(windowMs) && windowMs > 0 ? windowMs : null;
  const pace = validReset && validWindow ? computePace(used, validReset, validWindow, now) : null;
  const projectedWidth = pace ? Math.min(100, pace.projected) - usedWidth : 0;
  const elapsedPercent = pace ? Math.round(pace.elapsedFraction * 100) : null;
  const remainingMs = validReset ? validReset.getTime() - now : null;

  const ariaLabel = [
    `${name}${windowLabel ? ` ${windowLabel}` : ''}: ${usedRounded}% used`,
    elapsedPercent != null ? `${elapsedPercent}% of window elapsed` : null,
    pace ? `projected ${Math.round(pace.projected)}% at reset` : null,
  ]
    .filter(Boolean)
    .join(', ');

  const tooltip = pace
    ? `${usedRounded}% used · ${elapsedPercent}% of window elapsed\nStraight-line estimate at reset: ${Math.round(pace.projected)}%` +
      (pace.fullInMs != null && pace.fullInMs > 0 && pace.fullInMs < pace.remainingMs
        ? `\nHits 100% in ~${formatDuration(pace.fullInMs)}`
        : '')
    : `${usedRounded}% used`;

  return (
    <div className={`quota-card pace-${pace?.level ?? 'none'}`}>
      <div className="quota-head">
        <span className="quota-name" title={name}>{name}</span>
        {windowLabel && <span className="quota-window">{windowLabel}</span>}
        <span className="quota-used">
          <b>{usedRounded}</b>
          <span>% used</span>
        </span>
      </div>

      <div
        className="quota-meter"
        role="meter"
        aria-label={ariaLabel}
        aria-valuenow={usedRounded}
        aria-valuemin={0}
        aria-valuemax={100}
        title={tooltip}
      >
        <div className="quota-track">
          <span className="quota-fill" style={{ width: `${usedWidth}%` }} />
          {projectedWidth > 0.5 && (
            <span
              className="quota-projected"
              style={{ left: `${usedWidth}%`, width: `${projectedWidth}%` }}
            />
          )}
        </div>
        <span className="quota-cap" aria-hidden="true" />
        {pace && (
          <span
            className="quota-now"
            aria-hidden="true"
            style={{ left: `${(pace.elapsedFraction * 100).toFixed(1)}%` }}
          />
        )}
      </div>

      <div className="quota-foot">
        {validReset && remainingMs != null ? (
          remainingMs > 0 ? (
            <>
              <span>Resets in {formatDuration(remainingMs)}</span>
              <span className="quota-sep" aria-hidden="true">·</span>
              <span>{formatClock(validReset, remainingMs)}</span>
            </>
          ) : (
            <span>Resets soon</span>
          )
        ) : (
          <span>Reset time unknown</span>
        )}
        {pace && <PaceLabel pace={pace} used={used} />}
      </div>
    </div>
  );
}

export function QuotaLegend() {
  return (
    <div className="quota-legend">
      <span>
        <i className="quota-legend-now" aria-hidden="true" />
        Time elapsed in window
      </span>
      <span>
        <i className="quota-legend-projected" aria-hidden="true" />
        Projected at reset
      </span>
    </div>
  );
}
