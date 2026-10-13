import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ledgerApi as api } from "@/services/ledger";
import { getWebSocketManager } from "@/services/websocket";
import { setNativeCounts } from "@/pages/ledger/native";
import { groupRows } from "@/pages/ledger/presentation";
import type { LedgerConfig, LedgerDetail } from "../../../../domain/ledger/types";

export function useMenubar(token: string) {
  const [rows, setRows] = useState<LedgerDetail[] | null>(null);
  const [settings, setSettings] = useState<LedgerConfig | null>(null);
  const [error, setError] = useState("");
  const [nativeError, setNativeError] = useState("");
  const [settingsError, setSettingsError] = useState("");
  const [loading, setLoading] = useState(true);
  const [connected, setConnected] = useState(false);
  const [now, setNow] = useState(Date.now);
  const [lastSyncedAt, setLastSyncedAt] = useState<number | null>(null);
  const request = useRef(0);
  const settingsRequest = useRef(0);
  const savingFocus = useRef(false);
  const groups = useMemo(() => groupRows(rows ?? [], "urgency", now), [rows, now]);
  const needs = groups.find(group => group.key === "need")!.rows;
  const running = groups.find(group => group.key === "running")!.rows;
  const unconfirmed = groups.find(group => group.key === "unknown")?.rows ?? [];

  const refresh = useCallback(async () => {
    const id = ++request.current;
    setLoading(true);
    try {
      const data = await api<LedgerDetail[]>("/ledger/menubar");
      if (id !== request.current) return;
      setRows(data);
      setLastSyncedAt(Date.now());
      setError("");
      // The tray and the visible sections must use exactly the same grouping.
      const next = groupRows(data, "urgency");
      try {
        await setNativeCounts(next.find(group => group.key === "need")!.rows.length,
          next.find(group => group.key === "running")!.rows.length);
        if (id === request.current) setNativeError("");
      } catch (error) {
        if (id === request.current) setNativeError(String(error));
      }
    } catch (error) {
      if (id === request.current) setError(error instanceof Error ? error.message : String(error));
    } finally {
      if (id === request.current) setLoading(false);
    }
  }, []);
  const refreshSettings = useCallback(async () => {
    if (savingFocus.current) return;
    const id = ++settingsRequest.current;
    try {
      const data = await api<LedgerConfig>("/settings/ledger");
      if (id === settingsRequest.current) { setSettings(data); setSettingsError(""); }
    } catch (error) {
      if (id === settingsRequest.current) setSettingsError(String(error));
    }
  }, []);

  useEffect(() => {
    const manager = getWebSocketManager();
    let queued: ReturnType<typeof setTimeout> | undefined;
    const schedule = () => {
      if (queued) return;
      queued = setTimeout(() => { queued = undefined; void refresh(); }, 100);
    };
    const unsubscribe = manager.onMessage(message => {
      if (message.type.startsWith("ledger:") || message.type === "sessions:update" || message.type === "sync:complete") schedule();
    });
    const unsubscribeStatus = manager.onStatusChange(status => {
      setConnected(status === "connected");
      if (status === "connected") schedule();
    });
    manager.connect(token);
    void refresh();
    void refreshSettings();
    // Keep tray counts current even while the popover is hidden.
    const timer = window.setInterval(() => { void refresh(); void refreshSettings(); }, 30000);
    const clock = window.setInterval(() => {
      if (document.visibilityState === "visible") setNow(Date.now());
    }, 15000);
    const visible = () => {
      if (document.visibilityState === "visible") { schedule(); void refreshSettings(); setNow(Date.now()); }
    };
    document.addEventListener("visibilitychange", visible);
    return () => {
      ++request.current;
      ++settingsRequest.current;
      unsubscribe(); unsubscribeStatus();
      if (queued) clearTimeout(queued);
      window.clearInterval(timer); window.clearInterval(clock);
      document.removeEventListener("visibilitychange", visible);
    };
  }, [token, refresh, refreshSettings]);

  const toggleFocus = async () => {
    if (!settings) return;
    savingFocus.current = true;
    ++settingsRequest.current;
    try {
      // This endpoint replaces the config. Read the latest value so changes
      // made in the main window are preserved when toggling focus here.
      const current = await api<LedgerConfig>("/settings/ledger");
      const active = current.focus.until && Date.parse(current.focus.until) > Date.now();
      const saved = await api<LedgerConfig>("/settings/ledger", "PUT", {
        ...current,
        focus: { ...current.focus, until: active ? null : new Date(Date.now() + current.focus.minutes * 60000).toISOString() },
      });
      setSettings(saved);
      setSettingsError("");
      setNow(Date.now());
    } finally { savingFocus.current = false; }
  };

  return { needs, running, unconfirmed, settings, error, nativeError, settingsError, loading,
    loaded: rows !== null, connected, now, lastSyncedAt, refresh, refreshSettings, toggleFocus };
}
