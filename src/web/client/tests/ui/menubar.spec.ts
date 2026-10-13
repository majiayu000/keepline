import { test, expect, type Page, type WebSocketRoute } from "@playwright/test";
import { now, row, settings } from "./fixtures";
import type { LedgerConfig, LedgerDetail } from "../../../../domain/ledger/types";

// Close before request routes are removed, including WebKit's late refreshes.
test.afterEach(async ({ page }) => { await page.close(); });

type NativeCall = { command: string; args: Record<string, unknown> };
declare global {
  interface Window {
    menubarCalls: NativeCall[];
    menubarNativeError: string | null;
  }
}

function session(sessionId: string, changes: Partial<LedgerDetail> = {}): LedgerDetail {
  return { ...structuredClone(row), sessionId, ...changes };
}
const waiting = session("waiting", {
  title: "确认数据库备份位置", state: "needs_input", runtimeId: "codex",
  inputRequest: { tool: "request_user_input", question: "备份保存到本地，还是上传到远端？", at: now },
});
const review = session("review", { title: "恢复接口已补齐，等你核对", state: "review" });

async function setup(page: Page, rows: LedgerDetail[] = [waiting, review, row]) {
  await page.setViewportSize({ width: 390, height: 550 });
  await page.clock.install({ time: new Date(now) });
  await page.addInitScript(() => {
    localStorage.setItem("terminal_token", "menubar-ui-test");
    window.menubarCalls = [];
    window.menubarNativeError = null;
    Object.assign(window, {
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: Record<string, unknown>) => {
          window.menubarCalls.push({ command, args });
          if (window.menubarNativeError && command !== "set_ledger_counts") throw new Error(window.menubarNativeError);
        },
      },
    });
  });
  const state = {
    rows, settings: structuredClone(settings),
    failLedger: false, failSettings: false, failSave: false,
    ledgerCalls: 0, writes: [] as LedgerConfig[],
    gate: undefined as Promise<void> | undefined,
    socket: undefined as WebSocketRoute | undefined,
  };
  await page.routeWebSocket(/\/ws\?/, socket => {
    state.socket = socket;
    socket.onMessage(message => {
      if (JSON.parse(String(message)).type === "ping") socket.send(JSON.stringify({ type: "pong" }));
    });
  });
  await page.route("**/api/**", async route => {
    const path = new URL(route.request().url()).pathname;
    let data: unknown;
    if (path === "/api/auth/status") data = { setupComplete: true, authenticated: true, username: "local" };
    else if (path === "/api/ledger/menubar") {
      state.ledgerCalls++;
      data = structuredClone(state.rows);
      if (state.gate) await state.gate;
      if (state.failLedger) return route.fulfill({ status: 503, json: { success: false, error: "服务暂不可用" } });
    } else if (path === "/api/settings/ledger") {
      if (route.request().method() === "PUT") {
        const patch = route.request().postDataJSON() as LedgerConfig;
        state.writes.push(patch);
        if (state.failSave) return route.fulfill({ status: 500, json: { success: false, error: "专注设置保存失败" } });
        state.settings = patch;
      } else if (state.failSettings) return route.fulfill({ status: 503, json: { success: false, error: "设置暂不可用" } });
      data = state.settings;
    } else throw new Error(`Unexpected API request: ${path}`);
    await route.fulfill({ json: { success: true, data } });
  });
  return state;
}
async function ready(page: Page) {
  await page.goto("/menubar");
  await expect(page.getByLabel("会话统计")).toHaveAttribute("aria-busy", "false");
  await expect(page.getByRole("button", { name: /专注 30 分钟/ })).toBeEnabled();
}
async function latestCounts(page: Page) {
  return page.evaluate(() => window.menubarCalls.filter(call => call.command === "set_ledger_counts").at(-1)?.args);
}

// Covers the actual 390 × 550 popover, not a full-size dashboard viewport.
test("compact layout, shared state semantics and identical native counts", async ({ page }, testInfo) => {
  const deviation = session("deviation", { title: "发现计划外的文件修改", offPlan: [{ id: "off", at: now, callIds: ["call-1"] }] });
  const reply = session("reply", { title: "组件重构有了新回复", state: "review", progress: { done: 0, total: 0 } });
  await setup(page, [waiting, review, deviation, reply, row,
    session("unknown", { processState: "unknown" }),
    session("exited", { processState: "exited" }),
    session("exited-review", { processState: "exited", state: "review" }),
  ]);
  await ready(page);
  await expect.poll(() => latestCounts(page)).toEqual({ needsYou: 4, running: 1 });
  await expect(page.getByText(/等你审批/)).toHaveCount(0);
  await expect(page.getByText("▲ 等你处理 · 0 分钟", { exact: true })).toBeVisible();
  await expect(page.getByText("◐ 新回复", { exact: true })).toHaveCount(1);
  await expect(page.getByText(/1 个会话的在线状态待确认/)).toHaveCount(1);
  const footerBefore = await page.locator("footer").boundingBox();
  await page.getByLabel("会话列表").evaluate(node => { node.scrollTop = node.scrollHeight; });
  expect(await page.locator("footer").boundingBox()).toEqual(footerBefore);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(390);
  expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(550);
  await page.getByLabel("会话列表").evaluate(node => { node.scrollTop = 0; });
  await page.screenshot({ animations: "disabled", path: testInfo.outputPath("menubar-populated.png") });
});

