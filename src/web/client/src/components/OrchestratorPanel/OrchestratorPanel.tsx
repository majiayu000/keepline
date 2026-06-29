import { memo } from 'react'
import { Spinner } from '@/components/Spinner'
import { useOrchestratorOverview } from '@/hooks'
import type {
  OrchestratorDigest,
  OrchestratorQueueItem,
  OrchestratorReason,
  OrchestratorRecommendedAction,
} from '@/types'
import { formatCost, formatPath, formatRelativeTime } from '@/utils/format'
import styles from './OrchestratorPanel.module.css'

interface OrchestratorPanelProps {
  token: string
  onOpenSession?: (sessionId: string) => void
}

const ACTION_LABELS: Record<OrchestratorRecommendedAction, string> = {
  review: 'Review',
  recover: 'Recover',
  monitor: 'Monitor',
  resume: 'Resume',
  none: 'None',
}

export const OrchestratorPanel = memo(function OrchestratorPanel({
  token,
  onOpenSession,
}: OrchestratorPanelProps) {
  const { overview, loading, error, refresh } = useOrchestratorOverview(token)
  const stats = overview?.stats ?? {
    totalCandidates: 0,
    needingAttention: 0,
    critical: 0,
    warning: 0,
  }

  return (
    <section
      className={styles.panel}
      role="tabpanel"
      id="panel-orchestrator"
      aria-labelledby="tab-orchestrator"
    >
      <div className={styles.header}>
        <div className={styles.headerText}>
          <h2 className={styles.title}>Orchestrator</h2>
          <div className={styles.meta}>
            {overview ? `Generated ${formatRelativeTime(overview.generatedAt)}` : 'Waiting for overview'}
          </div>
        </div>
        <button
          type="button"
          className={styles.refreshButton}
          onClick={refresh}
          disabled={loading}
          aria-label="Refresh orchestrator overview"
          title="Refresh"
        >
          <span aria-hidden="true">@</span>
          <span>Refresh</span>
        </button>
      </div>

      <div className={styles.statsGrid}>
        <OrchestratorStat label="Candidates" value={stats.totalCandidates} tone="neutral" />
        <OrchestratorStat label="Attention" value={stats.needingAttention} tone="attention" />
        <OrchestratorStat label="Critical" value={stats.critical} tone="critical" />
        <OrchestratorStat label="Warning" value={stats.warning} tone="warning" />
      </div>

      {error && <div className={styles.error} role="alert">{error}</div>}

      {loading && !overview ? (
        <div className={styles.loading}>
          <Spinner size="sm" />
          <span>Loading overview...</span>
        </div>
      ) : overview && overview.items.length === 0 ? (
        <div className={styles.emptyState}>No sessions need attention</div>
      ) : overview ? (
        <div className={styles.queueList}>
          {overview.items.map((item) => (
            <QueueCard key={item.sessionId} item={item} onOpenSession={onOpenSession} />
          ))}
        </div>
      ) : null}
    </section>
  )
})

function OrchestratorStat({
  label,
  value,
  tone,
}: {
  label: string
  value: number
  tone: 'neutral' | 'attention' | 'critical' | 'warning'
}) {
  return (
    <div className={`${styles.statCard} ${styles[tone]}`}>
      <span className={styles.statValue}>{value}</span>
      <span className={styles.statLabel}>{label}</span>
    </div>
  )
}

function QueueCard({
  item,
  onOpenSession,
}: {
  item: OrchestratorQueueItem
  onOpenSession?: (sessionId: string) => void
}) {
  return (
    <article className={styles.queueCard}>
      <div className={styles.rankBlock}>
        <span className={styles.rank}>#{item.rank}</span>
        <span className={styles.score}>{item.score}</span>
      </div>

      <div className={styles.cardBody}>
        <div className={styles.cardHeader}>
          <div className={styles.identity}>
            <h3 className={styles.itemTitle}>{item.title || item.sessionId}</h3>
            <div className={styles.pathLine}>{formatPath(item.directory)}</div>
          </div>
          <div className={styles.badges}>
            <span className={`${styles.badge} ${styles[item.status]}`}>{item.status}</span>
            <span className={styles.badge}>{item.client}</span>
            <span className={`${styles.actionBadge} ${styles[item.recommendedAction]}`}>
              {ACTION_LABELS[item.recommendedAction]}
            </span>
            {onOpenSession && (
              <button
                type="button"
                className={styles.openButton}
                onClick={() => onOpenSession(item.sessionId)}
              >
                Open
              </button>
            )}
          </div>
        </div>

        <div className={styles.metaRow}>
          <span>{formatRelativeTime(item.lastActiveAt)}</span>
          <span>{item.processRunning ? 'process running' : 'no process'}</span>
          {item.usageCost != null && <span>{formatCost(item.usageCost)}</span>}
          <span>{item.sessionId}</span>
        </div>

        <ReasonList reasons={item.reasons} />
        {item.digest && <DigestBlock digest={item.digest} />}
      </div>
    </article>
  )
}

function ReasonList({ reasons }: { reasons: OrchestratorReason[] }) {
  if (reasons.length === 0) {
    return <div className={styles.reasonEmpty}>No active reasons</div>
  }

  return (
    <div className={styles.reasonList}>
      {reasons.map((reason) => (
        <span
          key={`${reason.code}-${reason.score}`}
          className={`${styles.reasonPill} ${styles[reason.severity]}`}
          title={reason.code}
        >
          <span className={styles.reasonCode}>{reason.code}</span>
          <span>{reason.message}</span>
        </span>
      ))}
    </div>
  )
}

function DigestBlock({ digest }: { digest: OrchestratorDigest }) {
  return (
    <div className={`${styles.digestBlock} ${styles[`digest_${digest.status}`]}`}>
      <div className={styles.digestHeader}>
        <span>{digest.source}</span>
        <span>{digest.status}</span>
        <span>{formatRelativeTime(digest.generatedAt)}</span>
        {digest.provider && <span>{digest.provider}</span>}
      </div>

      {digest.summary && <p className={styles.digestSummary}>{digest.summary}</p>}
      {digest.errorMessage && <div className={styles.digestError}>{digest.errorMessage}</div>}

      {(digest.nextActions.length > 0 || digest.blockers.length > 0) && (
        <div className={styles.digestLists}>
          <DigestList title="Next" items={digest.nextActions} />
          <DigestList title="Blockers" items={digest.blockers} />
        </div>
      )}
    </div>
  )
}

function DigestList({ title, items }: { title: string; items: string[] }) {
  if (items.length === 0) return null
  return (
    <div className={styles.digestList}>
      <span className={styles.digestListTitle}>{title}</span>
      <ul>
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </div>
  )
}
