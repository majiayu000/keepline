import { memo } from 'react'
import { Spinner } from '@/components/Spinner'
import { useOrchestratorOverview } from '@/hooks'
import type {
  OrchestratorDigest,
  OrchestratorIntent,
  OrchestratorQueueItem,
  OrchestratorReason,
  OrchestratorRecommendedAction,
} from '@/types'
import { formatCost, formatPath, formatRelativeTime } from '@/utils/format'
import styles from './OrchestratorPanel.module.css'

interface OrchestratorPanelProps {
  token: string
  onOpenSession?: (sessionId: string) => void
  onRecover?: (sessionId: string) => void | Promise<void>
  onStop?: (sessionId: string) => void | Promise<void>
  onComplete?: (sessionId: string) => void | Promise<void>
  onCopySessionId?: (sessionId: string) => void | Promise<void>
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
  onRecover,
  onStop,
  onComplete,
  onCopySessionId,
}: OrchestratorPanelProps) {
  const { overview, loading, error, refresh } = useOrchestratorOverview(token)
  const stats = overview?.stats ?? {
    totalCandidates: 0,
    needingAttention: 0,
    critical: 0,
    warning: 0,
    hiddenOldLost: 0,
  }
  const scopeText = formatScopeText(stats.hiddenOldLost, stats.lostWindowHours)

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
            {overview
              ? `Generated ${formatRelativeTime(overview.generatedAt)}${scopeText}`
              : 'Waiting for overview'}
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
            <QueueCard
              key={item.sessionId}
              item={item}
              onOpenSession={onOpenSession}
              onRecover={onRecover}
              onStop={onStop}
              onComplete={onComplete}
              onCopySessionId={onCopySessionId}
              onActionComplete={refresh}
            />
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
  onRecover,
  onStop,
  onComplete,
  onCopySessionId,
  onActionComplete,
}: {
  item: OrchestratorQueueItem
  onOpenSession?: (sessionId: string) => void
  onRecover?: (sessionId: string) => void | Promise<void>
  onStop?: (sessionId: string) => void | Promise<void>
  onComplete?: (sessionId: string) => void | Promise<void>
  onCopySessionId?: (sessionId: string) => void | Promise<void>
  onActionComplete?: () => void | Promise<void>
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
            <h3 className={styles.itemTitle}>{item.intent.task || 'No clear task captured'}</h3>
            <div className={styles.pathLine}>{formatIdentitySubtitle(item)}</div>
          </div>
          <div className={styles.badges}>
            <span className={`${styles.badge} ${styles[item.status]}`}>{item.status}</span>
            <span className={styles.badge}>{item.client}</span>
            <span className={`${styles.actionBadge} ${styles[item.recommendedAction]}`}>
              {ACTION_LABELS[item.recommendedAction]}
            </span>
          </div>
        </div>

        <div className={styles.metaRow}>
          <span>{formatRelativeTime(item.lastActiveAt)}</span>
          <span>{item.processRunning ? 'process running' : 'no process'}</span>
          <span>{item.context.messageCount} messages</span>
          <span>{item.context.toolCount} tools</span>
          {item.usageCost != null && <span>{formatCost(item.usageCost)}</span>}
          <span>{item.sessionId}</span>
        </div>

        <ReasonList reasons={item.reasons} />
        <IntentBlock intent={item.intent} />
        {item.digest && <DigestBlock digest={item.digest} />}
        <RawEvidenceBlock item={item} />
        <ActionRow
          item={item}
          onOpenSession={onOpenSession}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
          onCopySessionId={onCopySessionId}
          onActionComplete={onActionComplete}
        />
      </div>
    </article>
  )
}

function IntentBlock({ intent }: { intent: OrchestratorIntent }) {
  const rows = [
    { label: 'Task', text: intent.task },
    { label: 'Current state', text: intent.currentState },
    { label: 'Next action', text: intent.nextAction },
    { label: 'Why attention', text: intent.whyAttention },
  ].filter((row): row is { label: string; text: string } => Boolean(row.text))

  return (
    <div className={styles.intentBlock}>
      <div className={styles.intentHeader}>
        <span className={`${styles.confidenceBadge} ${styles[`confidence_${intent.confidence}`]}`}>
          {intent.confidence} confidence
        </span>
        {intent.noiseFlags.map((flag) => (
          <span className={styles.noiseBadge} key={flag}>{formatNoiseFlag(flag)}</span>
        ))}
      </div>
      <div className={styles.intentGrid}>
        {rows.map((row) => (
          <section className={styles.intentSection} key={row.label}>
            <div className={styles.intentLabel}>{row.label}</div>
            <div className={styles.intentText}>{row.text}</div>
          </section>
        ))}
      </div>
    </div>
  )
}

