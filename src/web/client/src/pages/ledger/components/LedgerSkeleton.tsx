import { Skeleton } from "@/components/Skeleton";
import styles from "../Workspace.module.css";

export function LedgerOverviewSkeleton() {
  return (
    <div className={styles.ledgerSkeleton} role="status" aria-label="正在加载会话" aria-busy="true">
      <div className={styles.monitorStatus}><Skeleton width={150} height={12} /></div>
      <div className={styles.overviewToolbar} aria-hidden="true">
        {[180, 120, 170, 230].map(width => <Skeleton key={width} width={width} height={32} variant="rect" />)}
      </div>
      <div className={styles.overviewGroups} aria-hidden="true">
        {[4, 3].map((count, group) => (
          <div key={group} className={styles.overviewGroup}>
            <div className={styles.skeletonGroupHeading}>
              <Skeleton width={group === 0 ? 72 : 56} height={14} />
              <Skeleton width={22} height={18} />
            </div>
            <div className={styles.cards}>
              {Array.from({ length: count }, (_, index) => (
                <div key={index} className={styles.skeletonCard}>
                  <div className={styles.skeletonRow}><Skeleton width={58} height={20} /><Skeleton width={48} height={12} /></div>
                  <Skeleton width={index % 2 ? "68%" : "86%"} height={17} />
                  <Skeleton width="92%" height={12} />
                  <Skeleton width="66%" height={12} />
                  <Skeleton width="100%" height={30} variant="rect" />
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function LedgerDrawerSkeleton({ onClose, modal, error, onRetry, busy }: { onClose: () => void; modal: boolean; error?: string; onRetry?: () => void; busy?: boolean }) {
  return (
    <aside className={`${styles.drawer} ${error ? "" : styles.ledgerSkeleton}`} role="dialog" aria-modal={modal} aria-label={error ? "会话详情加载失败" : "正在加载会话详情"} aria-busy={!error || busy}>
      <header className={styles.drawerHeader}>
        {error ? <h2>无法打开会话详情</h2> : <div className={styles.drawerHeading} aria-hidden="true">
          <div className={styles.drawerEyebrow}><Skeleton width={56} height={22} /><Skeleton width={92} height={12} /></div>
          <div className={styles.skeletonTitle}><Skeleton width="88%" height={24} /></div>
        </div>}
        <button className={styles.close} aria-label="关闭" onClick={onClose}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" /></svg>
        </button>
      </header>
      {error ? <div className={styles.drawerBody}><p role="alert">{error}</p><button className={styles.actionButton} disabled={busy} onClick={onRetry}>{busy ? "正在重试…" : "重试"}</button></div> : <><div className={styles.drawerBody} aria-hidden="true">
        <div className={styles.drawerActivity}>
          <Skeleton width={48} height={12} /><Skeleton width="80%" height={12} />
          <Skeleton width={48} height={12} /><Skeleton width="65%" height={18} />
          <Skeleton width={32} height={12} />
          <div className={`${styles.drawerReport} ${styles.skeletonParagraph}`}>
            {["96%", "100%", "58%"].map((width, i) => <Skeleton key={i} width={width} height={12} />)}
          </div>
          <Skeleton width={48} height={12} /><Skeleton width={40} height={12} />
        </div>
        <div className={styles.drawerSection}>
          <div><Skeleton width={160} height={12} /></div>
          {[0, 1, 2].map(index => (
            <div key={index} className={styles.askRow}>
              <Skeleton width={32} height={11} />
              <div className={styles.skeletonParagraph}><Skeleton width="96%" height={12} /><Skeleton width={index === 0 ? "76%" : "52%"} height={12} /></div>
            </div>
          ))}
        </div>
      </div>
      <div className={styles.drawerActions} aria-hidden="true">
        <Skeleton width="48%" height={40} variant="rect" /><Skeleton width="48%" height={40} variant="rect" />
      </div>
      </>}
    </aside>
  );
}
