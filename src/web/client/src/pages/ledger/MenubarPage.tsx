import { useEffect, useRef, useState } from "react";
import type { LedgerDetail } from "../../../../../domain/ledger/types";
import { useMenubar } from "@/hooks/useMenubar";
import { openNativeLedger, quitNativeApp } from "./native";
import { projectName, rowPresentation } from "./presentation";
import styles from "./MenubarPage.module.css";

function SessionRow({ row, now, onOpen }: {
  row: LedgerDetail;
  now: number;
  onOpen: (row: LedgerDetail) => void;
}) {
  const p = rowPresentation(row, now);
  return (
    <li>
      <button className={`${styles.session} ${p.need ? styles.attention : ""}`}
        onClick={() => onOpen(row)} aria-label={`打开会话：${row.title}`}>
        <span className={styles.sessionHeading}>
          <span className={styles.sessionTitle} title={row.title}>{row.title}</span>
          <span className={styles.chevron} aria-hidden="true">›</span>
        </span>
        <span className={styles.description} title={p.now}>{p.now}</span>
        <span className={styles.meta}>
          <span className={styles.project} title={`${row.projectRoot} · ${p.agent} · ${p.activityTitle}`}>
            {projectName(row.projectRoot)} · {p.agent}
          </span>
          <span className={styles.tag} style={{ color: p.tagFg, background: p.tagBg }} title={p.tag}>
            {p.tag}
          </span>
        </span>
      </button>
    </li>
  );
}