function RawEvidenceBlock({ item }: { item: OrchestratorQueueItem }) {
  const rows = getContextRows(item)
  const details = [
    item.intent.evidence.currentFile ? `File: ${item.intent.evidence.currentFile}` : undefined,
    item.intent.evidence.lastTool ? `Last tool: ${item.intent.evidence.lastTool}` : undefined,
  ].filter((value): value is string => Boolean(value))

  if (rows.length === 0 && details.length === 0) {
    return null
  }

  return (
    <details className={styles.rawEvidence}>
      <summary className={styles.rawSummary}>Raw evidence</summary>
      <div className={styles.contextBlock}>
        {rows.map((row) => (
          <section className={styles.contextSection} key={row.label}>
            <div className={styles.contextLabel}>{row.label}</div>
            <div className={styles.contextText}>{row.text}</div>
          </section>
        ))}
        {details.length > 0 && (
          <div className={styles.contextMeta}>
            {details.map((detail) => (
              <span key={detail}>{detail}</span>
            ))}
          </div>
        )}
      </div>
    </details>
  )
}

function ActionRow({
  item,
  onOpenSession,
  onRecover,
  onStop,
  onComplete,
  onCopySessionId,
  onActionComplete,
}: {
  item: OrchestratorQueueItem
  onOpenSession?: (sessionId: string) => void
  onRecover?: (sessionId: string) => void | Promise<void>
  onStop?: (sessionId: string) => void | Promise<void>
  onComplete?: (sessionId: string) => void | Promise<void>
  onCopySessionId?: (sessionId: string) => void | Promise<void>
  onActionComplete?: () => void | Promise<void>
}) {
  if (!onOpenSession && !onRecover && !onStop && !onComplete && !onCopySessionId) return null

  const canRecover = item.recommendedAction === 'recover' && onRecover
  const canStop = (item.status === 'running' || item.status === 'waiting') && onStop
  const canComplete = item.status !== 'completed' && onComplete
  const openLabel = item.recommendedAction === 'review' ? 'Review Details' : 'Open Details'

  return (
    <div className={styles.actionRow} role="group" aria-label={`Actions for ${item.sessionId}`}>
      {onOpenSession && (
        <button
          type="button"
          className={`${styles.commandButton} ${styles.primaryCommand}`}
          onClick={() => onOpenSession(item.sessionId)}
        >
          {openLabel}
        </button>
      )}
      {canRecover && (
        <button
          type="button"
          className={`${styles.commandButton} ${styles.recoverCommand}`}
          onClick={() => void runAction(() => onRecover(item.sessionId), onActionComplete)}
        >
          Recover
        </button>
      )}
      {canStop && (
        <button
          type="button"
          className={`${styles.commandButton} ${styles.stopCommand}`}
          onClick={() => void runAction(() => onStop(item.sessionId), onActionComplete)}
        >
          Stop
        </button>
      )}
      {canComplete && (
        <button
          type="button"
          className={styles.commandButton}
          onClick={() => void runAction(() => onComplete(item.sessionId), onActionComplete)}
        >
          Complete
        </button>
      )}
      {onCopySessionId && (
        <button
          type="button"
          className={styles.commandButton}
          onClick={() => void runAction(() => onCopySessionId(item.sessionId))}
        >
          Copy ID
        </button>
      )}
    </div>
  )
}

async function runAction(
  action: () => void | Promise<void>,
  onActionComplete?: () => void | Promise<void>
) {
  try {
    await action()
    await onActionComplete?.()
  } catch (error) {
    console.error('Orchestrator action failed:', error)
  }
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

function getContextRows(item: OrchestratorQueueItem): Array<{ label: string; text: string }> {
  const seen = new Set<string>()
  const rows: Array<{ label: string; text: string }> = []

  pushContextRow(rows, seen, 'Last response', item.intent.evidence.lastMessage)
  pushContextRow(rows, seen, 'Prompt excerpt', item.intent.evidence.promptExcerpt)

  return rows.slice(0, 3)
}

function pushContextRow(
  rows: Array<{ label: string; text: string }>,
  seen: Set<string>,
  label: string,
  value: string | undefined
) {
  const text = value?.trim()
  if (!text || seen.has(text)) return
  seen.add(text)
  rows.push({ label, text })
}

function formatScopeText(hiddenOldLost: number, lostWindowHours?: number): string {
  if (hiddenOldLost === 0 || lostWindowHours == null) return ''
  return ` | ${hiddenOldLost} old lost hidden (>${lostWindowHours}h)`
}

function formatNoiseFlag(flag: string): string {
  return flag.replace(/_/g, ' ')
}

function formatIdentitySubtitle(item: OrchestratorQueueItem): string {
  const directory = formatPath(item.directory)
  const titleIsNoise = item.intent.noiseFlags.includes('instructions_heavy')
  if (!item.intent.task || !item.title || titleIsNoise || item.title === item.intent.task) {
    return directory
  }
  return `${directory} | ${item.title}`
}
