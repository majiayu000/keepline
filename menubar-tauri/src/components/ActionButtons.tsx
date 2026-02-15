interface ActionButtonsProps {
  onRefresh: () => void;
  onDashboard: () => void;
  onQuit: () => void;
  loading: boolean;
}

function RefreshIcon({ spinning }: { spinning: boolean }) {
  return (
    <svg
      className={`btn-icon-svg ${spinning ? 'icon-spin' : ''}`}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M21 12a9 9 0 1 1-2.64-6.36" />
      <path d="M21 3v6h-6" />
    </svg>
  );
}

function DashboardIcon() {
  return (
    <svg
      className="btn-icon-svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <rect x="3" y="3" width="8" height="8" />
      <rect x="13" y="3" width="8" height="8" />
      <rect x="3" y="13" width="8" height="8" />
      <rect x="13" y="13" width="8" height="8" />
    </svg>
  );
}

function PowerIcon() {
  return (
    <svg
      className="btn-icon-svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M12 2v10" />
      <path d="M6.2 6.2a8 8 0 1 0 11.6 0" />
    </svg>
  );
}

export default function ActionButtons({
  onRefresh,
  onDashboard,
  onQuit,
  loading,
}: ActionButtonsProps) {
  return (
    <div className="action-buttons">
      <button
        className="action-btn refresh-btn"
        onClick={onRefresh}
        disabled={loading}
      >
        <RefreshIcon spinning={loading} />
        <span className="btn-text">{loading ? 'Loading' : 'Refresh'}</span>
      </button>

      <button className="action-btn dashboard-btn" onClick={onDashboard}>
        <DashboardIcon />
        <span className="btn-text">Dashboard</span>
      </button>

      <button className="action-btn quit-btn" onClick={onQuit}>
        <PowerIcon />
        <span className="btn-text">Quit</span>
      </button>
    </div>
  );
}
