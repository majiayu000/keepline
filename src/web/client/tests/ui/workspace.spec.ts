import { test, expect, type Page } from "@playwright/test";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { now, row, settings } from "./fixtures";

async function mockApi(page: Page) {
  await page.clock.install({ time: new Date(now) });
  await page.addInitScript(() =>
    localStorage.setItem("terminal_token", "ui-contract-test"),
  );
  const calls: Array<{
    path: string;
    method: string;
    data: Record<string, unknown> | null;
  }> = [];
  const state = {
    row: structuredClone(row),
    settings: structuredClone(settings),
    failSettings: false,
    failReview: false,
  };
  const task = {
    id: "todo-1",
    title: "补充恢复接口文档",
    kind: "todo",
    level: "task",
    parentId: "goal-1",
    status: "planned",
    projectRoot: "/project/keepline",
    statusSource: "user",
    createdAt: now,
    updatedAt: now,
    acceptance: [{ id: "check-1", text: "生成恢复接口文档", completed: false }],
  };
  const goal = {
    id: "goal-1",
    title: "完善本地 API",
    kind: "todo",
    level: "goal",
    status: "active",
    outcome: "全部路由有文档",
    statusSource: "user",
    createdAt: now,
    updatedAt: now,
    todos: [
      {
        ...task,
        readyToComplete: false,
        checklist: [
          { id: "check-1", text: "生成恢复接口文档", evidenced: false, satisfied: false },
        ],
        sessions: [],
      },
    ],
    progress: { done: 0, total: 1, active: 0 },
    weeklyMovement: 0,
    stale: false,
    recent: [],
  };
  await page.route("**/api/**", async (route) => {
    const req = route.request(),
      u = new URL(req.url()),
      path = u.pathname,
      method = req.method(),
      data = req.postDataJSON() as Record<string, unknown> | null;
    calls.push({ path, method, data });
    if (
      path === "/api/settings/ledger" &&
      method === "PUT" &&
      state.failSettings
    )
      return route.fulfill({
        status: 400,
        json: { success: false, error: "模拟设置保存失败" },
      });
    if (path === "/api/ledger/review" && state.failReview)
      return route.fulfill({
        status: 500,
        json: { success: false, error: "模拟回顾读取失败" },
      });
    let result: unknown;
    if (path === "/api/auth/status")
      result = { setupComplete: true, authenticated: true, username: "local" };
    else if (path === "/api/ledger") result = [state.row];
    else if (path === "/api/settings/ledger") {
      if (method === "PUT") state.settings = data as unknown as typeof settings;
      result = state.settings;
    } else if (path === "/api/goals") result = [goal];
    else if (path === "/api/work-items")
      result = method === "GET" ? { items: [task] } : { ...task, ...data };
    else if (path === "/api/ledger/review")
      result = {
        open: [state.row],
        accepted: [],
        offPlan: [],
        corrections: [],
        goals: [goal],
        unattributedRuntimeShare: 0.2,
      };
    else if (path === `/api/ledger/${row.sessionId}/follow-up`)
      result = { text: "请补充恢复接口文档并运行测试。" };
    else if (path === `/api/ledger/${row.sessionId}/items`) {
      state.row.items = (data?.items as typeof row.items).map((i) => ({
        ...i,
        source: "user",
      }));
      state.row.progress = {
        done: state.row.items.filter((i) => i.status === "done").length,
        total: state.row.items.length,
      };
      result = state.row;
    } else if (path.startsWith(`/api/ledger/${row.sessionId}`))
      result = state.row;
    else if (path.startsWith("/api/goals/todos/"))
      result = { id: "dispatch-1", status: "queued" };
    else if (path === "/api/sessions")
      result = {
        sessions: [],
        stats: {
          total: 0,
          running: 0,
          waiting: 0,
          idle: 0,
          lost: 0,
          completed: 0,
        },
        pagination: { total: 0, hasMore: false },
      };
    else if (path === "/api/projects")
      result = { projects: [], stats: { total: 0, active: 0 } };
    else result = {};
    await route.fulfill({ json: { success: true, data: result } });
  });
  return { calls, state, goal, task };
}
async function ready(page: Page) {
  await page.goto("/");
  await page.getByRole("button", { name: row.title, exact: true }).waitFor();
  await page.evaluate(() => document.fonts.ready);
}

test("时间范围按最后活动筛选，支持自定义 7 小时并在刷新后保留", async ({ page }) => {
  const { state } = await mockApi(page);
  state.row.lastActiveAt = new Date(Date.parse(now) - 2 * 3600000).toISOString();
  state.row.asks[0].at = new Date(Date.parse(now) - 40 * 3600000).toISOString();
  const older = [8, 26].map(hours => ({
    ...structuredClone(row), sessionId: `older-${hours}`, title: `${hours} 小时前的会话`,
    lastActiveAt: new Date(Date.parse(now) - hours * 3600000).toISOString(),
  }));
  const requestedHours: number[] = [];
  await page.route("**/api/ledger?*", route => {
    const hours = Number(new URL(route.request().url()).searchParams.get("hours"));
    requestedHours.push(hours);
    return route.fulfill({ json: { success: true, data: [state.row, ...older].filter(r => Date.parse(r.lastActiveAt) >= Date.parse(now) - hours * 3600000) } });
  });
  await ready(page);
  const range = page.getByRole("spinbutton", { name: "最近活动小时数" });
  await expect(range).toHaveValue("24");
  const card = page.getByRole("button", { name: row.title, exact: true });
  await expect(card).toContainText("2 小时前");
  await expect(card).not.toContainText("40 小时");
  await expect(page.getByRole("button", { name: older[0].title, exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: older[1].title, exact: true })).toHaveCount(0);
  await range.fill("7");
  await page.getByRole("button", { name: "应用", exact: true }).click();
  await expect.poll(() => requestedHours.at(-1)).toBe(7);
  await expect(card).toBeVisible();
  await expect(page.getByRole("button", { name: older[0].title, exact: true })).toHaveCount(0);
  await page.reload();
  await expect(range).toHaveValue("7");
  await expect(card).toBeVisible();
  expect(requestedHours.at(-1)).toBe(7);
  await range.fill("24");
  await page.getByRole("button", { name: "应用", exact: true }).click();
  await expect(page.getByRole("button", { name: older[0].title, exact: true })).toBeVisible();
  await range.fill("0");
  await page.getByRole("button", { name: "应用", exact: true }).click();
  expect(await range.evaluate((input: HTMLInputElement) => input.validity.valid)).toBe(false);
  expect(requestedHours.at(-1)).toBe(24);
});

