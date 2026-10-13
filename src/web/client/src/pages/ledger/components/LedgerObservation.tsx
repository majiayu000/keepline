import type { LedgerDetail } from "../../../../../../domain/ledger/types";
import { rowPresentation } from "../presentation";
import styles from "../Workspace.module.css";

export function LedgerObservation({ row }: { row: LedgerDetail }) {
  const p = rowPresentation(row);
  const changes = row.changes;
  return <div className={styles.changes}>
    <details>
      <summary>状态依据 · {p.observationLabel}</summary>
      <p>{p.online ? "已匹配到存活进程；不代表本轮仍在执行。" : p.exited ? "此前绑定的进程已退出；不代表工作已完成。" : "当前没有足够的进程证据确认在线或退出。"}</p>
      <p>{row.observation?.reason ?? "尚无可解释的状态依据"}</p>
      {row.observation && <p>{new Date(row.observation.at).toLocaleString("zh-CN", { hour12: false })}</p>}
      {row.inputRequest?.question && <p>等待回答：{row.inputRequest.question}</p>}
      {row.activity?.action && <p>{p.now}</p>}
    </details>
    {changes?.since ? <details open>
      <summary>自上次查看后 · {new Set(changes.files.map(e => e.value)).size} 处文件 · {changes.checks.filter(e => e.kind === "test").length} 条测试结果 · {changes.steps.length} 次操作</summary>
      <p>自 {new Date(changes.since).toLocaleString("zh-CN", { hour12: false })} 起；以下为执行记录中的证据。</p>
      {changes.files.length === 0 && changes.checks.length === 0 && <p>暂无新的文件或检查证据。</p>}
      {[...changes.files, ...changes.checks].map(e => <details key={e.id}>
        <summary>{e.value}{e.exitCode === undefined ? "" : e.exitCode === 0 ? " · 成功" : ` · 失败（exit ${e.exitCode}）`}</summary>
        <p>记录于 {new Date(e.at).toLocaleString("zh-CN", { hour12: false })} · 调用 {e.callId}</p>
        <pre>{row.trail.find(step => step.callId === e.callId)?.summary ?? e.value}</pre>
      </details>)}
    </details> : <p className={styles.subtle}>首次查看，尚无上次查看的时间基准。</p>}
  </div>;
}
