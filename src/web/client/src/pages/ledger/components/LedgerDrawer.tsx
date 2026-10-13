import { useEffect, useRef } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import type { LedgerDetail } from "../../../../../../domain/ledger/types";
import type { RecoveryInfo } from "@/types";
import {
  rowPresentation,
  projectName,
  time,
  requirementStatus,
  stateText,
  itemColors,
} from "../presentation";
import { StatusTag, SubtaskTag, ActionButton } from "./LedgerPrimitives";
import styles from "../Workspace.module.css";
import { LedgerObservation } from "./LedgerObservation";
import { LedgerAskText } from "./LedgerAskText";

const markdownComponents: Components = {
  a: ({ href, children }) => (
    <a href={href} target="_blank" rel="noopener noreferrer">{children}</a>
  ),
  table: ({ children }) => (
    <div className={styles.markdownTable}><table>{children}</table></div>
  ),
};

export function LedgerDrawer({
  row,
  modal,
  onClose,
  onOpenSession,
  onFullDetail,
  onSelectSession,
  recovery,
  onCopyRecovery,
}: {
  row: LedgerDetail;
  modal: boolean;
  onClose: () => void;
  onOpenSession: () => void;
  onFullDetail: () => void;
  onSelectSession: (sessionId: string) => void;
  recovery: RecoveryInfo | null;
  onCopyRecovery: () => void;
}) {
  const p = rowPresentation(row);
  const ref = useRef<HTMLElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current
      ?.querySelector<HTMLButtonElement>('button[aria-label="关闭"]')
      ?.focus();
    return () => { if (previous?.isConnected) previous.focus(); };
  }, [row.sessionId]);
  return (
    <aside
      ref={ref}
      className={styles.drawer}
      role="dialog"
      aria-modal={modal}
      aria-label={`会话详情：${row.title}`}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <header className={styles.drawerHeader}>
        <div className={styles.drawerHeading}>
          <div className={styles.drawerEyebrow}>
            <StatusTag tag={p.tag} bg={p.tagBg} fg={p.tagFg} />
            {(row.isSubAgent || row.parentSessionId) && <SubtaskTag />}
            <span>{p.agent} · {projectName(row.projectRoot)} · 会话跨度 {p.elapsed}</span>
          </div>
          <h2>{row.title}</h2>
        </div>
        <button className={styles.close} aria-label="关闭" onClick={onClose}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
            <path d="m6 6 12 12M18 6 6 18" />
          </svg>
        </button>
      </header>
      <div className={styles.drawerBody}>
        {row.parentSessionId && (
          <div className={styles.drawerSection}>
            <ActionButton onClick={() => onSelectSession(row.parentSessionId!)}>查看父会话</ActionButton>
          </div>
        )}
        {!!row.subagents?.length && (
          <section className={styles.drawerSection} aria-label="子任务列表">
            <SubtaskTag count={row.subagents.length} />
            {row.subagents.map(child => (
              <button key={child.sessionId} className={styles.subtaskRow}
                aria-label={`查看子任务 ${child.title}`} onClick={() => onSelectSession(child.sessionId)}>
                <SubtaskTag />
                <span><strong>{child.title}</strong><small>{child.activity?.action ?? "暂无执行记录"}</small></span>
                <span>{stateText[child.state]}</span>
              </button>
            ))}
          </section>
        )}
        <div className={styles.drawerActivity}>
          <span className={styles.subtle}>现在</span>
          <span className={styles.drawerFactValue}>
            {p.now} · {p.lastAgo}
          </span>
          <span className={styles.subtle}>最近证据</span>
          <span>
            <span className={styles.evidence}>{p.evidence}</span>
          </span>
          <span className={styles.subtle}>它说</span>
          <ReactMarkdown
            className={`${styles.markdown} ${styles.drawerReport}`}
            remarkPlugins={[remarkGfm]}
            components={markdownComponents}
            skipHtml
          >
            {row.activity?.lastMessage ?? "暂无汇报"}
          </ReactMarkdown>
          <span className={styles.subtle}>上下文</span>
          <span style={{ color: p.context?.fg ?? "#6B7280" }}>
            {p.context?.detail ?? "未知"}
          </span>
        </div>
        <div className={styles.drawerSection}>
          <div>你发的消息 · 最新在前</div>
          {[...row.asks].reverse().map((ask) => (
            <div key={ask.id} className={styles.askRow}>
              <span className={styles.mono}>{time(ask.at)}</span>
              <LedgerAskText text={ask.text} />
            </div>
          ))}
        </div>
        <div className={styles.drawerSection}>
          <div>
            做到哪了 ·{" "}
            <span>{`${row.progress.done} / ${row.progress.total}`}</span>
          </div>
          {p.items.map((item) => (
            <div key={item.id} className={styles.requirementRow}>
              <span
                className={styles.square}
                style={{ background: itemColors[item.status] }}
              />
              <span>{item.title}</span>
              <span
                style={{
                  color:
                    item.status === "unverified"
                      ? "#B45309"
                      : item.status === "done"
                        ? "#4F5BD5"
                        : "#6B7280",
                  whiteSpace: "nowrap",
                }}
              >
                {item.status === "todo" ? "□" : "■"} {requirementStatus(item)}
              </span>
            </div>
          ))}
          {!p.items.length && (
            <div className={styles.subtle}>
              只有候选项，不判进度和偏离；见「执行观察」。
            </div>
          )}
        </div>
        {row.items.some((i) => i.constraints.length > 0) && (
          <div className={styles.drawerSection}>
            <div>约束 · 规则检查，不计入进度</div>
            {row.items
              .flatMap((i) => i.constraints)
              .map((cn, i) => (
                <div key={i} className={styles.requirementRow}>
                  <span
                    className={styles.square}
                    style={{
                      border: "1px solid #6B7280",
                      boxSizing: "border-box",
                    }}
                  />
                  <span>
                    {cn.kind === "path_forbidden"
                      ? "禁止修改"
                      : cn.kind === "preserve_text"
                        ? "保留文字"
                        : "不改公共接口"}{" "}
                    {cn.value}
                  </span>
                  <span className={styles.subtle}>见执行轨迹</span>
                </div>
              ))}
          </div>
        )}
        {recovery?.canRecover && recovery.command && (
          <div className={styles.drawerSection}>
            <div>进程已不在，会话文件还在 · 可恢复</div>
            <pre className={styles.evidence} style={{ whiteSpace: "pre-wrap" }}>
              {recovery.command}
            </pre>
            <ActionButton onClick={onCopyRecovery}>复制恢复命令</ActionButton>
          </div>
        )}
        <LedgerObservation row={row} />
      </div>
      <div className={styles.drawerActions}>
        <ActionButton primary onClick={onOpenSession}>
          查看会话记录
        </ActionButton>
        <ActionButton onClick={onFullDetail}>完整进度账 →</ActionButton>
      </div>
    </aside>
  );
}