test("三种布局、项目过滤、搜索和详情保持可用", async ({ page }, info) => {
  const { calls } = await mockApi(page);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await ready(page);
  await page.screenshot({ path: info.outputPath("overview.png") });
  await page.getByRole("button", { name: "看板", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "看板", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "列表", exact: true }).click();
  await expect(
    page.getByRole("button", { name: row.title, exact: false }).last(),
  ).toBeVisible();
  await page.getByRole("button", { name: "网格", exact: true }).click();
  await page.getByRole("button", { name: "按项目", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "keepline", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "按紧急度", exact: true }).click();
  await page
    .getByRole("textbox", { name: "搜索", exact: true })
    .fill("不存在的会话");
  await expect(page.getByRole("status").filter({ hasText: "暂无会话" })).toBeVisible();
  await page.getByRole("textbox", { name: "搜索", exact: true }).fill("");
  await page.getByRole("button", { name: row.title, exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("上下文");
  await expect(page.getByRole("dialog")).toContainText("未知");
  await page.screenshot({ path: info.outputPath("drawer.png") });
  await page.getByRole("button", { name: "完整进度账 →" }).click();
  await expect(
    page.getByRole("heading", { name: "时间线", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "编辑要求", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveAttribute("aria-modal", "true");
  await page
    .getByRole("textbox", { name: "要求标题" })
    .first()
    .fill("生成 OpenAPI 3.1 文档");
  await page.getByRole("button", { name: "保存并确认" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(
    calls.find((c) => c.path.endsWith("/items") && c.method === "PUT")?.data
      ?.items,
  ).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ title: "生成 OpenAPI 3.1 文档" }),
    ]),
  );
  await page
    .getByRole("button", { name: "bun test openapi", exact: true })
    .click();
  await page
    .getByRole("combobox", { name: "步骤归属", exact: true })
    .selectOption("item-2");
  await expect
    .poll(
      () =>
        calls.find((call) => call.path.endsWith("/corrections"))?.data?.itemId,
    )
    .toBe("item-2");
  expect(errors).toEqual([]);
});
test("待办预览可切换 runtime，只有启动按钮发送派发请求", async ({ page }) => {
  const { calls } = await mockApi(page);
  await ready(page);
  await page
    .getByRole("button", { name: "待办", exact: false })
    .first()
    .click();
  await page.getByRole("button", { name: "交给 Codex ▾", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("先看要发的内容");
  expect(calls.filter((c) => c.path.endsWith("/dispatch"))).toHaveLength(0);
  await page.getByRole("button", { name: "Claude Code", exact: true }).click();
  await page
    .getByRole("dialog")
    .locator("textarea")
    .fill("生成文档后运行测试。");
  await page
    .getByRole("button", { name: "在 keepline 启动 Claude Code", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(calls.find((c) => c.path.endsWith("/dispatch"))?.data).toMatchObject({
    runtimeId: "claude-code",
    cwd: "/project/keepline",
    prompt: "生成文档后运行测试。",
  });
});
test("设置保存失败显示后端错误，不伪装成功；回顾和退出抽屉可导航", async ({
  page,
}) => {
  const { state, calls } = await mockApi(page);
  await ready(page);
  await page
    .getByRole("button", { name: "设置", exact: false })
    .first()
    .click();
  state.failSettings = true;
  await page.getByRole("switch", { name: "偏离", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("模拟设置保存失败");
  await expect(
    page.getByRole("switch", { name: "偏离", exact: true }),
  ).toHaveAttribute("aria-checked", "false");
  state.failSettings = false;
  await page.getByRole("switch", { name: "偏离", exact: true }).click();
  await expect(
    page.getByRole("switch", { name: "偏离", exact: true }),
  ).toHaveAttribute("aria-checked", "true");
  await page.getByText("30 分钟", { exact: true }).click();
  await page
    .getByRole("spinbutton", { name: "专注分钟数", exact: true })
    .click();
  await page
    .getByRole("spinbutton", { name: "专注分钟数", exact: true })
    .fill("45");
  await page.getByRole("heading", { name: "偏离检测", exact: true }).click();
  await expect
    .poll(() =>
      calls.some(
        (c) =>
          c.path === "/api/settings/ledger" &&
          c.method === "PUT" &&
          (c.data?.focus as { minutes?: number })?.minutes === 45,
      ),
    )
    .toBe(true);
  await expect(page.getByText("45 分钟", { exact: true })).toBeVisible();
  await page
    .getByRole("spinbutton", { name: "专注分钟数", exact: true })
    .press("Escape");
  await expect(
    page.getByRole("spinbutton", { name: "专注分钟数", exact: true }),
  ).toBeHidden();
  await page.getByRole("button", { name: "回顾", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "还挂着", exact: true }),
  ).toBeVisible();
  const reviewRequests = calls.filter(
    (c) => c.path === "/api/ledger/review",
  ).length;
  const overviewRequests = calls.filter((c) => c.path === "/api/ledger").length;
  await page.clock.fastForward(31000);
  await expect
    .poll(() => calls.filter((c) => c.path === "/api/ledger").length)
    .toBeGreaterThan(overviewRequests);
  expect(calls.filter((c) => c.path === "/api/ledger/review").length).toBe(
    reviewRequests,
  );
  await page.getByRole("button", { name: "带到明天" }).click();
  await expect
    .poll(() => calls.some((c) => c.path.endsWith("/carry-over")))
    .toBe(true);
  await expect
    .poll(() => calls.filter((c) => c.path === "/api/ledger/review").length)
    .toBeGreaterThan(reviewRequests);
  state.failReview = true;
  await page.getByRole("button", { name: "后一天", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("模拟回顾读取失败");
  await page.clock.fastForward(31000);
  await expect(page.getByRole("alert")).toContainText("模拟回顾读取失败");
  state.failReview = false;
  await page.getByRole("button", { name: "重试", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "还挂着", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
});
test("核对验收与带缺口验收沿用现有契约", async ({ page }) => {
  const { state, calls } = await mockApi(page);
  state.row.state = "review";
  state.row.items.forEach((item) => {
    item.status = "done";
    item.evidenceIds = ["ev-1"];
  });
  state.row.progress.done = 3;
  await ready(page);
  await page.getByRole("button", { name: "验收 (A)", exact: true }).click();
  await expect
    .poll(
      () =>
        calls.find((call) => call.path.endsWith("/acceptances"))?.data
          ?.decision,
    )
    .toBe("accepted");
  await page.getByRole("button", { name: "完整进度账 →" }).click();
  await page.getByRole("button", { name: "接受并放弃剩余项" }).waitFor();
  page.once("dialog", (dialog) => dialog.accept("后续任务单独处理"));
  await page.getByRole("button", { name: "接受并放弃剩余项" }).click();
  await expect
    .poll(() =>
      calls.some(
        (call) =>
          call.data?.decision === "accepted_with_gaps" &&
          call.data?.reason === "后续任务单独处理",
      ),
    )
    .toBe(true);
});
test("没有验收清单的新回复进入需要你，查看不会发送验收", async ({ page }) => {
  const { state, calls } = await mockApi(page);
  state.row.state = "review";
  state.row.items = [];
  state.row.progress = { done: 0, total: 0 };
  await ready(page);
  const needs = page.locator("section").filter({ has: page.getByRole("heading", { name: "需要你", exact: true }) });
  await expect(needs.getByRole("button", { name: "查看回复", exact: true })).toBeVisible();
  await needs.getByRole("button", { name: "查看回复", exact: true }).click();
  await expect(page.getByRole("dialog", { name: `会话详情：${row.title}` })).toBeVisible();
  await expect.poll(() => calls.some(call => call.path.endsWith("/viewing") && call.data?.viewed === true)).toBe(true);
  expect(calls.some(call => call.path.endsWith("/acceptances"))).toBe(false);
});
test("窄屏不横向溢出，抽屉和编辑器支持 Escape", async ({ page }) => {
  await mockApi(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await ready(page);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  const mobileCard = await page
    .getByRole("button", { name: row.title, exact: true })
    .boundingBox();
  expect(mobileCard!.width).toBeGreaterThan(300);
  expect(mobileCard!.x + mobileCard!.width).toBeLessThanOrEqual(390);
  await page.getByRole("button", { name: row.title, exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("button", { name: row.title, exact: true }).click();
  await page.getByRole("button", { name: "完整进度账 →" }).click();
  await expect(
    page.getByRole("heading", { name: "时间线", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.getByRole("button", { name: "编辑要求", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("button", { name: "← 总览", exact: true }).click();
  await expect(page).not.toHaveURL(/detail=full/);
});
test("历史收纳、活跃项目和视图偏好", async ({ page }, info) => {
  const { state } = await mockApi(page);
  const histories = Array.from({ length: 7 }, (_, i) => ({
    ...structuredClone(row),
    sessionId: `history-${i}`,
    title: `历史任务 ${i}`,
    state: "ended" as const,
    projectRoot: `/archive/${i < 4 ? "first" : "second"}/work`,
    lastActiveAt: new Date(Date.parse(now) - (i + 1) * 60000).toISOString(),
  }));
  await page.route("**/api/ledger?*", (r) =>
    r.fulfill({ json: { success: true, data: [state.row, ...histories] } }),
  );
  await ready(page);
  await expect(page.getByRole("button", { name: /^历史任务/ })).toHaveCount(4);
  await expect(
    page.getByRole("button", { name: "历史任务 0", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "查看其余 3 个已结束会话 ↓" }).click();
  await expect(page.getByRole("button", { name: /^历史任务/ })).toHaveCount(7);
  await page.getByRole("button", { name: "收起历史会话 ↑" }).click();
  await expect(page.getByRole("button", { name: /^历史任务/ })).toHaveCount(4);
  await expect(
    page.locator("nav").getByRole("button", { name: /work/ }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "展开更多项目（2 个）" }).click();
  await page.setViewportSize({ width: 1440, height: 480 });
  const collapse = page.getByRole("button", { name: "收起更多项目" });
  const collapseTop = (await collapse.boundingBox())?.y;
  await expect(
    page.locator("nav").getByRole("button", { name: /work.*archive\/first/ }),
  ).toBeVisible();
  await page
    .locator("nav")
    .getByRole("button", { name: /work.*archive\/second/ })
    .click();
  await expect(collapse).toBeInViewport();
  expect((await collapse.boundingBox())?.y).toBe(collapseTop);
  await expect(page.getByRole("button", { name: /^历史任务/ })).toHaveCount(3);
  await page.getByRole("button", { name: "收起更多项目" }).click();
  await expect(
    page.locator("nav").getByRole("button", { name: /work.*archive\/second/ }),
  ).toBeVisible();
  await page
    .locator("nav")
    .getByRole("button", { name: /全部项目/ })
    .click();
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.getByRole("button", { name: "列表", exact: true }).click();
  await page.getByRole("button", { name: "详细", exact: true }).click();
  await page.getByRole("button", { name: "按项目", exact: true }).click();
  await page.reload();
  for (const name of ["列表", "详细", "按项目"])
    await expect(
      page.getByRole("button", { name, exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "网格", exact: true }).click();
  await page.getByRole("button", { name: "按紧急度", exact: true }).click();
  await page.screenshot({ path: info.outputPath("优化后的历史收纳.png") });
});

test("详情优先关键步骤，原始记录和失败上下文可展开", async ({ page }, info) => {
  const { state } = await mockApi(page);
  state.row.trail.unshift({
    ...structuredClone(row.trail[0]),
    callId: "raw-call",
    itemId: undefined,
    evidenceIds: [],
    summary: 'exec {"input":{"cmd":"rg files"}}',
  });
  state.row.trail.push({
    ...structuredClone(row.trail[0]),
    callId: "failed-call",
    itemId: undefined,
    evidenceIds: ["failed-ev"],
    summary: "bun test failed-step",
  });
  state.row.evidence.push({
    id: "failed-ev",
    kind: "command",
    callId: "failed-call",
    at: now,
    value: "247 failed",
    exitCode: 1,
  });
  state.row.activity!.lastMessage = "本轮汇报。".repeat(70);
  await ready(page);
  await page.getByRole("button", { name: row.title, exact: true }).click();
  await page.getByRole("button", { name: "完整进度账 →" }).click();
  await expect(
    page.getByRole("button", {
      name: 'exec {"input":{"cmd":"rg files"}}',
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "bun test failed-step", exact: true }),
  ).toContainText("本次执行失败（exit 1）");
  await page.getByRole("button", { name: "展开全部 3 条记录 ↓" }).click();
  await page
    .getByRole("button", {
      name: 'exec {"input":{"cmd":"rg files"}}',
      exact: true,
    })
    .click();
  await page.getByText("原始执行记录", { exact: true }).first().click();
  await expect(
    page.locator("pre").filter({ hasText: '"cmd":"rg files"' }),
  ).toBeVisible();
  await page.getByRole("button", { name: "只看关键步骤 ↑" }).click();
  await expect(
    page.getByRole("button", {
      name: 'exec {"input":{"cmd":"rg files"}}',
      exact: true,
    }),
  ).toHaveCount(0);
  await page.locator("summary").filter({ hasText: "展开汇报" }).click();
  await expect(
    page.locator("details[open]").filter({ hasText: "本轮汇报" }),
  ).toBeVisible();
  await page.screenshot({ path: info.outputPath("优化后的执行详情.png") });
});

test("计划外工作使用真实步骤计数并提供目标归属入口", async ({ page }) => {
  const { state, calls } = await mockApi(page);
  state.row.offPlan = [{ id: "off-1", at: now, callIds: ["call-1"] }];
  await ready(page);
  await page.getByRole("button", { name: "待办", exact: true }).click();
  await expect(page.getByText(/当前最近 24 小时计划外工作 100%/)).toBeVisible();
  await page.getByText("归到目标", { exact: true }).click();
  await page.getByRole("button", { name: row.title, exact: true }).click();
  await page.getByRole("button", { name: "完整进度账 →", exact: true }).click();
  await page.getByRole("combobox", { name: "关联待办" }).selectOption("todo-1");
  await expect.poll(() => calls.find(call => call.path.endsWith("/attribution"))?.data?.workItemId).toBe("todo-1");
});

test("v3 风格回归、固定组件像素对照及功能变更差异记录", async ({
  page,
  browser,
  baseURL,
}, info) => {
  // Render the supplied HTML itself, preserving its template and styles. Only data is normalized.
  const { state, goal, task } = await mockApi(page);
  await page.route("**/api/goals", (r) =>
    r.fulfill({ json: { success: true, data: [] } }),
  );
  await page.route("**/api/work-items", (r) =>
    r.fulfill({ json: { success: true, data: { items: [] } } }),
  );
  await ready(page);
  const reference = await browser.newPage({
    viewport: { width: 1440, height: 1000 },
    deviceScaleFactor: 1,
  });
  const root = resolve("tests/ui/reference");
  const sample = {
    id: 6,
    state: "running",
    title: row.title,
    agent: "Claude Code",
    project: "keepline",
    start: 12 * 60 + 5,
    lastAt: 13 * 60 + 1,
    ask: row.asks[0].text,
    items: ["done", "doing", "todo"],
    now: row.activity?.action,
    ev: "openapi lint · 0 错误 · 成功",
    said: row.activity?.lastMessage,
    ctx: null,
  };
  let html = readFileSync(
    resolve(root, "Keepline App v3.dc.html"),
    "utf8",
  ).replace(
    "  renderVals() {",
    `  data(){return ${JSON.stringify([sample])}}\n  goals(){return []}\n  NAMES = {6:${JSON.stringify(row.items.map((i) => i.title))}};\n  CONSTRAINTS = {};\n  renderVals() {`,
  );
  html = html.replace(
    '<script src="./support.js"></script>',
    `<script>window.__resources={"https://unpkg.com/react@18.3.1/umd/react.production.min.js":"/__reference/react.js","https://unpkg.com/react-dom@18.3.1/umd/react-dom.production.min.js":"/__reference/react-dom.js"}</script><script src="./support.js"></script>`,
  );
  await reference.route("**/__reference/**", async (r) => {
    const path = new URL(r.request().url()).pathname.replace(
      "/__reference/",
      "",
    );
    if (path === "index.html")
      return r.fulfill({ contentType: "text/html", body: html });
    if (path === "react.js" || path === "react-dom.js")
      return r.fulfill({
        contentType: "text/javascript",
        body: readFileSync(
          resolve(
            "node_modules",
            path === "react.js"
              ? "react/umd/react.production.min.js"
              : "react-dom/umd/react-dom.production.min.js",
          ),
        ),
      });
    return r.fulfill({
      contentType: path.endsWith(".css") ? "text/css" : "text/javascript",
      body: readFileSync(resolve(root, path)),
    });
  });
  const fontCss = readFileSync(resolve("src/fonts.css"), "utf8").replaceAll(
    "./assets/",
    `${baseURL}/src/assets/`,
  );
  await reference.route("https://fonts.googleapis.com/**", (r) =>
    r.fulfill({ contentType: "text/css", body: fontCss }),
  );
  await reference.route("https://unpkg.com/@phosphor-icons/**", (r) =>
    r.fulfill({
      contentType: "text/css",
      body: readFileSync(resolve("src/icons.css"), "utf8").replaceAll(
        "./assets/",
        `${baseURL}/src/assets/`,
      ),
    }),
  );
  await reference.goto(`${baseURL}/__reference/index.html`);
  await reference.getByText(row.title, { exact: true }).waitFor();
  await reference.evaluate(() => document.fonts.ready);
  const actualCard = page.getByRole("button", { name: row.title, exact: true }),
    refCard = reference
      .locator('div[role="button"]')
      .filter({ has: reference.getByText(row.title, { exact: true }) });
  const actualBox = await actualCard.boundingBox(),
    refBox = await refCard.boundingBox();
  expect(actualBox?.width).toEqual(refBox?.width);
  expect(await page.locator("nav").boundingBox()).toEqual(
    await reference.locator("nav").boundingBox(),
  );
  expect(await page.locator("header").boundingBox()).toEqual(
    await reference.locator("header").boundingBox(),
  );
  await page.screenshot({ path: info.outputPath("implementation.png") });
  await reference.screenshot({ path: info.outputPath("reference.png") });
  await info.attach("implementation", {
    body: await actualCard.screenshot({ animations: "disabled" }),
    contentType: "image/png",
  });
  await info.attach("reference", {
    body: await refCard.screenshot({ animations: "disabled" }),
    contentType: "image/png",
  });
  const pixelChecks: Array<{
    label: string;
    changed: number;
    width: number;
    height: number;
    exact: boolean;
  }> = [];
  const comparePixels = async (
    actualPng: Buffer,
    referencePng: Buffer,
    label: string,
    exact = true,
  ) => {
    const diff = await page.evaluate(
      async ([a, b]) => {
        const decode = async (s: string) => {
          const image = new Image();
          image.src = "data:image/png;base64," + s;
          await image.decode();
          const canvas = document.createElement("canvas");
          canvas.width = image.width;
          canvas.height = image.height;
          const ctx = canvas.getContext("2d")!;
          ctx.drawImage(image, 0, 0);
          return {
            data: ctx.getImageData(0, 0, image.width, image.height).data,
            width: image.width,
            height: image.height,
          };
        };
        const [actual, reference] = await Promise.all([decode(a), decode(b)]);
        if (
          actual.width !== reference.width ||
          actual.height !== reference.height
        )
          throw new Error(
            `截图尺寸不同: ${actual.width}x${actual.height} / ${reference.width}x${reference.height}`,
          );
        const aa = actual.data,
          bb = reference.data;
        let changed = 0,
          minX = Infinity,
          minY = Infinity,
          maxX = 0,
          maxY = 0;
        const width = actual.width;
        for (let i = 0; i < aa.length; i += 4)
          if (
            aa[i] !== bb[i] ||
            aa[i + 1] !== bb[i + 1] ||
            aa[i + 2] !== bb[i + 2] ||
            aa[i + 3] !== bb[i + 3]
          ) {
            changed++;
            const x = (i / 4) % width,
              y = Math.floor(i / 4 / width);
            minX = Math.min(minX, x);
            minY = Math.min(minY, y);
            maxX = Math.max(maxX, x);
            maxY = Math.max(maxY, y);
          }
        return {
          changed,
          minX,
          minY,
          maxX,
          maxY,
          width,
          height: actual.height,
        };
      },
      [actualPng.toString("base64"), referencePng.toString("base64")],
    );
    pixelChecks.push({
      label,
      exact,
      changed: diff.changed,
      width: diff.width,
      height: diff.height,
    });
    if (exact) expect(diff.changed, `${label}: ${JSON.stringify(diff)}`).toBe(0);
  };
  await comparePixels(
    await actualCard.screenshot({ animations: "disabled" }),
    await refCard.screenshot({ animations: "disabled" }),
    "整张总览卡片与原稿差异（最后活动时间及注意队列已变更）",
    false,
  );
  await comparePixels(
    await page.screenshot({ animations: "disabled" }),
    await reference.screenshot({ animations: "disabled" }),
    "总览整页与原稿差异（最后活动时间及注意队列已变更）",
    false,
  );
  const sideBySide = await browser.newPage({
    viewport: { width: 1440, height: 550 },
  });
  await sideBySide.setContent(
    `<body style="margin:0;background:#111318;color:white;font:14px system-ui"><div style="display:grid;grid-template-columns:1fr 1fr;gap:2px"><section><div style="padding:12px 18px">v3 原稿 · 相同测试数据</div><img style="display:block;width:100%" src="data:image/png;base64,${(await reference.screenshot({ animations: "disabled" })).toString("base64")}"></section><section><div style="padding:12px 18px">修正后的实际应用 · 相同测试数据 · 差异见统计</div><img style="display:block;width:100%" src="data:image/png;base64,${(await page.screenshot({ animations: "disabled" })).toString("base64")}"></section></div></body>`,
  );
  await sideBySide.screenshot({
    path: info.outputPath("总览原稿并排对照.png"),
    fullPage: true,
  });
  await sideBySide.close();
  await comparePixels(
    await page.locator("nav").screenshot(),
    await reference.locator("nav").screenshot(),
    "侧栏与原稿逐像素一致",
  );
  await comparePixels(
    await page.locator("header").screenshot(),
    await reference.locator("header").screenshot(),
    "顶栏与原稿差异（新增最近活动范围筛选）",
    false,
  );
  await actualCard.click();
  await refCard.click();
  await page.screenshot({
    path: info.outputPath("抽屉-implementation.png"),
    animations: "disabled",
  });
  await reference.screenshot({
    path: info.outputPath("抽屉-reference.png"),
    animations: "disabled",
  });
  expect(await page.getByRole("dialog").boundingBox()).toEqual(
    await reference.locator("aside").boundingBox(),
  );
  await comparePixels(
    await page.getByRole("dialog").screenshot(),
    await reference.locator("aside").screenshot(),
    "抽屉与原稿差异（最后活动时间及注意队列已变更）",
    false,
  );
  await page.getByRole("button", { name: "关闭", exact: true }).click();
  await reference.getByRole("button", { name: "关闭", exact: true }).click();
  for (const label of ["详细", "看板", "列表"]) {
    await page.getByRole("button", { name: label, exact: true }).click();
    await reference.getByRole("button", { name: label, exact: true }).click();
    const actual =
      label === "列表"
        ? page.getByRole("button", { name: row.title, exact: false }).last()
        : actualCard;
    const ref =
      label === "列表"
        ? reference
            .getByRole("button")
            .filter({ has: reference.getByText(row.title, { exact: true }) })
        : refCard;
    expect((await actual.boundingBox())?.width).toEqual(
      (await ref.boundingBox())?.width,
    );
    await expect(actual).toContainText(row.title);
    await comparePixels(
      await actual.screenshot({ animations: "disabled" }),
      await ref.screenshot({ animations: "disabled" }),
      `${label}整卡与原稿差异（最后活动时间已变更）`,
      false,
    );
  }
  const initialHtml = html;
  await page.getByRole("button", { name: "网格", exact: true }).click();
  await page.getByRole("button", { name: "紧凑", exact: true }).click();
  await page.getByRole("button", { name: "按项目", exact: true }).click();
  for (const variant of [
    {
      state: "needs_input",
      refState: "needs_input",
      action: "跳到 agent 回答",
    },
    { state: "running", refState: "running", offPlan: true, action: "看偏离" },
    { state: "review", refState: "review", gap: true, action: "生成追问" },
    { state: "review", refState: "review", complete: true, action: "验收 (A)" },
    { state: "accepted", refState: "ended", accepted: true },
    { state: "ended", refState: "ended" },
    { state: "stopped", refState: "running", stalled: true },
  ] as const) {
    state.row = structuredClone(row);
    state.row.state = variant.state;
    if ("offPlan" in variant)
      state.row.offPlan = [
        { id: "off-1", at: row.lastActiveAt, callIds: ["call-1"] },
      ];
    if ("complete" in variant) {
      state.row.items.forEach((item) => {
        item.status = "done";
        item.evidenceIds = ["ev-1"];
      });
      state.row.progress.done = 3;
    }
    const normalized = {
      ...sample,
      ...variant,
      state: variant.refState,
      items: state.row.items.map((item) => item.status),
      ev: "openapi lint · 0 错误 · 成功",
    };
    html = initialHtml.replace(
      JSON.stringify([sample]),
      JSON.stringify([normalized]),
    );
    await ready(page);
    await reference.reload();
    await reference.getByText(row.title, { exact: true }).waitFor();
    await reference.evaluate(() => document.fonts.ready);
    await reference
      .getByRole("button", { name: "按项目", exact: true })
      .click();
    await reference
      .getByRole("heading", { name: "keepline", exact: true })
      .waitFor();
    const actualTag = actualCard
      .locator("span")
      .first()
      .locator("span")
      .first();
    const referenceTag = refCard
      .locator("span")
      .first()
      .locator("span")
      .first();
    const palette = (el: HTMLElement) => {
      const style = getComputedStyle(el);
      return { background: style.backgroundColor, color: style.color };
    };
    expect(await actualTag.evaluate(palette)).toEqual(
      await referenceTag.evaluate(palette),
    );
    await comparePixels(
      await actualCard.screenshot({ animations: "disabled" }),
      await refCard.screenshot({ animations: "disabled" }),
      `${variant.state}${"gap" in variant ? "有缺口" : ""}状态整卡与原稿差异（最后活动时间已变更）`,
      false,
    );
    if ("action" in variant)
      await expect(
        actualCard.getByRole("button", { name: variant.action, exact: true }),
      ).toBeVisible();
  }
  // A complete page with multiple simultaneous states catches row heights, wrapping and group spacing.
  const multiRows = [
    "needs_input",
    "running",
    "review",
    "running",
    "running",
    "accepted",
    "ended",
    "stopped",
  ].map((status, i) => {
    const r = structuredClone(row);
    r.sessionId = `alignment-${i}`;
    r.title = `${["备份位置等待确认", "增量扫描偏离排查", "导航重构待核对", "生成 OpenAPI 文档", "桌面版识别运行中", "追加要求已验收", "列表闪烁修复结束", "数据库迁移已停下"][i]}`;
    r.runtimeId = i % 2 ? "claude-code" : "codex";
    r.state = status as typeof row.state;
    r.lastActiveAt = new Date(Date.parse(now) - (i + 1) * 60000).toISOString();
    if (i === 1)
      r.offPlan = [
        { id: "alignment-off", at: r.lastActiveAt, callIds: ["call-1"] },
      ];
    if (i === 4 || i === 5) {
      r.items.forEach((item) => {
        item.status = "done";
        item.evidenceIds = ["ev-1"];
      });
      r.progress.done = 3;
    }
    return r;
  });
  const multiSamples = multiRows.map((r, i) => ({
    ...sample,
    id: 100 + i,
    title: r.title,
    agent: r.runtimeId === "codex" ? "Codex" : "Claude Code",
    state:
      r.state === "accepted" || r.state === "ended"
        ? "ended"
        : r.state === "stopped"
          ? "running"
          : r.state,
    accepted: r.state === "accepted",
    stalled: r.state === "stopped",
    offPlan: i === 1,
    gap: i === 2,
    lastAt: 13 * 60 + 2 - (i + 1),
    items: r.items.map((item) => item.status),
    action:
      i === 0
        ? "跳到 agent 回答"
        : i === 1
          ? "看偏离"
          : i === 2
            ? "生成追问"
            : null,
  }));
  await page.route("**/api/ledger?*", (r) =>
    r.fulfill({ json: { success: true, data: multiRows } }),
  );
  html = initialHtml.replace(
    JSON.stringify([sample]),
    JSON.stringify(multiSamples),
  );
  await page.mouse.move(0, 0);
  await reference.mouse.move(0, 0);
  await page.getByRole("button", { name: "按紧急度", exact: true }).click();
  await page.goto("/");
  await page
    .getByRole("button", { name: multiRows[0].title, exact: true })
    .waitFor();
  await page.evaluate(() => document.fonts.ready);
  await reference.reload();
  await reference.getByText(multiRows[0].title, { exact: true }).waitFor();
  await reference.evaluate(() => document.fonts.ready);
  await page.screenshot({
    path: info.outputPath("多状态总览-implementation.png"),
    animations: "disabled",
  });
  await reference.screenshot({
    path: info.outputPath("多状态总览-reference.png"),
    animations: "disabled",
  });
  await comparePixels(
    await page.screenshot({ animations: "disabled" }),
    await reference.screenshot({ animations: "disabled" }),
    "八会话多状态总览整页与原稿差异（最后活动时间及注意队列已变更）",
    false,
  );
  const multiComparison = await browser.newPage({
    viewport: { width: 1440, height: 550 },
  });
  await multiComparison.setContent(
    `<body style="margin:0;background:#111318;color:white;font:14px system-ui"><div style="display:grid;grid-template-columns:1fr 1fr;gap:2px"><section><div style="padding:12px 18px">v3 原稿 · 相同测试数据</div><img style="display:block;width:100%" src="data:image/png;base64,${(await reference.screenshot({ animations: "disabled" })).toString("base64")}"></section><section><div style="padding:12px 18px">修正后的实际应用 · 相同测试数据 · 差异见统计</div><img style="display:block;width:100%" src="data:image/png;base64,${(await page.screenshot({ animations: "disabled" })).toString("base64")}"></section></div></body>`,
  );
  await multiComparison.screenshot({
    path: info.outputPath("多状态总览原稿并排对照.png"),
    fullPage: true,
  });
  await multiComparison.close();
  await page.unroute("**/api/ledger?*");
  // Match reference data; retain its original template and styles.
  state.row = structuredClone(row);
  await page.route("**/api/goals", (r) =>
    r.fulfill({ json: { success: true, data: [goal] } }),
  );
  await page.route("**/api/work-items", (r) =>
    r.fulfill({ json: { success: true, data: { items: [task] } } }),
  );
  const referenceGoal = {
    id: goal.id,
    title: goal.title,
    outcome: goal.outcome,
    weekly: 0,
    todos: [{ title: task.title, check: [0] }],
    suggest: [],
  };
  html = initialHtml.replace(
    "goals(){return []}",
    `goals(){return ${JSON.stringify([referenceGoal])}}`,
  );
  const sections = [
    {
      label: "还挂着",
      count: 1,
      rows: [
        {
          title: row.title,
          meta: "Claude Code",
          detail: "还差：检查 lint、运行测试",
          fg: "#6B7280",
          font: "inherit",
          border: "0",
          actions: ["复制续做提示", "带到明天"],
        },
      ],
    },
    { label: "已验收", count: 0, rows: [] },
    { label: "今天的偏离和纠正", count: 0, rows: [] },
  ];
  html = html
    .replace(
      /const reviewSections = \[[\s\S]*?\];\n    const tog/,
      `const reviewSections = ${JSON.stringify(sections)};\n    const tog`,
    )
    .replace(
      "14 个会话结束 · 9 个已验收 · 3 个还挂着 · 2 次偏离",
      "0 个已验收 · 1 个还挂着 · 0 次偏离",
    );
  await ready(page);
  await reference.reload();
  await reference.getByText(row.title, { exact: true }).waitFor();
  await reference.evaluate(() => document.fonts.ready);
  for (const label of ["待办", "目标", "回顾", "设置"]) {
    if (label === "设置") {
      // Capture all four settings sections in one shared viewport.
      await page.setViewportSize({ width: 1440, height: 1200 });
      await reference.setViewportSize({ width: 1440, height: 1200 });
    }
    await page
      .getByRole("button", { name: label, exact: true })
      .first()
      .click();
    await reference
      .getByRole("button")
      .filter({ has: reference.getByText(label, { exact: true }) })
      .first()
      .click();
    await page.screenshot({
      path: info.outputPath(`${label}-implementation.png`),
    });
    await reference.screenshot({
      path: info.outputPath(`${label}-reference.png`),
    });
    if (label === "待办") {
      const actual = page.locator("section").filter({
        has: page.getByRole("heading", { name: goal.title, exact: true }),
      });
      const expected = reference.locator("section").filter({
        has: reference.getByRole("heading", {
          name: goal.title,
          exact: true,
        }),
      });
      expect(await actual.boundingBox()).toEqual(await expected.boundingBox());
      await comparePixels(
        await actual.screenshot(),
        await expected.screenshot(),
        "待办分组与原稿逐像素一致",
      );
      await comparePixels(
        await page
          .getByRole("button", { name: "＋ 加待办", exact: true })
          .screenshot(),
        await reference
          .getByRole("button", { name: "＋ 加待办", exact: true })
          .screenshot(),
        "添加待办按钮与原稿逐像素一致",
      );
    } else if (label === "目标") {
      // The independently updated project map has no matching v3 reference.
      // Preserve that feature and explicitly exclude it from pixel claims.
      await expect(page.getByRole("region", { name: `项目进度图：${goal.title}`, exact: true })).toHaveCount(0);
      await page.getByRole("button", { name: "查看项目进度图", exact: true }).click();
      await expect(page.getByRole("region", { name: `项目进度图：${goal.title}`, exact: true })).toBeVisible();
    } else {
      for (const title of label === "设置"
        ? ["通知", "偏离检测", "AI 判断", "待核对与保留"]
        : ["还挂着"]) {
        const actual = page.locator("section").filter({
          has: page.getByRole("heading", { name: title, exact: true }),
        });
        const expected = reference.locator("section").filter({
          has: reference.getByRole("heading", { name: title, exact: true }),
        });
        expect(await actual.boundingBox()).toEqual(
          await expected.boundingBox(),
        );
        await comparePixels(
          await actual.screenshot(),
          await expected.screenshot(),
          `${label} ${title}与原稿${label === "设置" ? "文案差异" : "逐像素一致"}`,
          label !== "设置",
        );
      }
      if (label === "回顾") {
        const actual = page
          .getByRole("button", { name: "导出 Markdown", exact: true })
          .locator("..");
        const expected = reference
          .getByRole("button", { name: "导出 Markdown", exact: true })
          .locator("..");
        expect(await actual.boundingBox()).toEqual(
          await expected.boundingBox(),
        );
        await expect(actual.locator("summary")).toContainText("▾");
      }
    }
  }
  // The supplied Agent Panel is still v2; verify the complete detail adopts v3 while preserving its operations.
  await page.setViewportSize({ width: 1440, height: 1000 });
  await ready(page);
  await actualCard.click();
  await page.getByRole("button", { name: "完整进度账 →", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "时间线", exact: true }),
  ).toBeVisible();
  const panelStyle = await page.locator("main").evaluate((el) => ({
    font: getComputedStyle(el).fontFamily,
    background: getComputedStyle(el).backgroundColor,
  }));
  expect(panelStyle.font).toContain("Geist");
  expect(panelStyle.background).toBe("rgb(247, 248, 250)");
  await expect(
    page.getByRole("button", { name: "编辑要求", exact: true }),
  ).toBeVisible();
  await page.screenshot({
    path: info.outputPath("完整详情-v3.png"),
    animations: "disabled",
  });
  writeFileSync(
    info.outputPath("像素对照结果.json"),
    JSON.stringify(pixelChecks, null, 2),
  );
  await reference.close();
});

test("推送更新悬停中的卡片与已打开详情，重连后补齐内容，无需刷新", async ({ page }) => {
  const { state } = await mockApi(page);
  let socket: { send: (data: string) => void } | undefined;
  await page.routeWebSocket(/\/ws\?/, (ws) => {
    socket = ws;
    ws.send(JSON.stringify({ type: "connected", timestamp: now }));
  });
  await ready(page);
  const card = page.getByRole("button", { name: row.title, exact: true });
  await card.hover();
  state.row.activity = { ...state.row.activity!,action: "bun test realtime-card" };
  socket!.send(JSON.stringify({ type: "ledger:update", data: {}, timestamp: now }));
  await page.clock.runFor(250);
  await expect(card).toContainText("bun test realtime-card");
  await card.click();
  const drawer = page.getByRole("dialog");
  await expect(drawer).toBeVisible();
  state.row.activity = { ...state.row.activity!,lastMessage: "新证据已经实时显示" };
  state.row.progress.done = 2;
  state.row.items[1].status = "done";
  socket!.send(JSON.stringify({ type: "ledger:update", data: {}, timestamp: now }));
  await page.clock.runFor(250);
  await expect(drawer).toContainText("新证据已经实时显示");
  await expect(drawer).toContainText("2 / 3");
  // The manager emits connected after every reconnect; missed state must be re-fetched.
  state.row.activity.lastMessage = "重连期间错过的内容已补齐";
  socket!.send(JSON.stringify({ type: "connected", timestamp: now }));
  await page.clock.runFor(250);
  await expect(drawer).toContainText("重连期间错过的内容已补齐");
});

test("推送不重复读取设置和目标，后台暂停请求，回到前台补齐", async ({ page }) => {
  const { state, calls } = await mockApi(page);
  let socket: { send: (data: string) => void } | undefined;
  await page.routeWebSocket(/\/ws\?/, ws => {
    socket = ws;
  });
  await ready(page);
  await expect(page.getByRole("status", { name: "同步状态" })).toContainText("已连接");
  await page.clock.runFor(500);
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => {
    document.documentElement.dataset.pushCount = "0";
    const manager = (window as unknown as { __wsManager: { onMessage: (fn: (m: { type: string }) => void) => void } }).__wsManager;
    manager.onMessage(m => {
      if (m.type === "ledger:update") document.documentElement.dataset.pushCount = String(Number(document.documentElement.dataset.pushCount) + 1);
    });
  });
  const start = calls.length;
  for (let i = 0; i < 20; i++) socket!.send(JSON.stringify({ type: "ledger:update", data: {}, timestamp: now }));
  await expect(page.locator("html")).toHaveAttribute("data-push-count", "20");
  await page.clock.runFor(500);
  await page.waitForLoadState("networkidle");
  await expect.poll(() => calls.slice(start).filter(c => c.path === "/api/ledger").length).toBe(1);
  expect(calls.slice(start).filter(c => ["/api/settings/ledger", "/api/goals", "/api/work-items"].includes(c.path))).toHaveLength(0);
  const refreshStart = calls.length;
  await page.getByRole("button", { name: "更多", exact: true }).click();
  await page.getByRole("menuitem", { name: "刷新数据", exact: true }).click();
  await expect.poll(() => calls.slice(refreshStart).filter(c => c.path === "/api/ledger").length).toBe(1);
  await page.waitForLoadState("networkidle");
  expect(calls.slice(refreshStart).filter(c => c.path === "/api/ledger")).toHaveLength(1);
  const card = page.getByRole("button", { name: row.title, exact: true });
  await card.click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "hidden" });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  const hiddenStart = calls.length;
  state.row.activity = { ...state.row.activity!, lastMessage: "后台期间追加的证据" };
  socket!.send(JSON.stringify({ type: "ledger:update", timestamp: now }));
  await page.clock.runFor(31000);
  expect(calls.slice(hiddenStart)).toHaveLength(0);
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "visible" });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await page.clock.runFor(500);
  await expect(page.getByRole("dialog")).toContainText("后台期间追加的证据");
  await expect(page.getByRole("status", { name: "同步状态" })).toContainText("已连接 · 上次同步");
});

test("详情请求乱序时较晚返回的旧内容不会覆盖新内容", async ({ page }) => {
  const { state } = await mockApi(page);
  let socket: { send: (data: string) => void } | undefined;
  await page.routeWebSocket(/\/ws\?/, ws => {
    socket = ws;
    ws.send(JSON.stringify({ type: "connected", timestamp: now }));
  });
  await ready(page);
  await page.getByRole("button", { name: row.title, exact: true }).click();
  const drawer = page.getByRole("dialog");
  await expect(drawer).toBeVisible();
  let release: (() => Promise<void>) | undefined;
  let held = false;
  await page.route(`**/api/ledger/${row.sessionId}`, async route => {
    if (held) return route.fallback();
    held = true;
    const old = structuredClone(state.row);
    old.activity = { ...old.activity!, lastMessage: "过期的汇报不能覆盖新汇报" };
    release = () => route.fulfill({ json: { success: true, data: old } });
  });
  socket!.send(JSON.stringify({ type: "ledger:update", timestamp: now }));
  await page.clock.runFor(500);
  await expect.poll(() => held).toBe(true);
  state.row.activity = { ...state.row.activity!, lastMessage: "最新汇报必须保留" };
  socket!.send(JSON.stringify({ type: "ledger:update", timestamp: now }));
  await page.clock.runFor(500);
  await expect(drawer).toContainText("最新汇报必须保留");
  await release!();
  // Confirm the delayed response has reached the hook before asserting the final state.
  await page.waitForLoadState("networkidle");
  await expect(drawer).toContainText("最新汇报必须保留");
  await expect(drawer).not.toContainText("过期的汇报不能覆盖新汇报");
});

test("WebSocket 没有心跳回应时自动重连并补齐数据", async ({ page }) => {
  const { state } = await mockApi(page);
  let connections = 0;
  await page.routeWebSocket(/\/ws\?/, ws => {
    connections++;
    ws.send(JSON.stringify({ type: "connected", timestamp: now }));
    // Deliberately omit pong to simulate an apparently open but dead connection.
  });
  await ready(page);
  await page.clock.runFor(500);
  await page.waitForLoadState("networkidle");
  const initialConnections = connections;
  state.row.activity = { ...state.row.activity!, action: "心跳恢复后读取的新动作" };
  await page.clock.runFor(46000);
  await expect.poll(() => connections).toBeGreaterThan(initialConnections);
  await page.clock.runFor(500);
  await expect(page.getByRole("button", { name: row.title, exact: true })).toContainText("心跳恢复后读取的新动作");
});

test("列表请求乱序不会倒退，正常心跳保持同一连接", async ({ page }) => {
  const { state } = await mockApi(page);
  let connections = 0;
  let pongs = 0;
  let socket: { send: (data: string) => void } | undefined;
  await page.routeWebSocket(/\/ws\?/, ws => {
    connections++;
    socket = ws;
    ws.onMessage(data => {
      if (JSON.parse(String(data)).type === "ping") {
        pongs++;
        ws.send(JSON.stringify({ type: "pong", timestamp: now }));
      }
    });
  });
  await ready(page);
  await page.clock.runFor(500);
  await page.waitForLoadState("networkidle");
  const initialConnections = connections;
  let release: (() => Promise<void>) | undefined;
  let held = false;
  await page.route(url => url.pathname === "/api/ledger", async route => {
    if (held) return route.fallback();
    held = true;
    const old = structuredClone(state.row);
    old.activity = { ...old.activity!, action: "过期的列表动作" };
    release = () => route.fulfill({ json: { success: true, data: [old] } });
  });
  socket!.send(JSON.stringify({ type: "ledger:update", timestamp: now }));
  await page.clock.runFor(500);
  await expect.poll(() => held).toBe(true);
  state.row.activity = { ...state.row.activity!, action: "最新的列表动作" };
  await page.clock.runFor(30000);
  const card = page.getByRole("button", { name: row.title, exact: true });
  await expect(card).toContainText("最新的列表动作");
  await release!();
  await page.waitForLoadState("networkidle");
  await expect(card).not.toContainText("过期的列表动作");
  await expect.poll(() => pongs).toBeGreaterThan(0);
  await page.waitForFunction(() => (window as unknown as { __wsManager: { pongTimeout: unknown } }).__wsManager.pongTimeout === null);
  await page.clock.runFor(16000);
  expect(connections).toBe(initialConnections);
});
