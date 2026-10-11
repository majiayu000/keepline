import { useEffect, useRef } from "react";
import type { LedgerDetail } from "../../../../../../domain/ledger/types";
import type { RecoveryInfo } from "@/types";
import {
  rowPresentation,
  projectName,
  time,
  requirementStatus,
  itemColors,
} from "../presentation";
import { StatusTag, ActionButton } from "./LedgerPrimitives";
import styles from "../Workspace.module.css";

export function LedgerDrawer({
  row,
  onClose,
  onOpenSession,
  onFullDetail,
  recovery,
  onCopyRecovery,
}: {
  row: LedgerDetail;
  onClose: () => void;
  onOpenSession: () => void;
  onFullDetail: () => void;
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
    return () => previous?.focus();
  }, [row.sessionId]);
  return (
    <aside
      ref={ref}
      className={styles.drawer}
      role="dialog"
      aria-modal="false"
      aria-label={`会话详情：${row.title}`}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <div className={styles.drawerHeader}>
        <StatusTag tag={p.tag} bg={p.tagBg} fg={p.tagFg} />
        <span>
          {p.agent} · {projectName(row.projectRoot)} · 会话跨度 {p.elapsed}
        </span>
        <button className={styles.close} aria-label="关闭" onClick={onClose}>
          <i aria-hidden="true" className="ph-duotone ph-x" />
        </button>
      </div>
      <h2>{row.title}</h2>
      <div className={styles.drawerActivity}>
        <span className={styles.subtle}>现在</span>
        <span>
          {p.now} · {p.lastAgo}
        </span>
        <span className={styles.subtle}>最近证据</span>
        <span>
          <span className={styles.evidence}>{p.evidence}</span>
        </span>
        <span className={styles.subtle}>它说</span>
        <span className={styles.subtle}>
          “{row.activity?.lastMessage ?? "暂无汇报"}”
        </span>
        <span className={styles.subtle}>上下文</span>
        <span className={styles.subtle}>未知</span>
      </div>
      <div className={styles.drawerSection}>
        <div>你发了什么</div>
        {row.asks.map((ask) => (
          <div key={ask.id} className={styles.askRow}>
            <span className={styles.mono}>{time(ask.at)}</span>
            <span>{ask.text}</span>
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
            只有候选项，不判进度和偏离；看「现在」这一行。
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
      <div className={styles.drawerActions}>
        <ActionButton primary onClick={onOpenSession}>
          {recovery?.canRecover ? "在终端里继续" : "跳到 agent"}
        </ActionButton>
        <ActionButton onClick={onFullDetail}>完整进度账 →</ActionButton>
      </div>
    </aside>
  );
}