test("long synthetic titles stay compact and every item remains reachable", async ({ page }) => {
  const rows = Array.from({ length: 14 }, (_, i) => session(`synthetic-${i}`, {
    ...waiting, sessionId: `synthetic-${i}`, title: `Keepline SYNTHETIC TRACE CLICK notification acceptance ${i} 很长的测试会话标题`,
  }));
  await setup(page, rows);
  await ready(page);
  await expect(page.getByRole("button", { name: /^打开会话/ })).toHaveCount(14);
  const heights = await page.getByRole("button", { name: /^打开会话/ }).evaluateAll(buttons => buttons.map(button => button.getBoundingClientRect().height));
  expect(Math.max(...heights)).toBeLessThan(105);
  const last = page.getByRole("button", { name: `打开会话：${rows[13].title}` });
  await last.click();
  await expect.poll(() => page.evaluate(() => window.menubarCalls.filter(call => call.command === "open_ledger").at(-1))).toEqual({
    command: "open_ledger", args: { sessionId: "synthetic-13", anchor: "current-step" },
  });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(390);
});

test("loading does not report zero and empty state is deliberate", async ({ page }, testInfo) => {
  const state = await setup(page, []);
  let release!: () => void;
  state.gate = new Promise<void>(resolve => { release = resolve; });
  await page.goto("/menubar");
  await expect(page.getByRole("status", { name: "正在读取会话" })).toBeVisible();
  await expect(page.getByLabel("会话统计")).toContainText("—");
  await page.screenshot({ animations: "disabled", path: testInfo.outputPath("menubar-loading.png") });
  release();
  state.gate = undefined;
  await expect(page.getByRole("heading", { name: "暂无待处理或运行中的会话" })).toBeVisible();
  await expect.poll(() => latestCounts(page)).toEqual({ needsYou: 0, running: 0 });
  await page.screenshot({ animations: "disabled", path: testInfo.outputPath("menubar-empty.png") });
});

test("failed refresh preserves rows, native counts and a retry action", async ({ page }) => {
  const state = await setup(page);
  // Pause before loading the app, while it has no polling timers. A future
  // instant avoids a cross-process Date.now()/pauseAt race on WebKit.
  await page.clock.pauseAt(new Date(now).getTime() + 3600000);
  await ready(page);
  // Drain the initial WebSocket catch-up before introducing a failure. Freeze
  // polling while clicking retry so a background success cannot remove it.
  await page.clock.runFor(150);
  await expect.poll(() => state.ledgerCalls).toBeGreaterThanOrEqual(2);
  await expect(page.getByLabel("会话统计")).toHaveAttribute("aria-busy", "false");
  state.failLedger = true;
  state.socket!.send(JSON.stringify({ type: "ledger:update" }));
  await page.clock.runFor(150);
  await expect(page.getByRole("alert")).toContainText("更新失败，保留上次记录");
  await expect(page.getByRole("button", { name: `打开会话：${waiting.title}` })).toBeVisible();
  expect(await latestCounts(page)).toEqual({ needsYou: 2, running: 1 });
  state.failLedger = false;
  state.rows = [];
  const beforeRetry = state.ledgerCalls;
  await page.getByRole("button", { name: "重新加载" }).click();
  await expect.poll(() => state.ledgerCalls).toBe(beforeRetry + 1);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect.poll(() => latestCounts(page)).toEqual({ needsYou: 0, running: 0 });
});

test("older responses cannot overwrite newer state or tray counts", async ({ page }) => {
  const state = await setup(page);
  await ready(page);
  // Let the connection catch-up request finish before controlling the next pair.
  await expect.poll(() => state.ledgerCalls).toBeGreaterThanOrEqual(2);
  let release!: () => void;
  state.gate = new Promise<void>(resolve => { release = resolve; });
  const before = state.ledgerCalls;
  state.socket!.send(JSON.stringify({ type: "ledger:update" }));
  await expect.poll(() => state.ledgerCalls).toBe(before + 1);
  state.gate = undefined;
  state.rows = [];
  state.socket!.send(JSON.stringify({ type: "ledger:update" }));
  await expect(page.getByRole("heading", { name: "暂无待处理或运行中的会话" })).toBeVisible();
  release();
  await page.clock.runFor(200);
  expect(await latestCounts(page)).toEqual({ needsYou: 0, running: 0 });
  await expect(page.getByRole("button", { name: /^打开会话/ })).toHaveCount(0);
});

