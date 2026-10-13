import type {
  LedgerDetail,
  LedgerEvidence,
  LedgerStep,
  RequirementItem,
  LedgerMonitorStatus,
} from "../../../../../domain/ledger/types";
import type { WorkItem } from "../../types/work-item";
import {
  confirmedRequirement,
  ledgerNeedsAttention,
  ledgerAttentionKind,
} from "../../../../../domain/ledger/types";

export const stateText = {
  running: "运行中",
  needs_input: "等你处理",
  review: "待核对",
  accepted: "已验收",
  stopped: "已停下",
  ended: "已结束",
};
export const itemStateText = {
  todo: "未开始",
  doing: "正在做",
  done: "有证据",
  unverified: "未验证",
};
export const sourceText = {
  fallback: "候选",
  work_item: "验收清单",
  model: "AI 识别",
  user: "你确认的",
};
export const itemColors = {
  done: "#4F5BD5",
  doing: "#B4BAF0",
  unverified: "#F59E0B",
  todo: "#E6E8EC",
};
export const titles = {
  overview: "总览",
  todos: "待办",
  goals: "目标",
  review: "回顾",
  "ledger-settings": "设置",
};
export function elapsed(at: string, end = Date.now()) {
  const minutes = Math.max(0, Math.floor((end - Date.parse(at)) / 60000));
  if (minutes < 60) return `${minutes} 分钟`;
  const rest = minutes % 60;
  return `${Math.floor(minutes / 60)} 小时${rest ? ` ${rest} 分` : ""}`;
}
export function time(at: string) {
  return new Date(at).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}
