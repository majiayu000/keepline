import React from 'react';

interface StatusHeaderProps {
  connected: boolean;
  loading: boolean;
  lastUpdated?: Date;
}

export default function StatusHeader({ connected, loading, lastUpdated }: StatusHeaderProps) {
  const formatTime = (date: Date) => {
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  };

  return (
    <div className="status-header">
      <div className="title-row">
        <h1 className="app-title">Claude Quota</h1>
        <div className={`connection-status ${connected ? 'connected' : 'disconnected'}`}>
          <span className="status-dot" />
          <span className="status-text">
            {loading ? 'Loading...' : connected ? 'Connected' : 'Disconnected'}
          </span>
        </div>
      </div>

      {lastUpdated && (
        <div className="last-updated">
          Updated at {formatTime(lastUpdated)}
        </div>
      )}
    </div>
  );
}
