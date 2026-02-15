interface QuotaCardProps {
  label: string;
  percentage: number;
  resetsIn: string;
}

function getStatusColor(percentage: number): string {
  if (percentage >= 80) return 'critical';
  if (percentage >= 50) return 'warning';
  return 'good';
}

function getStatusLabel(percentage: number): string {
  if (percentage >= 80) return 'Critical';
  if (percentage >= 50) return 'Warning';
  return 'Good';
}

export default function QuotaCard({ label, percentage, resetsIn }: QuotaCardProps) {
  const status = getStatusColor(percentage);
  const statusLabel = getStatusLabel(percentage);
  const clampedPercent = Math.max(0, Math.min(100, percentage));

  return (
    <div className="quota-card">
      <div className="quota-header">
        <span className="quota-label">{label}</span>
        <div className="quota-status">
          <span className={`status-badge ${status}`}>{statusLabel}</span>
          <span className="quota-percentage">{percentage}%</span>
        </div>
      </div>

      <div
        className="progress-bar"
        role="progressbar"
        aria-label={`${label} usage`}
        aria-valuenow={Math.round(clampedPercent)}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div
          className={`progress-fill ${status}`}
          style={{ width: `${clampedPercent}%` }}
        />
      </div>

      <div className="quota-footer">
        <span className="reset-icon" aria-hidden="true">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M21 12a9 9 0 1 1-2.64-6.36" />
            <path d="M21 3v6h-6" />
          </svg>
        </span>
        <span className="reset-text">Resets in {resetsIn}</span>
      </div>
    </div>
  );
}
