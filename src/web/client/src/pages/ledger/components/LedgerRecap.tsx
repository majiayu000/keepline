import type { LedgerDetail, LedgerEvidence } from "../../../../../../domain/ledger/types";
import { rowPresentation, time } from "../presentation";
import styles from "../Workspace.module.css";

/** A projection of recorded facts; no requirement matching or completion judgment. */
export function LedgerRecap({ row }: { row: LedgerDetail }) {
  const p = rowPresentation(row);
  const since = row.changes?.since;
  const records = since ? [...row.changes!.checks, ...row.changes!.files] : row.evidence;
  const steps = since ? row.changes!.steps : row.trail;
  const stepsByCall = new Map(steps.map(step => [step.callId, step]));
  const checks = new Map<string, LedgerEvidence[]>();
  const files = new Map<string, LedgerEvidence>();
  for (const evidence of [...records].reverse()) {
    if (evidence.kind === "command" || evidence.kind === "test") {
      checks.set(evidence.callId, [...(checks.get(evidence.callId) ?? []), evidence]);
    } else if (evidence.kind === "file" && !files.has(evidence.value)) {
      files.set(evidence.value, evidence);
    }
  }
  // A recorded tool call is still activity when it has no classified evidence.
  // Do not duplicate calls already represented by a command, test, or file.
  const representedCalls = new Set(records.filter(e => e.kind === "command" || e.kind === "test" || e.kind === "file").map(e => e.callId));
  for (const step of steps) if (!representedCalls.has(step.callId)) checks.set(step.callId, []);
  const recordedAt = (callId: string, evidence: LedgerEvidence[]) => evidence[0]?.at ?? stepsByCall.get(callId)!.at;
  const orderedChecks = [...checks.entries()].sort(([aId, a], [bId, b]) => Date.parse(recordedAt(bId, b)) - Date.parse(recordedAt(aId, a)));
  const orderedFiles = [...files.values()].sort((a, b) => Date.parse(b.at) - Date.parse(a.at));
  const hasRequest = row.state === "needs_input" && row.processState !== "exited";
  return (
    <section className={styles.recap} aria-label="最近情况">
      <div className={styles.recapQuestion} data-pending={hasRequest || undefined}>
        <h3>需要你回答</h3>
        <p>{hasRequest ? row.inputRequest?.question ?? row.statusReason ?? "有明确输入请求，请查看会话记录。" : "暂无明确待回答的问题"}</p>
        {hasRequest && row.inputRequest && <small>请求记录于 {time(row.inputRequest.at)} · {p.observationLabel}</small>}
      </div>
      <div className={styles.recapRange}>
        {since ? <>自上次查看后 · {new Date(since).toLocaleString("zh-CN", { hour12: false })} 起</>
          : "首次查看，尚无上次查看的时间基准；以下为已记录的执行与文件变化。"}
      </div>
      <section aria-label="最近检查记录">
        <h3>执行与检查 <span>{checks.size}</span></h3>
        {!checks.size && <p className={styles.subtle}>{since ? "这段时间没有新的执行结果记录。" : "尚无执行结果记录。"}</p>}
        {orderedChecks.map(([callId, evidence]) => {
          const command = evidence.find(e => e.kind === "command");
          const failed = evidence.find(e => e.exitCode !== undefined && e.exitCode !== 0);
          const code = failed?.exitCode ?? command?.exitCode ?? evidence.find(e => e.exitCode !== undefined)?.exitCode;
          const step = stepsByCall.get(callId) ?? row.trail.find(s => s.callId === callId);
          return <details className={styles.recapRecord} key={callId}>
            <summary>
              <span>{command?.value ?? step?.summary ?? evidence[0]?.value ?? callId}</span>
              <strong data-failed={code !== undefined && code !== 0 || undefined}>{code === undefined ? "未收到结果" : code === 0 ? "执行成功" : `执行失败 · exit ${code}`}</strong>
            </summary>
            <p>记录于 {new Date(recordedAt(callId, evidence)).toLocaleString("zh-CN", { hour12: false })} · 调用 {callId}</p>
            {evidence.filter(e => e.kind === "test").map(e => <pre key={e.id}>{e.value}</pre>)}
            {step && <pre>{step.summary}</pre>}
          </details>;
        })}
        {!!checks.size && <p className={styles.recapNote}>记录表示当时的执行结果；之后的修改可能需要重新检查。</p>}
      </section>
      <section aria-label="最近文件变化">
        <h3>文件变化 <span>{files.size}</span></h3>
        {!files.size && <p className={styles.subtle}>{since ? "这段时间没有新的文件修改记录。" : "尚无文件修改记录。"}</p>}
        {orderedFiles.map(e => <details className={styles.recapRecord} key={e.value}>
          <summary><span className={styles.mono}>{e.value}</span></summary>
          <p>记录于 {new Date(e.at).toLocaleString("zh-CN", { hour12: false })} · 调用 {e.callId}</p>
          {e.exitCode !== undefined && e.exitCode !== 0 && <p>本次执行失败 · exit {e.exitCode}</p>}
          <pre>{row.trail.find(step => step.callId === e.callId)?.summary ?? e.value}</pre>
        </details>)}
      </section>
      <details className={styles.recapRecord}>
        <summary>状态依据 · {p.observationLabel}</summary>
        <p>{p.now}</p>
        <p>{row.observation?.reason ?? "尚无可解释的状态依据"}</p>
        <p>{p.online ? "已匹配到存活进程；不代表本轮仍在执行。" : p.exited ? "此前绑定的进程已退出；不代表工作已完成。" : "当前没有足够的进程证据确认在线或退出。"}</p>
        {row.observation && <p>{new Date(row.observation.at).toLocaleString("zh-CN", { hour12: false })}</p>}
      </details>
    </section>
  );
}
