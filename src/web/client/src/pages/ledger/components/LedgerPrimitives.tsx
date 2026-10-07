import type { ReactNode } from "react";
import type { RequirementItem } from "../../../../../../domain/ledger/types";
import { itemColors } from "../presentation";
import styles from "../Workspace.module.css";

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: ReadonlyArray<readonly [T, string]>;
  onChange: (value: T) => void;
  label: string;
}) {
  return (
    <span className={styles.segmented} role="group" aria-label={label}>
      {options.map(([id, text]) => (
        <button
          key={id}
          aria-pressed={value === id}
          className={value === id ? styles.current : undefined}
          onClick={() => onChange(id)}
        >
          {text}
        </button>
      ))}
    </span>
  );
}
export function ProgressSegments({
  items,
  stretch = false,
}: {
  items: Array<Pick<RequirementItem, "status">>;
  stretch?: boolean;
}) {
  return (
    <span
      className={`${styles.progressSegments} ${stretch ? styles.stretch : ""}`}
      aria-label={
        items.length
          ? `已完成 ${items.filter((i) => i.status === "done").length} / ${items.length} 项`
          : "暂无已确认要求"
      }
    >
      {items.map((item, i) => (
        <i key={i} style={{ background: itemColors[item.status] }} />
      ))}
    </span>
  );
}
export function StatusTag({
  tag,
  bg,
  fg,
}: {
  tag: string;
  bg: string;
  fg: string;
}) {
  return (
    <span className={styles.tag} style={{ background: bg, color: fg }}>
      {tag}
    </span>
  );
}
export function ActionButton({
  children,
  onClick,
  primary = false,
  disabled = false,
  className = "",
}: {
  children: ReactNode;
  onClick: () => void;
  primary?: boolean;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <button
      className={`${styles.actionButton} ${primary ? styles.primary : ""} ${className}`}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
export function EmptyState({ children }: { children: ReactNode }) {
  return (
    <div className={styles.empty} role="status">
      {children}
    </div>
  );
}