export function MenubarPage({ token }: { token: string }) {
  const c = useMenubar(token);
  const [quitting, setQuitting] = useState(false);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState("");
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const quitButton = useRef<HTMLButtonElement>(null);
  const actionPending = useRef(false);
  const focusActive = !!c.settings?.focus.until && Date.parse(c.settings.focus.until) > c.now;
  const minutesLeft = focusActive ? Math.ceil((Date.parse(c.settings!.focus.until!) - c.now) / 60000) : 0;

  useEffect(() => {
    const quit = () => { setActionError(""); setQuitting(true); };
    window.addEventListener("keepline:quit", quit);
    return () => window.removeEventListener("keepline:quit", quit);
  }, []);
  useEffect(() => {
    if (quitting) { dialog.current?.showModal(); cancel.current?.focus(); }
    else if (dialog.current?.open) { dialog.current.close(); quitButton.current?.focus(); }
  }, [quitting]);

  const perform = async (action: () => Promise<unknown>) => {
    if (actionPending.current) return;
    actionPending.current = true;
    setBusy(true);
    setActionError("");
    try { await action(); }
    catch (error) { setActionError(error instanceof Error ? error.message : String(error)); }
    finally { actionPending.current = false; setBusy(false); }
  };
  const open = (row?: LedgerDetail) => void perform(() => openNativeLedger(row?.sessionId,
    row ? row.state === "needs_input" ? "current-step" : row.offPlan.length ? "off-plan" : row.state === "review" ? "requirements" : "current-step" : undefined));

  return (
    <main className={styles.panel} aria-label="Keepline 会话速览">
      <header className={styles.header}>
        <span className={styles.brandMark} aria-hidden="true">k<span>·</span></span>
        <div className={styles.brand}><h1>Keepline</h1><span>会话速览</span></div>
        <button className={styles.open} disabled={busy} onClick={() => open()}>
          打开主面板 <span aria-hidden="true">↗</span>
        </button>
      </header>

      <div className={styles.summary} aria-label="会话统计" aria-busy={c.loading}>
        <div><span className={styles.summaryLabel}><i className={styles.orangeDot} />需要你</span><strong className={styles.needsCount}>{c.loaded ? c.needs.length : "—"}</strong></div>
        <div><span className={styles.summaryLabel}><i className={styles.blueDot} />运行中</span><strong>{c.loaded ? c.running.length : "—"}</strong></div>
      </div>

      <div className={styles.content} tabIndex={0} aria-label="会话列表" aria-busy={c.loading}>
        {c.error && <div className={styles.notice} role="alert">
          <strong>{c.loaded ? "更新失败，保留上次记录" : "暂时无法读取会话"}</strong>
          <span>{c.error}</span><button disabled={c.loading} onClick={() => void c.refresh()}>重新加载</button>
        </div>}
        {c.nativeError && <div className={styles.notice} role="alert">托盘计数更新失败：{c.nativeError}</div>}
        {!c.loaded && c.loading ? <div className={styles.skeleton} role="status" aria-label="正在读取会话">
          <span>正在读取会话…</span>{[0, 1, 2].map(i => <div key={i}><i /><i /><i /></div>)}
        </div> : c.loaded && <>
          {!!c.needs.length && <section aria-labelledby="menubar-needs">
            <div className={styles.sectionHeading}><h2 id="menubar-needs">需要你处理</h2><span>等待响应优先</span></div>
            <ul className={styles.list}>{c.needs.map(row => <SessionRow key={row.sessionId} row={row} now={c.now} onOpen={open} />)}</ul>
          </section>}
          {!c.needs.length && !c.unconfirmed.length && <div className={`${styles.empty} ${c.running.length ? styles.calm : ""}`}>
            <span className={styles.emptyIcon} aria-hidden="true">✓</span>
            <h2>{c.running.length ? "暂时不用你处理" : "暂无待处理或运行中的会话"}</h2>
            <p>{c.running.length ? "Agent 正在推进，可以继续手头的事。" : "新的执行活动会显示在这里。历史记录可在主面板查看。"}</p>
          </div>}
          {!!c.running.length && <section aria-labelledby="menubar-running">
            <div className={styles.sectionHeading}><h2 id="menubar-running">正在执行 <span>{c.running.length}</span></h2><span>最近活动优先</span></div>
            <ul className={styles.list}>{c.running.map(row => <SessionRow key={row.sessionId} row={row} now={c.now} onOpen={open} />)}</ul>
          </section>}
          {!!c.unconfirmed.length && <p className={styles.unconfirmed}>{c.unconfirmed.length} 个会话的在线状态待确认，未计入运行中。<button onClick={() => open()} disabled={busy}>查看记录 ↗</button></p>}
        </>}
      </div>

      <footer className={styles.footer}>
        {actionError && !quitting && <div className={styles.notice} role="alert">{actionError}</div>}
        {c.settingsError && <div className={styles.settingsError} role="alert">无法读取专注设置 <button onClick={() => void c.refreshSettings()}>重试</button></div>}
        <button className={`${styles.focus} ${focusActive ? styles.focusActive : ""}`} aria-pressed={focusActive}
          disabled={busy || !c.settings || !!c.settingsError} onClick={() => void perform(c.toggleFocus)}>
          <span className={styles.moon} aria-hidden="true">☾</span>
          <span><strong>{focusActive ? `专注中 · 还剩 ${minutesLeft} 分钟` : c.settings ? `专注 ${c.settings.focus.minutes} 分钟` : "正在读取专注设置…"}</strong>
            <small>{focusActive ? "点击结束专注，恢复提醒" : "静音普通提醒，仍提醒你回答或审批"}</small></span>
          <span className={styles.switch} aria-hidden="true"><i /></span>
        </button>
        <div className={styles.footerLine}>
          <span title={c.lastSyncedAt ? `最近同步：${new Date(c.lastSyncedAt).toLocaleTimeString("zh-CN")}` : undefined}>
            <i className={c.error ? styles.orangeDot : c.connected ? styles.blueDot : styles.mutedDot} />
            {c.error ? "等待重新同步" : c.connected ? "实时更新" : "定时更新"} · 最近 24 小时
          </span>
          <button className={styles.quit} ref={quitButton} disabled={busy} onClick={() => { setActionError(""); setQuitting(true); }}>退出…</button>
        </div>
      </footer>

      <dialog className={styles.dialog} ref={dialog} aria-labelledby="menubar-quit-title" aria-describedby="menubar-quit-description"
        onCancel={event => { event.preventDefault(); if (!busy) setQuitting(false); }}
        onKeyDown={event => {
          if (event.key !== "Tab") return;
          const buttons = [...event.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
          const first = buttons[0], last = buttons.at(-1);
          if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
          else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
        }}>
        <h2 id="menubar-quit-title">退出 Keepline？</h2>
        <p id="menubar-quit-description">选择是否继续后台监控。已有的会话记录会保留。</p>
        {actionError && <p className={styles.notice} role="alert">{actionError}</p>}
        <button className={styles.keepMonitoring} disabled={busy} onClick={() => void perform(() => quitNativeApp(false))}>保持监控并退出</button>
        <button className={styles.stopMonitoring} disabled={busy} onClick={() => void perform(() => quitNativeApp(true))}>停止监控并退出</button>
        <button className={styles.cancel} ref={cancel} disabled={busy} onClick={() => setQuitting(false)}>取消</button>
      </dialog>
    </main>
  );
}
