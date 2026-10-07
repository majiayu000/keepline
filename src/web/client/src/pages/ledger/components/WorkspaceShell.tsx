import { useEffect, useRef, useState, type ReactNode } from "react";
import type { TabId } from "@/components/TabNav";
import { isNativeApp } from "../native";
import type { LedgerView } from "../types";
import { titles } from "../presentation";
import styles from "../Workspace.module.css";

export interface SidebarProject {
  root: string;
  name: string;
  count: number;
  need: number;
}
const navigation = [
  ["overview", "总览", "squares-four"],
  ["todos", "待办", "check-square"],
  ["goals", "目标", "target"],
  ["review", "回顾", "moon-stars"],
] as const;
const moreViews: ReadonlyArray<readonly [TabId, string]> = [
  ["sessions", "会话记录"],
  ["orchestrator", "Agent 任务板"],
  ["work", "工作项"],
  ["projects", "项目"],
  ["plans", "计划"],
  ["memory", "记忆"],
  ["analytics", "用量统计"],
];
export function WorkspaceShell({
  view,
  onNavigate,
  subtitle,
  search,
  onSearch,
  projects,
  project,
  onProject,
  needCount = 0,
  todoCount = 0,
  focusUntil,
  focusMinutes = 30,
  onFocus,
  onLogout,
  onRefresh,
  children,
}: {
  view: TabId;
  onNavigate: (view: TabId) => void;
  subtitle?: string;
  search?: string;
  onSearch?: (s: string) => void;
  projects?: SidebarProject[];
  project?: string | null;
  onProject?: (root: string | null) => void;
  needCount?: number;
  todoCount?: number;
  focusUntil?: string | null;
  focusMinutes?: number;
  onFocus?: () => void;
  onLogout?: () => void;
  onRefresh?: () => void;
  children: ReactNode;
}) {
  const [more, setMore] = useState(false);
  const [allProjects, setAllProjects] = useState(false);
  const [clock, setClock] = useState(new Date());
  const searchRef = useRef<HTMLInputElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const timer = window.setInterval(() => setClock(new Date()), 30000);
    return () => window.clearInterval(timer);
  }, []);
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (
        (e.target as HTMLElement).closest(
          'input,textarea,select,[contenteditable="true"]',
        )
      )
        return;
      if (e.key === "/") {
        e.preventDefault();
        searchRef.current?.focus();
      }
      if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        onNavigate("ledger-settings");
      }
      if (e.key === "Escape") setMore(false);
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onNavigate]);
  useEffect(() => {
    if (!more) return;
    const close = (e: PointerEvent) => {
      if (!menuRef.current?.contains(e.target as Node)) setMore(false);
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [more]);
  const focus = focusUntil && Date.parse(focusUntil) > clock.getTime();
  const hiddenProjectCount = (projects ?? []).filter(
    (p) => p.count === 0 && p.need === 0 && p.root !== project,
  ).length;
  const title =
    titles[view as LedgerView] ??
    moreViews.find(([id]) => id === view)?.[1] ??
    "Keepline";
  return (
    <div className={styles.workspace}>
      <nav className={styles.sidebar} aria-label="主导航">
        <div className={styles.brand}>
          <span />
          <strong>Keepline</strong>
          <small className={styles.version}>v0.4</small>
        </div>
        <div className={styles.navGroup}>
          {navigation.map(([id, label, icon]) => {
            const badge =
              id === "overview" ? needCount : id === "todos" ? todoCount : 0;
            return (
              <button
                key={id}
                onClick={() => onNavigate(id)}
                className={`${styles.navButton} ${view === id ? styles.selectedNav : ""}`}
                aria-label={label}
                aria-current={view === id ? "page" : undefined}
              >
                <i
                  aria-hidden="true"
                  className={`${styles.navIcon} ph-duotone ph-${icon}`}
                />
                <span className={styles.flex}>{label}</span>
                {badge > 0 && (
                  <span
                    className={`${styles.navBadge} ${id === "overview" ? styles.attentionBadge : ""}`}
                  >
                    {badge}
                  </span>
                )}
              </button>
            );
          })}
        </div>
        <div className={styles.projectSection}>
          <div className={`${styles.navLabel} ${styles.projectHeading}`}>
            <span>项目</span>
            {(allProjects || hiddenProjectCount > 0) && (
              <button
                className={styles.projectToggle}
                aria-label={
                  allProjects
                    ? "收起更多项目"
                    : `展开更多项目（${hiddenProjectCount} 个）`
                }
                aria-expanded={allProjects}
                onClick={() => setAllProjects(!allProjects)}
              >
                {allProjects ? "收起 ↑" : `更多 ${hiddenProjectCount} ↓`}
              </button>
            )}
          </div>
          <div className={`${styles.navGroup} ${styles.projectList}`}>
            {[
              {
                root: "",
                name: "全部项目",
                count: projects?.reduce((n, p) => n + p.count, 0) ?? 0,
                need: needCount,
              },
              ...(projects ?? []).filter(
                (p) =>
                  allProjects ||
                  p.count > 0 ||
                  p.need > 0 ||
                  p.root === project,
              ),
            ].map((p) => (
              <button
                key={p.root}
                className={`${styles.projectButton} ${(project ?? "") === p.root ? styles.selectedNav : ""}`}
                onClick={() => onProject?.(p.root || null)}
                title={p.root || "全部项目"}
              >
                <span className={styles.projectName}>
                  {p.name}
                  {p.root &&
                    (projects ?? []).filter((other) => other.name === p.name)
                      .length > 1 && (
                      <small className={styles.projectPath}>
                        {p.root
                          .split("/")
                          .filter(Boolean)
                          .slice(0, -1)
                          .slice(-2)
                          .join("/") || "/"}
                      </small>
                    )}
                </span>
                {p.need > 0 && (
                  <span className={styles.projectNeed}>▲ {p.need}</span>
                )}
                <span className={styles.mono}>{p.count}</span>
              </button>
            ))}
          </div>
        </div>
        <div className={styles.sidebarBottom}>
          <button
            className={styles.navButton}
            onClick={onFocus}
            disabled={!onFocus}
          >
            <i
              aria-hidden="true"
              className={`${styles.navIcon} ph-duotone ph-timer`}
            />
            <span className={styles.flex}>
              {focus
                ? `专注中 · 还剩 ${Math.ceil((Date.parse(focusUntil!) - clock.getTime()) / 60000)} 分钟`
                : `专注 ${focusMinutes} 分钟`}
            </span>
            <span
              className={styles.focusSwitch}
              style={{
                background: focus ? "#7C8CFF" : "rgba(255,255,255,.22)",
              }}
            >
              <i style={{ left: focus ? 16 : 2 }} />
            </span>
          </button>
          <div className={styles.more} ref={menuRef}>
            <button
              className={styles.navButton}
              onClick={() => setMore(!more)}
              aria-expanded={more}
            >
              <i
                aria-hidden="true"
                className={`${styles.navIcon} ph-duotone ph-dots-three-outline`}
              />
              <span className={styles.flex}>更多</span>
              <i
                aria-hidden="true"
                className={`${styles.moreArrow} ph-duotone ph-caret-down`}
              />
            </button>
            {more && (
              <div className={styles.moreMenu} role="menu">
                {moreViews
                  .filter(
                    ([id]) =>
                      !isNativeApp() ||
                      ![
                        "memory",
                        "plans",
                        "analytics",
                        "orchestrator",
                      ].includes(id),
                  )
                  .map(([id, label]) => (
                    <button
                      key={id}
                      role="menuitem"
                      onClick={() => {
                        setMore(false);
                        onNavigate(id);
                      }}
                    >
                      {label}
                    </button>
                  ))}
                {onRefresh && (
                  <button
                    role="menuitem"
                    onClick={() => {
                      setMore(false);
                      onRefresh();
                    }}
                  >
                    刷新数据
                  </button>
                )}
                {onLogout && (
                  <button role="menuitem" onClick={onLogout}>
                    退出登录
                  </button>
                )}
              </div>
            )}
          </div>
          <button
            className={`${styles.navButton} ${view === "ledger-settings" ? styles.selectedNav : ""}`}
            onClick={() => onNavigate("ledger-settings")}
            aria-label="设置"
            aria-current={view === "ledger-settings" ? "page" : undefined}
          >
            <i
              aria-hidden="true"
              className={`${styles.navIcon} ph-duotone ph-gear-six`}
            />
            <span className={styles.flex}>设置</span>
            <span className={`${styles.mono} ${styles.shortcut}`}>⌘,</span>
          </button>
        </div>
      </nav>
      <div className={styles.mainColumn}>
        <header className={styles.topbar}>
          <h1>{title}</h1>
          <span className={styles.subtitle}>{subtitle}</span>
          <span className={styles.headerRight}>
            {onSearch && (
              <span className={styles.searchField}>
                <i
                  aria-hidden="true"
                  className="ph-duotone ph-magnifying-glass"
                />
                <input
                  ref={searchRef}
                  value={search ?? ""}
                  onChange={(e) => onSearch(e.target.value)}
                  placeholder="搜索会话、原话、项目"
                  aria-label="搜索"
                />
                <kbd aria-hidden="true">/</kbd>
              </span>
            )}
            <span className={styles.clock}>
              {clock.toLocaleTimeString("zh-CN", {
                hour: "2-digit",
                minute: "2-digit",
                hour12: false,
              })}
            </span>
          </span>
        </header>
        {children}
      </div>
    </div>
  );
}