test("focus has confirmed feedback, cancellation, expiration and preserves unrelated settings", async ({ page }, testInfo) => {
  const state = await setup(page, [waiting, row]);
  await ready(page);
  const focus = page.getByRole("button", { name: /专注.*分钟/ });
  // A change in the main window must survive the replace-style settings PUT.
  state.settings.retentionDays = 14;
  state.settings.alerts.off_plan = true;
  await focus.click();
  await expect(focus).toHaveAttribute("aria-pressed", "true");
  await expect(focus).toContainText("还剩 30 分钟");
  expect(state.settings.retentionDays).toBe(14);
  expect(state.settings.alerts.off_plan).toBe(true);
  expect(state.settings.focus.minutes).toBe(30);
  await page.screenshot({ animations: "disabled", path: testInfo.outputPath("menubar-focus.png") });
  await focus.click();
  await expect(focus).toHaveAttribute("aria-pressed", "false");
  expect(state.settings.focus.until).toBeNull();
  state.failSave = true;
  await focus.click();
  await expect(page.getByRole("alert")).toContainText("专注设置保存失败");
  await expect(focus).toHaveAttribute("aria-pressed", "false");
  state.failSave = false;
  await focus.click();
  await expect(focus).toHaveAttribute("aria-pressed", "true");
  await page.clock.fastForward(30 * 60000 + 15000);
  await expect(focus).toHaveAttribute("aria-pressed", "false");
});

test("settings failure does not hide sessions and can be retried", async ({ page }) => {
  const state = await setup(page);
  state.failSettings = true;
  await page.goto("/menubar");
  await expect(page.getByRole("button", { name: `打开会话：${waiting.title}` })).toBeVisible();
  await expect(page.getByRole("alert")).toContainText("无法读取专注设置");
  state.failSettings = false;
  await page.getByRole("button", { name: "重试", exact: true }).click();
  await expect(page.getByRole("button", { name: /专注 30 分钟/ })).toBeEnabled();
});

test("quit confirmation traps focus, cancels with Escape and routes both exit choices", async ({ page }, testInfo) => {
  await setup(page);
  await ready(page);
  const quit = page.getByRole("button", { name: "退出…", exact: true });
  await quit.click();
  const modal = page.getByRole("dialog", { name: "退出 Keepline？" });
  await expect(modal).toBeVisible();
  await expect(page.getByRole("button", { name: "取消", exact: true })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "保持监控并退出", exact: true })).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(page.getByRole("button", { name: "取消", exact: true })).toBeFocused();
  await page.screenshot({ animations: "disabled", path: testInfo.outputPath("menubar-quit.png") });
  await page.keyboard.press("Escape");
  await expect(modal).not.toBeVisible();
  await expect(quit).toBeFocused();
  await quit.click();
  await page.getByRole("button", { name: "保持监控并退出", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.menubarCalls.filter(call => call.command === "quit_app").at(-1)?.args)).toEqual({ stopMonitoring: false });
  await page.evaluate(() => { window.menubarNativeError = "退出失败，请重试"; });
  await page.getByRole("button", { name: "停止监控并退出", exact: true }).click();
  await expect(modal.getByRole("alert")).toContainText("退出失败，请重试");
  expect(await page.evaluate(() => window.menubarCalls.filter(call => call.command === "quit_app").at(-1)?.args)).toEqual({ stopMonitoring: true });
});

test("session and main-panel navigation surface native failures", async ({ page }) => {
  await setup(page);
  await ready(page);
  await page.getByRole("button", { name: /打开主面板/ }).click();
  await expect.poll(() => page.evaluate(() => window.menubarCalls.filter(call => call.command === "open_ledger").length)).toBe(1);
  await page.evaluate(() => { window.menubarNativeError = "无法打开主面板"; });
  await page.getByRole("button", { name: `打开会话：${waiting.title}` }).click();
  await expect(page.getByRole("alert")).toContainText("无法打开主面板");
  await page.evaluate(() => { window.menubarNativeError = null; });
  await page.getByRole("button", { name: `打开会话：${review.title}` }).click();
  await expect.poll(() => page.evaluate(() => window.menubarCalls.filter(call => call.command === "open_ledger").at(-1)?.args)).toEqual({ sessionId: "review", anchor: "requirements" });
  await expect(page.getByRole("alert")).toHaveCount(0);
});
