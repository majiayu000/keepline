import type {
  LedgerDetail,
  LedgerEvidence,
  LedgerStep,
  RequirementItem,
} from "../../../../../domain/ledger/types";
import {
  confirmedRequirement,
  ledgerNeedsAttention,
} from "../../../../../domain/ledger/types";

export const stateText = {
  running: "运行中",
  needs_input: "等你审批",
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
export function evidenceText(row: LedgerDetail) {
  const e = row.activity?.evidence.at(-1) ?? row.evidence.at(-1);
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
  // Attention determines the queue; only missing evidence or intervention uses the orange design treatment.
  const highlight =
    row.state === "needs_input" ||
    (row.state === "running" && row.offPlan.length > 0) ||
    (row.state === "review" && row.progress.done < row.progress.total);
  const paused = row.state === "stopped";
  const ended = row.state === "ended" || row.state === "accepted";
  const tag =
    row.state === "needs_input"
      ? `▲ 等你回答 · ${elapsed(row.lastActiveAt, now)}`
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
  const first = row.asks[0]?.at ?? row.turns[0]?.at ?? row.lastActiveAt;
  const duration = Math.max(
    0,
    Math.floor(
      ((ended ? Date.parse(row.lastActiveAt) : now) - Date.parse(first)) /
        60000,
    ),
  );
  const latestEvidence = row.activity?.evidence.at(-1) ?? row.evidence.at(-1);
  const activityMinutes = Math.max(0, Math.floor((now - Date.parse(row.lastActiveAt)) / 60000));
  const activeAgo = activityMinutes < 1
    ? "刚刚"
    : activityMinutes < 60
      ? `${activityMinutes} 分钟前`
      : `${Math.floor(activityMinutes / 60)} 小时前`;
  return {
    need,
    highlight,
    paused,
    ended,
    tag,
    agent:
      row.runtimeId === "codex"
        ? "Codex"
        : row.runtimeId === "claude-code"
          ? "Claude Code"
          : row.runtimeId,
    elapsed: elapsed(first, ended ? Date.parse(row.lastActiveAt) : now),
    compactElapsed:
      duration < 60
        ? `${duration}m`
        : `${Math.floor(duration / 60)}h${duration % 60 ? ` ${duration % 60}m` : ""}`,
    activeAgo,
    activityTitle: `最后活动：${new Date(row.lastActiveAt).toLocaleString("zh-CN", { hour12: false })}`,
    lastAgo: activityMinutes < 1 ? "刚刚" : `${elapsed(row.lastActiveAt, now)}前`,
    tagBg: highlight
      ? "#FFF1DB"
      : row.state === "running"
        ? "#EEF0FD"
        : row.state === "accepted"
          ? "#4F5BD5"
          : "#EEF0F3",
    tagFg: highlight
      ? "#B45309"
      : row.state === "running"
        ? "#4F5BD5"
        : row.state === "accepted"
          ? "#fff"
          : "#4B5160",
    action:
      row.state === "needs_input"
        ? "跳到 agent 回答"
        : row.state === "review"
          ? row.progress.total === 0
            ? "查看回复"
            : row.progress.done < row.progress.total
            ? "生成追问"
            : "验收 (A)"
          : row.offPlan.length && !ended
            ? "看偏离"
            : null,
    now:
      row.activity?.action ??
      (row.state === "review"
        ? row.progress.total === 0 ? "这一轮已回复，等你查看" : "这一轮已结束，等你核对"
        : (row.statusReason ?? "暂无可读取的执行活动")),
    items: row.items.filter(confirmedRequirement),
    evidence: evidenceText(row),
    evidenceSummary: latestEvidence
      ? evidenceSummary(latestEvidence)
      : "暂无执行证据",
    ask: row.asks[0]?.text ?? "",
  };
}
export type RowPresentation = ReturnType<typeof rowPresentation>;
export function groupRows(
  rows: LedgerDetail[],
  grouping: "urgency" | "project",
  now = Date.now(),
) {
  if (grouping === "project")
    return [...new Set(rows.map((r) => r.projectRoot))].map((root) => ({
      key: root,
      label: projectName(root),
      hint: `${rows.filter((r) => r.projectRoot === root && rowPresentation(r, now).need).length} 需要你`,
      rows: rows
        .filter((r) => r.projectRoot === root)
        .sort(
          (a, b) =>
            Number(rowPresentation(b, now).need) -
            Number(rowPresentation(a, now).need),
        ),
    }));
  return [
    {
      key: "need",
      label: "需要你",
      hint: "按等待时长排序",
      rows: rows
        .filter((r) => rowPresentation(r, now).need)
        .sort(
          (a, b) => Number(b.state === "needs_input") - Number(a.state === "needs_input") || Date.parse(a.lastActiveAt) - Date.parse(b.lastActiveAt),
        ),
    },
    {
      key: "running",
      label: "运行中",
      hint: "按最后动作排序",
      rows: rows
        .filter((r) => {
          const p = rowPresentation(r, now);
          return !p.need && !p.paused && !p.ended;
        })
        .sort(
          (a, b) => Date.parse(b.lastActiveAt) - Date.parse(a.lastActiveAt),
        ),
    },
    {
      key: "ended",
      label: "已结束",
      hint: "按最后活动排序",
      rows: rows
        .filter((r) => rowPresentation(r, now).ended)
        .sort(
          (a, b) => Date.parse(b.lastActiveAt) - Date.parse(a.lastActiveAt),
        ),
    },
    {
      key: "paused",
      label: "暂停",
      hint: "进程安静、停下或丢失",
      rows: rows.filter((r) => rowPresentation(r, now).paused),
    },
  ].filter((g) => g.rows.length || g.key !== "paused");
}
export function requirementStatus(item: RequirementItem) {
  return item.dropped
    ? "已放弃"
    : item.source === "fallback"
      ? "待你确认 · 不计进度"
      : itemStateText[item.status];
}
