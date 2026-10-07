import { useWebSocket } from "./useWebSocket";
import { getWebSocketManager } from "@/services/websocket";
import { getAutostart, isNativeApp, setAutostart } from "@/pages/ledger/native";
import { useCallback, useEffect, useRef, useState } from "react";
import { fetchSession } from "@/services/api";
import type { RecoveryInfo } from "@/types";
import { ledgerApi as api } from "@/services/ledger";
import type { WorkItem } from "@/types/work-item";
import type {
  LedgerConfig,
  LedgerDetail,
  RequirementItem,
} from "../../../../domain/ledger/types";
import { DEFAULT_LEDGER_CONFIG } from "../../../../domain/ledger/types";

import type { LedgerProps, Todo, Goal, Review } from "@/pages/ledger/types";

function savedHours() {
  try {
    const hours = Number(localStorage.getItem("keepline.overview-hours"));
    if (Number.isInteger(hours) && hours >= 1 && hours <= DEFAULT_LEDGER_CONFIG.retentionDays * 24)
      return hours;
  } catch {
    // The time filter still works when browser storage is unavailable.
  }
  return 24;
}

export function useLedger({ view, token }: LedgerProps) {
  useWebSocket({ token });
  const [hours, setHours] = useState(savedHours);
  const hoursRef = useRef(hours);
  const [rows, setRows] = useState<LedgerDetail[]>([]);
  const [goals, setGoals] = useState<Goal[]>([]);
  const [projectMapGoalId, setProjectMapGoalId] = useState<string | null>(null);
  const projectMapGoalRef = useRef(projectMapGoalId);
  projectMapGoalRef.current = projectMapGoalId;
  useEffect(() => {
    if (view !== "goals") setProjectMapGoalId(null);
  }, [view]);
  const [todos, setTodos] = useState<WorkItem[]>([]);
  const [settings, setSettings] = useState<LedgerConfig>(
    structuredClone(DEFAULT_LEDGER_CONFIG),
  );
  const [recovery, setRecovery] = useState<RecoveryInfo | null>(null);
  const [detail, setDetail] = useState<LedgerDetail | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(() =>
    new URLSearchParams(window.location.search).get("sessionId"),
  );
  const [autostart, setAutostartState] = useState(false);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [review, setReview] = useState<Review | null>(null);
  const [reviewError, setReviewError] = useState("");
  const [reviewRevision, setReviewRevision] = useState(0);
  const [date, setDate] = useState(new Date().toLocaleDateString("en-CA"));
  const [weekly, setWeekly] = useState(false);
  const [prompt, setPrompt] = useState<{
    text: string;
    sessionId?: string;
    dispatch?: Todo;
    runtime?: "codex" | "claude-code";
    idempotencyKey?: string;
  } | null>(null);
  const [editing, setEditing] = useState<{
    id?: string;
    title: string;
    level: "goal" | "task";
    parentId: string;
    outcome: string;
    checklist: string;
    projectRoot: string;
  } | null>(null);
  const [itemEditor, setItemEditor] = useState<RequirementItem[] | null>(null);
  const [itemFilter, setItemFilter] = useState<string | null>(null);
  const hovering = useRef(false);
  const [queuedRows, setQueuedRows] = useState<LedgerDetail[] | null>(null);
  const settingsLoaded = useRef(false);
  const changeHours = (next: number) => {
    if (next === hours) return;
    hoursRef.current = next;
    hovering.current = false;
    setQueuedRows(null);
    setRows([]);
    setHours(next);
  };
  useEffect(() => {
    try {
      localStorage.setItem("keepline.overview-hours", String(hours));
    } catch {
      // Saving the preference is optional.
    }
  }, [hours]);
  useEffect(() => {
    const url = new URL(window.location.href);
    if (selectedId) url.searchParams.set("sessionId", selectedId);
    else {
      url.searchParams.delete("sessionId");
      url.searchParams.delete("anchor");
    }
    window.history.replaceState(null, "", url);
  }, [selectedId]);
  const load = useCallback(async () => {
    const [ledger, goalData, work, cfg] = await Promise.all([
      api<LedgerDetail[]>(`/ledger?hours=${hours}`),
      api<Goal[]>(projectMapGoalId ? `/goals?projectMap=${encodeURIComponent(projectMapGoalId)}` : "/goals"),
      api<{ items: WorkItem[] }>("/work-items"),
      api<LedgerConfig>("/settings/ledger"),
    ]);
    if (hoursRef.current !== hours || projectMapGoalRef.current !== projectMapGoalId) return;
    if (hovering.current) setQueuedRows(ledger);
    else setRows(ledger);
    setGoals(goalData);
    setTodos(work.items.filter((w) => w.level !== "goal" && w.kind === "todo"));
    if (!settingsLoaded.current) {
      setSettings(cfg);
      settingsLoaded.current = true;
    }
    setError("");
    setLoading(false);
  }, [hours, projectMapGoalId]);
  const pauseRows = () => {
    hovering.current = true;
  };
  const resumeRows = () => {
    hovering.current = false;
    if (queuedRows) {
      setRows(queuedRows);
      setQueuedRows(null);
    }
  };
  const perform = useCallback(
    async (action: () => Promise<unknown>) => {
      setError("");
      setBusy(true);
      try {
        await action();
        await load();
        setReviewRevision((revision) => revision + 1);
        if (selectedId)
          setDetail(
            await api<LedgerDetail>(
              `/ledger/${encodeURIComponent(selectedId)}`,
            ),
          );
      } catch (e) {
        setError(e instanceof Error ? e.message : "操作失败");
      } finally {
        setBusy(false);
      }
    },
    [load, selectedId],
  );
  useEffect(() => {
    void load().catch((e) => {
      setError(String(e));
      setLoading(false);
    });
    const timer = window.setInterval(() => {
      void load().catch((e) => {
        setError(String(e));
        setLoading(false);
      });
    }, 30000);
    return () => window.clearInterval(timer);
  }, [load]);
  useEffect(() => {
    setDetail(null);
    if (!selectedId || !["overview", "todos", "goals", "review"].includes(view)) return;
    let active = true;
    const update = async () => {
      await api(`/ledger/${encodeURIComponent(selectedId)}/viewing`, "POST", {
        viewed: true,
      });
      const data = await api<LedgerDetail>(
        `/ledger/${encodeURIComponent(selectedId)}`,
      );
      if (active) setDetail(data);
    };
    void update().catch((e) => setError(String(e)));
    const timer = window.setInterval(() => {
      void update().catch((e) => setError(String(e)));
    }, 15000);
    return () => {
      active = false;
      window.clearInterval(timer);
      void api(`/ledger/${encodeURIComponent(selectedId)}/viewing`, "POST", {
        viewed: false,
      }).catch((e) => console.warn("Unable to clear ledger viewing state", e));
    };
  }, [selectedId, view]);
  useEffect(() => {
    if (view !== "review") return;
    let active = true;
    setReview(null);
    setReviewError("");
    void api<Review>(`/ledger/review?${weekly ? "week" : "date"}=${date}`)
      .then((data) => {
        if (active) setReview(data);
      })
      .catch((e) => {
        if (active) setReviewError(String(e));
      });
    return () => {
      active = false;
    };
  }, [view, date, weekly, reviewRevision]);
  const openPrompt = async (row: Pick<LedgerDetail, "sessionId">, runId?: string) => {
    const result = await api<{ text: string }>(
      `/ledger/${encodeURIComponent(row.sessionId)}/${runId ? `correction?runId=${encodeURIComponent(runId)}` : "follow-up"}`,
    );
    setPrompt({ text: result.text, sessionId: row.sessionId });
  };
  useEffect(
    () =>
      getWebSocketManager().onMessage((message) => {
        if (
          message.type.startsWith("ledger:") ||
          message.type === "sessions:update"
        )
          void load().catch((e) => {
            setError(String(e));
            setLoading(false);
          });
      }),
    [load],
  );
  useEffect(() => {
    if (isNativeApp())
      void getAutostart()
        .then(setAutostartState)
        .catch((e) => setError(String(e)));
  }, []);
  useEffect(() => {
    const anchor = new URLSearchParams(window.location.search).get("anchor");
    if (detail && anchor)
      document
        .getElementById(`ledger-${anchor}`)
        ?.scrollIntoView({ block: "center" });
  }, [detail?.sessionId]);
  const itemPath = (row: LedgerDetail) => {
    const todo = todos.find((t) => t.id === row.workItemId);
    const goal = goals.find((g) => g.id === todo?.parentId);
    return todo
      ? `${goal ? `${goal.title} › ` : ""}${todo.title}`
      : "未关联目标";
  };
  const mutateDetail = async (path: string, data: unknown, method = "POST") => {
    if (!detail) return;
    const result = await api<LedgerDetail>(
      `/ledger/${encodeURIComponent(detail.sessionId)}/${path}`,
      method,
      data,
    );
    setDetail(result);
  };
  const saveWorkItem = async () => {
    if (!editing) return;
    const old = [...goals, ...todos].find((i) => i.id === editing.id);
    const texts = editing.checklist
      .split("\n")
      .map((t) => t.trim())
      .filter(Boolean);
    const acceptance = texts.map((text, i) => ({
      id: old?.acceptance?.[i]?.id ?? crypto.randomUUID(),
      text,
      completed: old?.acceptance?.[i]?.completed ?? false,
    }));
    await api(
      `/work-items${editing.id ? `/${editing.id}` : ""}`,
      editing.id ? "PATCH" : "POST",
      {
        title: editing.title,
        level: editing.level,
        parentId: editing.level === "goal" ? null : editing.parentId || null,
        outcome: editing.outcome,
        acceptance,
        projectRoot: editing.projectRoot || null,
        kind: "todo",
      },
    );
    setEditing(null);
  };
  const editWorkItem = (item?: WorkItem, parentId = "") =>
    setEditing({
      id: item?.id,
      title: item?.title ?? "",
      level: item?.level ?? "task",
      parentId: item?.parentId ?? parentId,
      outcome: item?.outcome ?? "",
      checklist: item?.acceptance?.map((c) => c.text).join("\n") ?? "",
      projectRoot:
        item?.projectRoot ??
        goals.find((g) => g.id === parentId)?.projectRoot ??
        "",
    });
  const saveSettings = async (patch: Partial<LedgerConfig> = {}) => {
    const next = { ...settings, ...patch };
    const saved = await api<LedgerConfig>("/settings/ledger", "PUT", {
      ...next,
      exclude: {
        ...next.exclude,
        projects: next.exclude.projects
          .map((path) => path.trim())
          .filter(Boolean),
      },
    });
    setSettings(saved);
  };
  const toggleAutostart = async (on: boolean) => {
    await setAutostart(on);
    setAutostartState(on);
  };
  const toggleFocus = () =>
    saveSettings({
      focus: {
        ...settings.focus,
        until:
          settings.focus.until && Date.parse(settings.focus.until) > Date.now()
            ? null
            : new Date(
                Date.now() + settings.focus.minutes * 60000,
              ).toISOString(),
      },
    });
  const accept = async (
    row: LedgerDetail,
    decision: "accepted" | "accepted_with_gaps" | "follow_up",
    reason?: string,
  ) => {
    const result = await api<LedgerDetail>(
      `/ledger/${encodeURIComponent(row.sessionId)}/acceptances`,
      "POST",
      {
        decision,
        reason,
        ...(decision === "accepted_with_gaps"
          ? {
              droppedItemIds: row.items
                .filter((i) => i.status !== "done" || !i.evidenceIds.length)
                .map((i) => i.id),
            }
          : {}),
      },
    );
    if (selectedId === row.sessionId) setDetail(result);
  };
  const carryOver = (sessionId: string) =>
    api(`/ledger/${encodeURIComponent(sessionId)}/carry-over`, "POST", {});
  const completeTodo = (id: string) =>
    api(`/goals/todos/${id}/complete`, "POST", {});
  const prepareDispatch = (todo: Todo) =>
    setPrompt({
      text: `${todo.title}\n${todo.parentId ? `目标：${goals.find((g) => g.id === todo.parentId)?.title ?? ""}\n` : ""}\n验收清单：\n${todo.checklist.map((i) => `- ${i.text}`).join("\n")}`,
      dispatch: todo,
      runtime: "codex",
      idempotencyKey: crypto.randomUUID(),
    });
  const dispatchPrompt = async () => {
    if (!prompt?.dispatch) return;
    await api(`/goals/todos/${prompt.dispatch.id}/dispatch`, "POST", {
      runtimeId: prompt.runtime,
      cwd: prompt.dispatch.projectRoot,
      prompt: prompt.text,
      idempotencyKey: prompt.idempotencyKey ?? crypto.randomUUID(),
    });
    setPrompt(null);
  };
  const saveRequirements = async () => {
    if (itemEditor === null) return;
    await mutateDetail(
      "items",
      {
        items: itemEditor.map((item) => ({
          ...item,
          id: detail?.items.some((existing) => existing.id === item.id)
            ? item.id
            : undefined,
          anchors: Object.fromEntries(
            Object.entries(item.anchors).map(([key, values]) => [
              key,
              values.map((value: string) => value.trim()).filter(Boolean),
            ]),
          ),
          constraints: item.constraints
            .filter((c) => c.kind !== "path_forbidden" || c.value?.trim())
            .map((c) => ({
              ...c,
              ...(c.value !== undefined ? { value: c.value.trim() } : {}),
            })),
        })),
      },
      "PUT",
    );
    setItemEditor(null);
  };
  useEffect(() => {
    setRecovery(null);
    if (!detail || detail.state !== "stopped") return;
    let active = true;
    void fetchSession(detail.sessionId)
      .then((response) => {
        if (!active) return;
        if (response.success && response.data)
          setRecovery(response.data.recovery);
        else setError(response.error ?? "无法读取恢复信息");
      })
      .catch((e) => {
        if (active)
          setError(e instanceof Error ? e.message : "无法读取恢复信息");
      });
    return () => {
      active = false;
    };
  }, [detail?.sessionId, detail?.state]);
  return {
    hours,
    changeHours,
    rows,
    setRows,
    goals,
    projectMapGoalId,
    setProjectMapGoalId,
    todos,
    settings,
    setSettings,
    detail,
    setDetail,
    selectedId,
    setSelectedId,
    autostart,
    setAutostartState,
    error: error || (view === "review" ? reviewError : ""),
    loading,
    busy,
    review,
    date,
    setDate,
    weekly,
    setWeekly,
    prompt,
    setPrompt,
    editing,
    setEditing,
    itemEditor,
    setItemEditor,
    itemFilter,
    setItemFilter,
    queuedRows,
    pauseRows,
    resumeRows,
    load,
    perform,
    openPrompt,
    itemPath,
    mutateDetail,
    saveWorkItem,
    editWorkItem,
    saveSettings,
    recovery,
    toggleAutostart,
    toggleFocus,
    accept,
    carryOver,
    completeTodo,
    prepareDispatch,
    dispatchPrompt,
    saveRequirements,
  };
}

export type LedgerController = ReturnType<typeof useLedger>;