export function projectName(root: string) {
  return root.split("/").filter(Boolean).at(-1) || "未关联项目";
}
function contextTokens(tokens: number) {
  return new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 }).format(tokens).replace("K", "k");
}
export function evidenceText(row: LedgerDetail) {
  const e = row.activity?.evidence[0] ?? row.evidence.at(-1);
  return e
    ? `${e.value}${e.exitCode === undefined ? "" : e.exitCode === 0 ? " · 成功" : ` · exit ${e.exitCode}`}`
    : "暂无执行证据";
}
export function evidenceSummary(e: LedgerEvidence) {
  const value = e.kind === "file" ? projectName(e.value) : e.value;
  return e.exitCode === undefined
    ? value
    : e.exitCode === 0
      ? `${value} · 成功`
      : `本次执行失败（exit ${e.exitCode}） · ${value}`;
}
export function reviewAcceptanceAt(row: LedgerDetail, start: string, end: string) {
  return row.acceptances.filter(acceptance => acceptance.decision !== "follow_up" &&
    Date.parse(acceptance.at) >= Date.parse(start) && Date.parse(acceptance.at) < Date.parse(end))
    .sort((a, b) => Date.parse(b.at) - Date.parse(a.at))[0]?.at;
}
export function keyStep(step: LedgerStep, evidence: LedgerEvidence[]) {
  return (
    !!step.itemId ||
    !!step.violations.length ||
    step.acceptedOffPlan ||
    evidence.some(
      (e) =>
        step.evidenceIds.includes(e.id) &&
        (e.kind !== "command" ||
          (e.exitCode !== undefined && e.exitCode !== 0)),
    )
  );
}
export function rowPresentation(row: LedgerDetail, now = Date.now()) {
  const need = ledgerNeedsAttention(row);
  const highlight = need;
  const online = row.processState === "online";
  const exited = row.processState === "exited";
  const unknown = !online && !exited;
  const paused = row.state === "stopped" && !exited;
  const ended = row.state === "ended" || row.state === "accepted";
  const historical = exited || ended;
  const workTag =
    row.state === "needs_input"
      ? `▲ 等你处理 · ${elapsed(row.inputRequest?.at ?? row.observation?.at ?? row.lastActiveAt, now)}`
      : row.state === "running" && row.offPlan.length
        ? "▲ 偏离"
        : row.state === "review"
          ? row.progress.total === 0
            ? "◐ 新回复"
            : row.progress.done < row.progress.total
            ? "▲ 待核对 · 有缺口"
            : "◐ 待核对"
          : row.state === "accepted"
            ? "✓ 已验收"
            : paused
              ? "○ 停下了"
              : ended
                ? "已结束"
                : "● 运行中";
  const tag = exited ? `已退出${row.unread ? " · 新回复" : row.state === "review" ? " · 待核对" : ""}`
    : unknown ? `无法确认在线 · ${row.state === "running" ? "未见本轮结束" : workTag}`
    : workTag;
  const first = row.asks[0]?.at ?? row.turns[0]?.at ?? row.lastActiveAt;
  const duration = Math.max(
    0,
    Math.floor(
      ((historical ? Date.parse(row.lastActiveAt) : now) - Date.parse(first)) /
        60000,
    ),
  );
  const latestEvidence = row.activity?.evidence[0] ?? row.evidence.at(-1);
  const activityMinutes = Math.max(0, Math.floor((now - Date.parse(row.lastActiveAt)) / 60000));
  const pendingTool = row.state === "running" && row.activity?.completed === false && row.activity.turnId === row.turnId;
  const observationLabel = row.observation?.source === "hook" ? "实时事件"
    : row.observation?.source === "transcript" ? "执行记录"
    : row.observation?.source === "user" ? "你的确认" : "推测";
  const activeAgo = activityMinutes < 1
    ? "刚刚"
    : activityMinutes < 60
      ? `${activityMinutes} 分钟前`
      : `${Math.floor(activityMinutes / 60)} 小时前`;
  const contextPercent = row.contextUsage
    ? Math.round(row.contextUsage.usedTokens / row.contextUsage.windowTokens * 100)
    : undefined;
  return {
    need,
    highlight,
    paused,
    ended,
    online,
    exited,
    unknown,
    historical,
    tag,
    observationLabel,
    observationTitle: row.observation ? `${observationLabel}：${row.observation.reason} · ${new Date(row.observation.at).toLocaleString("zh-CN", { hour12: false })}` : "尚无可解释的状态依据",
    agent:
      row.runtimeId === "codex"
        ? "Codex"
        : row.runtimeId === "claude-code"
          ? "Claude Code"
          : row.runtimeId,
    elapsed: elapsed(first, historical ? Date.parse(row.lastActiveAt) : now),
    compactElapsed:
      duration < 60
        ? `${duration}m`
        : `${Math.floor(duration / 60)}h${duration % 60 ? ` ${duration % 60}m` : ""}`,
    activeAgo,
    activityTitle: `最后活动：${new Date(row.lastActiveAt).toLocaleString("zh-CN", { hour12: false })}`,
    context: contextPercent === undefined ? undefined : {
      percent: contextPercent,
      text: `${contextPercent}%`,
      tooltip: `上下文 ${contextTokens(row.contextUsage!.usedTokens)} / ${contextTokens(row.contextUsage!.windowTokens)} (${contextPercent}%)`,
      detail: `${contextPercent}% 已用${contextPercent >= 90 ? " · 接近上限，长任务可能被截断" : contextPercent >= 70 ? " · 偏高" : ""}`,
      fg: contextPercent >= 90 ? "#B45309" : "#6B7280",
      bar: contextPercent >= 90 ? itemColors.unverified : contextPercent >= 70 ? itemColors.doing : itemColors.done,
    },
    lastAgo: activityMinutes < 1 ? "刚刚" : `${elapsed(row.lastActiveAt, now)}前`,
    tagBg: highlight
      ? "#FFF1DB"
      : online && row.state === "running"
        ? "#EEF0FD"
        : row.state === "accepted"
          ? "#4F5BD5"
          : "#EEF0F3",
    tagFg: highlight
      ? "#B45309"
      : online && row.state === "running"
        ? "#4F5BD5"
        : row.state === "accepted"
          ? "#fff"
          : "#4B5160",
    action:
      row.state === "needs_input"
        ? "查看待响应会话"
        : row.state === "review"
          ? row.progress.total === 0
            ? "查看回复"
            : row.progress.done < row.progress.total
            ? "生成追问"
            : "验收 (A)"
          : row.offPlan.length && !ended
            ? "看偏离"
            : null,
    now: exited && row.state !== "review" ? "已确认进程退出，执行记录已保留" : row.state === "needs_input"
      ? row.inputRequest?.question ?? row.statusReason ?? "等待你的审批或补充信息"
      : row.activity?.action
        ? pendingTool ? `工具已发起，尚未观察到结果：${row.activity.action} · ${elapsed(row.activity.at ?? row.lastActiveAt, now)}`
          : `上次动作：${row.activity.action}${row.activity.exitCode !== undefined ? ` · exit ${row.activity.exitCode}` : ""}`
        :
      (row.state === "review"
        ? row.progress.total === 0 ? "这一轮已回复，等你查看" : "这一轮已结束，等你核对"
        : row.possiblyWaiting ? "可能在等待，暂无明确输入请求" : "暂未观察到新的执行活动"),
    items: row.items.filter(confirmedRequirement),
    evidence: evidenceText(row),
    evidenceSummary: latestEvidence
      ? evidenceSummary(latestEvidence)
      : "暂无执行证据",
    ask: row.asks[0]?.text ?? "",
  };
}
export function attentionSummary(rows: Array<Pick<LedgerDetail, 'state' | 'processState' | 'offPlan' | 'progress'>>, replyLabel = '条新回复') {
  const kinds = rows.map(ledgerAttentionKind);
  return ([['input', '个等你回答或审批'], ['reply', replyLabel], ['acceptance', '个待验收'], ['deviation', '个偏离待核对']] as const)
    .map(([kind, label]) => { const count = kinds.filter(value => value === kind).length; return count ? `${count} ${label}` : ''; })
    .filter(Boolean).join(' · ') || '暂无待处理事项';
}
export function monitorPresentation(status: LedgerMonitorStatus, now = Date.now()) {
  const scan = status.scan;
  const at = scan.lastCompletedAt ?? status.runtimes.map(runtime => runtime.lastScanAt).filter((at): at is string => !!at).sort().at(-1);
  const stale = !!at && now - Date.parse(at) > (scan.intervalMs ?? 60000) + (scan.timeoutMs ?? 30000) + 15000;
  const warning = scan.failed || stale || scan.intervalMs === 0 || !at && !scan.running || status.runtimes.some(runtime => runtime.errorCount > 0);
  const text = scan.failed ? '扫描失败，记录可能不完整'
    : stale ? `数据已过期 · 最近扫描 ${elapsed(at!, now)}前`
    : !at ? scan.running ? '首次扫描中，尚不能确认待处理数量' : '尚未观察到扫描，不能确认待处理数量'
    : scan.intervalMs === 0 ? `定时扫描已关闭 · 最近扫描 ${elapsed(at, now)}前`
    : `最近扫描 ${now - Date.parse(at) < 60000 ? '刚刚' : `${elapsed(at, now)}前`}${scan.running ? ' · 正在更新' : ''}${status.runtimes.some(runtime => runtime.errorCount > 0) ? ' · 有记录读取失败' : ''}`;
  return { warning, text };
}
export type RowPresentation = ReturnType<typeof rowPresentation>;
export function groupRows(
  rows: LedgerDetail[],
  grouping: "recent" | "urgency" | "project" | "goal",
  now = Date.now(),
  goalContext?: { todos: Pick<WorkItem, "id" | "parentId">[]; goals: Pick<WorkItem, "id" | "title">[] },
) {
  const latestFirst = (a: LedgerDetail, b: LedgerDetail) =>
    Date.parse(b.lastActiveAt) - Date.parse(a.lastActiveAt);
  if (grouping === "recent")
    return [{
      key: "recent",
      label: "最近活动",
      hint: "最新在前 · 包含历史会话",
      rows: [...rows].sort(latestFirst),
    }];
  if (grouping === "goal") {
    const todoGoals = new Map(goalContext?.todos.map(todo => [todo.id, todo.parentId]));
    const goals = new Map(goalContext?.goals.map(goal => [goal.id, goal.title]));
    const goalId = (row: LedgerDetail) => {
      const id = row.workItemId ? todoGoals.get(row.workItemId) : undefined;
      return id && goals.has(id) ? id : "unassigned";
    };
    return [...new Set(rows.map(goalId))].map(id => {
      const grouped = rows.filter(row => goalId(row) === id);
      return { key: id, label: goals.get(id) ?? "未关联目标",
        hint: `${grouped.filter(row => rowPresentation(row, now).need).length} 需要你`,
        rows: grouped.sort((a, b) => Number(rowPresentation(b, now).need) - Number(rowPresentation(a, now).need)) };
    });
  }
  if (grouping === "project")
    return [...new Set([...rows].sort(latestFirst).map((r) => r.projectRoot))].map((root) => ({
      key: root,
      label: projectName(root),
      hint: `${rows.filter((r) => r.projectRoot === root && rowPresentation(r, now).need).length} 需要你`,
      rows: rows
        .filter((r) => r.projectRoot === root)
        .sort(latestFirst),
    }));
  return [
    {
      key: "need",
      label: "需要你",
      hint: "最新活动在前",
      rows: rows
        .filter((r) => { const p = rowPresentation(r, now); return p.need && !p.historical; })
        .sort(
          (a, b) => Number(b.state === "needs_input") - Number(a.state === "needs_input") || latestFirst(a, b),
        ),
    },
    {
      key: "running",
      label: "运行中",
      hint: "按最后动作排序",
      rows: rows
        .filter((r) => {
          const p = rowPresentation(r, now);
          return p.online && !p.need && !p.paused && !p.historical;
        })
        .sort(
          (a, b) => Date.parse(b.lastActiveAt) - Date.parse(a.lastActiveAt),
        ),
    },
    {
      key: "ended",
      label: "历史会话",
      hint: "按最后活动排序",
      rows: rows
        .filter((r) => rowPresentation(r, now).historical)
        .sort(
          (a, b) => Date.parse(b.lastActiveAt) - Date.parse(a.lastActiveAt),
        ),
    },
    {
      key: "paused",
      label: "停下了",
      hint: "本轮中断或暂未继续",
      rows: rows.filter((r) => rowPresentation(r, now).paused).sort(latestFirst),
    },
    {
      key: "unknown", label: "在线状态待确认", hint: "保留记录，不计入在线数量",
      rows: rows.filter(r => { const p = rowPresentation(r, now); return p.unknown && !p.need && !p.paused && !p.historical; }).sort(latestFirst),
    },
  ].filter((g) => g.rows.length || !["paused", "unknown"].includes(g.key));
}
export function requirementStatus(item: RequirementItem) {
  return item.dropped
    ? "已放弃"
    : item.source === "fallback"
      ? "待你确认 · 不计进度"
      : itemStateText[item.status];
}
