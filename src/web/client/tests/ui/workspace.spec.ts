import { test, expect, type Page } from "@playwright/test";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { now, row, settings, monitor } from "./fixtures";

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
    children: [] as Array<typeof row>,
    settings: structuredClone(settings),
    failSettings: false,
    failReview: false,
    monitor: structuredClone(monitor),
    failMonitor: false,
    failInstallHooks: false,
    outside: [] as Array<Pick<typeof row, 'sessionId' | 'title' | 'projectRoot' | 'state' | 'progress' | 'offPlan' | 'lastActiveAt'>>,
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
    inactiveDays: 0,
    recent: [],
  };
  await page.route("**/api/**", async (route) => {
    const req = route.request(),
      u = new URL(req.url()),
      path = u.pathname,
      method = req.method(),
      data = req.postDataJSON() as Record<string, unknown> | null;
    calls.push({ path, method, data });
    if (path === "/api/status/monitor" && state.failMonitor)
      return route.fulfill({ status: 500, json: { success: false, error: "模拟监测状态读取失败" } });
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
    else if (path === "/api/status/monitor") result = state.monitor;
    else if (path === "/api/status/hooks/install") {
      if (state.failInstallHooks) return route.fulfill({ status: 500, json: { success: false, error: "Hook 安装失败" } });
      state.monitor.runtimes.forEach(runtime => { runtime.hookInstalled = true; runtime.receiverRunning = true; runtime.lastHookAt = undefined; });
      result = { installed: true };
    }
    else if (path === "/api/ledger/outside-attention") result = state.outside;
    else if (path === "/api/settings/ledger") {
      if (method === "PUT") state.settings = data as unknown as typeof settings;
      result = state.settings;
    } else if (path === "/api/goals") result = [goal];
    else if (path === "/api/goals/todos") result = goal.todos;
    else if (path === "/api/work-items")
      result = method === "GET" ? { items: [task] } : { ...task, ...data };
    else if (path === "/api/ledger/review")
      result = {
        start: new Date(now).toISOString(),
        end: new Date(Date.parse(now) + 86400000).toISOString(),
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
    } else if (state.children.some(child => path.startsWith(`/api/ledger/${child.sessionId}`)))
      result = state.children.find(child => path.startsWith(`/api/ledger/${child.sessionId}`));
    else if (path.startsWith(`/api/ledger/${row.sessionId}`))
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

test("连接实时事件区分配置安装与真实事件，安装失败可重试", async ({ page }) => {
  const { state, calls } = await mockApi(page);
  state.monitor.runtimes.forEach(runtime => { runtime.hookInstalled = false; runtime.lastHookAt = undefined; });
  await ready(page);
  await page.getByRole("button", { name: "设置", exact: false }).first().click();
  expect(calls.some(call => call.path === "/api/status/hooks/install")).toBe(false);
  state.failInstallHooks = true;
  await page.getByRole("button", { name: "连接实时事件", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Hook 安装失败");
  state.failInstallHooks = false;
  await page.getByRole("button", { name: "连接实时事件", exact: true }).click();
  await expect(page.getByRole("button", { name: "重新连接", exact: true })).toBeEnabled();
  await expect(page.getByText("待验证", { exact: true })).toHaveCount(2);
  await expect(page.getByText(/配置已安装，尚未收到事件/)).toHaveCount(2);
  state.monitor.runtimes[0].lastHookAt = now;
  await page.clock.runFor(15000);
  await expect(page.getByText("已验证", { exact: true })).toHaveCount(1);
});

test("保存的时间范围先适配服务器保留期，缩短保留期后的刷新也使用新上限", async ({ page }) => {
  const { state, calls } = await mockApi(page);
  state.settings.retentionDays = 7;
  await page.addInitScript(() => localStorage.setItem("keepline.overview-hours", "720"));
  const requests: Array<{ hours: number; limit: number }> = [];
  await page.route("**/api/ledger?*", route => {
    const hours = Number(new URL(route.request().url()).searchParams.get("hours"));
    const limit = state.settings.retentionDays * 24;
    requests.push({ hours, limit });
    return route.fulfill({ status: hours <= limit ? 200 : 400, json: hours <= limit ? { success: true, data: [state.row] } : { success: false, error: "hours exceeds retention" } });
  });
  await ready(page);
  await expect(page.getByRole("spinbutton", { name: "最近活动小时数" })).toHaveValue("168");
  expect(requests[0]).toEqual({ hours: 168, limit: 168 });
  await page.getByRole("button", { name: "设置", exact: false }).first().click();
  await page.getByText("7 天", { exact: true }).click();
  await page.getByRole("spinbutton", { name: "记录保留天数" }).fill("1");
  await page.getByRole("heading", { name: "待核对与保留", exact: true }).click();
  await expect.poll(() => calls.some(call => call.path === "/api/settings/ledger" && call.method === "PUT" && call.data?.retentionDays === 1)).toBe(true);
  await page.getByRole("button", { name: "总览", exact: true }).click();
  await expect(page.getByRole("spinbutton", { name: "最近活动小时数" })).toHaveValue("24");
  expect(requests.every(request => request.hours <= request.limit)).toBe(true);
  expect(await page.evaluate(() => localStorage.getItem("keepline.overview-hours"))).toBe("24");
});

test("首次设置读取失败后手动刷新恢复，首个账本请求仍先限制保留期", async ({ page }) => {
  const { state } = await mockApi(page);
  state.settings.retentionDays = 7;
  await page.addInitScript(() => localStorage.setItem("keepline.overview-hours", "720"));
  let reads = 0;
  await page.route("**/api/settings/ledger", route => {
    reads++;
    return route.fulfill(reads === 1
      ? { status: 503, json: { success: false, error: "设置暂时不可用" } }
      : { json: { success: true, data: state.settings } });
  });
  const hours: number[] = [];
  await page.route("**/api/ledger?*", route => {
    hours.push(Number(new URL(route.request().url()).searchParams.get("hours")));
    return route.fulfill({ json: { success: true, data: [state.row] } });
  });
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText("设置暂时不可用");
  expect(hours).toEqual([]);
  await page.getByRole("button", { name: "更多", exact: true }).click();
  await page.getByRole("menuitem", { name: "刷新数据", exact: true }).click();
  await expect(page.getByRole("button", { name: row.title, exact: true })).toBeVisible();
  await expect(page.getByRole("spinbutton", { name: "最近活动小时数" })).toHaveValue("168");
  expect(hours.every(value => value === 168)).toBe(true);
  expect(reads).toBe(2);
  await page.getByRole("button", { name: "更多", exact: true }).click();
  await page.getByRole("menuitem", { name: "刷新数据", exact: true }).click();
  await expect.poll(() => hours.length).toBeGreaterThan(1);
  expect(reads).toBe(2);
});

test("编辑待办时插入和重排清单不会将已完成 ID 转给新要求", async ({ page }) => {
  const { task, goal, calls } = await mockApi(page);
  task.acceptance = [{ id: "api", text: "测试 API", completed: true }, { id: "ui", text: "测试 UI", completed: false }];
  goal.todos[0].acceptance = task.acceptance;
  await ready(page);
  await page.getByRole("button", { name: "待办", exact: true }).click();
  await page.getByRole("button", { name: task.title, exact: true }).click();
  await page.getByLabel("验收清单（每行一项）").fill("新的安全检查\n测试 UI\n测试 API");
  await page.getByRole("button", { name: "保存", exact: true }).click();
  await expect.poll(() => calls.some(call => call.path === `/api/work-items/${task.id}` && call.method === "PATCH")).toBe(true);
  const saved = calls.find(call => call.path === `/api/work-items/${task.id}` && call.method === "PATCH")!.data!.acceptance as Array<{ id: string; text: string; completed: boolean }>;
  expect(saved.slice(1)).toEqual([task.acceptance[1], task.acceptance[0]]);
  expect(saved[0]).toMatchObject({ text: "新的安全检查", completed: false });
  expect(["api", "ui"]).not.toContain(saved[0].id);
});

test("按目标分组保留目标名称且从进度面板跳到 agent 后清理旧详情 URL", async ({ page }) => {
  const { state, task, goal } = await mockApi(page);
  state.row.workItemId = task.id;
  await ready(page);
  await page.getByRole("button", { name: "按目标", exact: true }).click();
  await expect(page.getByRole("heading", { name: goal.title, exact: true })).toBeVisible();
  await page.goto(`/?view=overview&sessionId=${row.sessionId}&anchor=item-1&detail=full`);
  const jump = page.getByRole("button", { name: "查看会话记录", exact: true }).last();
  await expect(jump).toBeVisible();
  await jump.click();
  await expect.poll(() => new URL(page.url()).searchParams.get("view")).toBe("sessions");
  const params = new URL(page.url()).searchParams;
  expect(params.get("sessionId")).toBe(row.sessionId);
  for (const key of ["anchor", "detail"]) expect(params.has(key)).toBe(false);
});

for (const layout of ["网格", "列表", "看板"]) test(`上下文低于 80% 仍显示圆环，悬停和聚焦展示用量 · ${layout}`, async ({ page }, info) => {
  const { state } = await mockApi(page);
  state.row.contextUsage = { usedTokens: 749600, windowTokens: 1000000, at: now };
  await ready(page);
  await page.getByRole("button", { name: layout, exact: true }).click();
  const context = page.getByRole("progressbar", { name: "上下文占用" });
  await expect(context).toBeVisible();
  await expect(context).toHaveAttribute("aria-valuenow", "75");
  await expect(page.getByRole("tooltip")).toBeHidden();
  await context.hover();
  await expect(page.getByRole("tooltip")).toHaveText("上下文 749.6k / 1M (75%)");
  await expect(page.getByRole("tooltip")).toBeVisible();
  await page.screenshot({ path: info.outputPath("context-ring.png"), animations: "disabled" });
  await page.mouse.move(0, 0);
  await expect(page.getByRole("tooltip")).toBeHidden();
  await context.focus();
  await expect(page.getByRole("tooltip")).toBeVisible();
});

for (const usedTokens of [0, undefined]) test(`上下文区分零占用与未获取 · ${usedTokens ?? "未知"}`, async ({ page }) => {
  const { state } = await mockApi(page);
  state.row.contextUsage = usedTokens === undefined ? undefined : { usedTokens, windowTokens: 1000000, at: now };
  await ready(page);
  const context = page.getByRole("progressbar", { name: "上下文占用" });
  await expect(context).toBeVisible();
  await expect(context).toContainText(usedTokens === undefined ? "上下文 —" : "上下文 0%");
  if (usedTokens === undefined) await expect(context).not.toHaveAttribute("aria-valuenow");
  else await expect(context).toHaveAttribute("aria-valuenow", "0");
  await context.hover();
  await expect(page.getByRole("tooltip")).toHaveText(usedTokens === undefined ? "上下文用量暂未获取" : "上下文 0 / 1M (0%)");
});

for (const width of [1440,390]) test(`原稿紧凑布局展示真实上下文、停滞天数和可处理的推断待办 · ${width}`, async ({ page },info) => {
  await page.setViewportSize({ width,height: 1000 });
  const { state,goal,task,calls } = await mockApi(page);
  state.row.contextUsage = { usedTokens: 9100,windowTokens: 10000,at: now };
  state.row.workItemId = task.id;
  state.row.followUpSuggestions = ['补充恢复幂等测试'];
  state.row.subagents = [{ ...structuredClone(row),sessionId: 'reviewer-child',title: 'reviewer（审查）',agentRole: 'reviewer',followUpSuggestions: ['添加恢复接口示例'] }];
  goal.stale = true;
  goal.inactiveDays = 9;
  const created: Array<typeof task> = [];
  await page.route('**/api/work-items',async route => {
    const req = route.request();
    if (req.method() === 'POST') {
      const data = req.postDataJSON();
      calls.push({ path: '/api/work-items',method: 'POST',data });
      const item = { ...task,...data,id: 'suggested-todo' };
      created.push(item);
      goal.todos.push({ ...item,readyToComplete: false,checklist: [],sessions: [] });
      return route.fulfill({ json: { success: true,data: { item } } });
    }
    return route.fulfill({ json: { success: true,data: { items: [task,...created] } } });
  });
  await ready(page);
  await expect(page.getByRole('progressbar',{ name: '上下文占用' })).toHaveAttribute('aria-valuenow','91');
  await page.getByRole('button',{ name: row.title,exact: true }).click();
  const drawer = page.getByRole('dialog');
  await expect(drawer.getByText('91% 已用 · 接近上限，长任务可能被截断',{ exact: true })).toBeVisible();
  await expect(drawer.getByText('先把 recovery 路由生成出来',{ exact: true })).toBeVisible();
  await drawer.screenshot({ path: info.outputPath('compact-drawer.png'),animations: 'disabled' });
  await drawer.getByRole('button',{ name: '关闭',exact: true }).click();
  await page.getByRole('button',{ name: '目标',exact: true }).click();
  await expect(page.getByText('▲ 9 天没推进',{ exact: true })).toBeVisible();
  await expect(page.getByText(/深蓝已验收，浅蓝进行中/)).toBeVisible();
  await page.getByRole('button',{ name: /待办/ }).first().click();
  const inferred = page.locator('[class*="todoSuggestion"]').filter({ hasText: '补充恢复幂等测试' });
  await inferred.getByRole('button',{ name: '加为待办',exact: true }).click();
  const editor = page.getByRole('dialog',{ name: '编辑目标或待办' });
  await expect(editor.getByRole('textbox',{ name: '标题',exact: true })).toHaveValue('补充恢复幂等测试');
  await expect(editor.getByRole('combobox',{ name: '目标',exact: true })).toHaveValue(goal.id);
  await editor.getByRole('button',{ name: '取消',exact: true }).click();
  await expect(inferred).toBeVisible();
  await page.getByRole('button',{ name: '忽略建议：添加恢复接口示例',exact: true }).click();
  await page.reload();
  await expect(page.locator('[class*="todoSuggestion"]').filter({ hasText: '添加恢复接口示例' })).toHaveCount(0);
  await inferred.getByRole('button',{ name: '加为待办',exact: true }).click();
  await editor.getByRole('button',{ name: '保存',exact: true }).click();
  await expect(editor).toHaveCount(0);
  await expect(inferred).toHaveCount(0);
  expect(calls.find(call => call.path === '/api/work-items' && call.method === 'POST')?.data).toMatchObject({ title: '补充恢复幂等测试',parentId: goal.id,projectRoot: row.projectRoot });
  await page.screenshot({ path: info.outputPath('inferred-todo-saved.png'),animations: 'disabled' });
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

for (const width of [1440, 390]) {
test(`紧凑卡片显示子任务标签，抽屉支持父子 thread 跳转 · ${width}`, async ({ page }, info) => {
  await page.setViewportSize({ width, height: 1000 });
  const { state } = await mockApi(page);
  const child = { ...structuredClone(row), sessionId: 'codex_child', title: 'you-are-here · Chandrasekhar', runtimeId: 'codex', parentSessionId: row.sessionId, isSubAgent: true, agentRole: 'you-are-here', agentNickname: 'Chandrasekhar', subagents: undefined };
  state.children.push(child);
  state.row.subagents = [child];
  await ready(page);
  const card = page.getByRole('button', { name: row.title, exact: true });
  await expect(card.getByText('子任务 1', { exact: true })).toBeVisible();
  await card.click();
  let drawer = page.getByRole('dialog');
  await expect(drawer.getByRole('region', { name: '子任务列表' }).getByText('子任务', { exact: true })).toBeVisible();
  await drawer.getByRole('button', { name: `查看子任务 ${child.title}`, exact: true }).click();
  drawer = page.getByRole('dialog', { name: `会话详情：${child.title}`, exact: true });
  await expect(drawer.getByText('子任务', { exact: true })).toBeVisible();
  await expect(drawer.getByRole('heading', { name: child.title, exact: true })).toBeVisible();
  await page.screenshot({ path: info.outputPath('codex-child-thread.png'), fullPage: true, animations: 'disabled' });
  await drawer.getByRole('button', { name: '查看父会话', exact: true }).click();
  await expect(page.getByRole('dialog', { name: `会话详情：${row.title}`, exact: true })).toBeVisible();
  await page.getByRole('button', { name: '关闭', exact: true }).click();
  await page.getByRole('button', { name: '列表', exact: true }).click();
  await expect(page.getByRole('button', { name: row.title, exact: true }).getByText('子任务 1', { exact: true })).toBeVisible();
});
}

test("父 thread 未出现时，独立显示的子任务仍有专属标签", async ({ page }) => {
  const { state } = await mockApi(page);
  state.row.isSubAgent = true;
  state.row.parentSessionId = 'codex_missing-parent';
  await ready(page);
  await expect(page.getByRole('button', { name: row.title, exact: true }).getByText('子任务', { exact: true })).toBeVisible();
});

test("首次加载显示总览和详情骨架，详情等待时可关闭且不会被迟到响应重开", async ({ page }, info) => {
  await mockApi(page);
  let releaseOverview!: () => void;
  let releaseDetail!: () => void;
  const overviewReady = new Promise<void>(resolve => { releaseOverview = resolve; });
  const detailReady = new Promise<void>(resolve => { releaseDetail = resolve; });
  await page.route("**/api/ledger?*", async route => { await overviewReady; await route.fallback(); });
  await page.route(`**/api/ledger/${row.sessionId}`, async route => { await detailReady; await route.fallback(); });
  await page.goto(`/?sessionId=${row.sessionId}`);
  const overview = page.getByRole("status", { name: "正在加载会话", exact: true });
  const drawer = page.getByRole("dialog", { name: "正在加载会话详情", exact: true });
  await expect(overview).toBeVisible();
  await expect(drawer).toBeVisible();
  await expect(page.getByText("正在加载最近 24 小时的会话…", { exact: true })).toBeVisible();
  await expect(drawer.getByRole("button", { name: "关闭", exact: true })).toBeEnabled();
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.clock.runFor(32);
  expect(await overview.locator('[aria-hidden="true"][style]').first().evaluate(el => getComputedStyle(el).animationName)).toBe("none");
  await page.screenshot({ path: info.outputPath("loading-desktop.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(drawer.getByRole("button", { name: "关闭", exact: true })).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: info.outputPath("loading-mobile.png") });
  await page.setViewportSize({ width: 1440, height: 1000 });
  releaseOverview();
  await expect(overview).toHaveCount(0);
  await expect(page.getByRole("button", { name: row.title, exact: true })).toBeVisible();
  await expect(drawer).toBeVisible();
  await drawer.getByRole("button", { name: "关闭", exact: true }).click();
  await expect(drawer).toHaveCount(0);
  const response = page.waitForResponse(r => new URL(r.url()).pathname === `/api/ledger/${row.sessionId}`);
  releaseDetail();
  await response;
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("button", { name: row.title, exact: true }).click();
  await expect(page.getByRole("dialog", { name: `会话详情：${row.title}`, exact: true })).toBeVisible();
  await expect(page.getByRole("dialog", { name: "正在加载会话详情", exact: true })).toHaveCount(0);
});

test("用户消息最新在前，长粘贴保留末尾要求且能展开完整原文", async ({ page }) => {
  const { state } = await mockApi(page);
  const first = state.row.asks[0];
  const middle = "中间原文必须完整保留";
  const tail = "请按这个顺序处理，可以并行开 threads 完成吗？";
  first.text = `此前的审查报告\n\n${"报告细节。".repeat(300)}\n\n${middle}\n\n${"检查记录。".repeat(300)}\n\n${tail}`;
  state.row.asks.push({ ...first, id: "ask-latest", at: now, text: "先处理我自己的 issue 和 PR，没问题就合并。" });
  await ready(page);
  await page.getByRole("button", { name: row.title, exact: true }).click();
  const drawer = page.getByRole("dialog");
  await expect(drawer.getByText("你发的消息 · 最新在前", { exact: true })).toBeVisible();
  await expect(drawer.locator('[class*="askRow"]').first()).toContainText(state.row.asks[1].text);
  await expect(drawer.getByText(tail, { exact: true })).toBeVisible();
  await expect(drawer.getByText(middle, { exact: true })).toHaveCount(0);
  const expand = drawer.getByRole("button", { name: /展开完整原文/ });
  await expand.click();
  await expect(drawer.getByText(middle, { exact: true })).toBeVisible();
  await drawer.getByRole("button", { name: "收起原文", exact: true }).click();
  await expect(drawer.getByText(middle, { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "完整进度账 →", exact: true }).click();
  await expect(page.getByRole("heading", { name: "你发的消息 · 最新在前", exact: true })).toBeVisible();
  await expect(page.locator('[class*="agentAsk"]').first()).toContainText(state.row.asks[1].text);
  await expect(page.getByText(tail, { exact: true })).toBeVisible();
  await page.getByRole("button", { name: /展开完整原文/ }).click();
  await expect(page.getByText(middle, { exact: true })).toBeVisible();
});

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

test("监测来源、阻塞问题、范围外提醒与查看后的证据变化可见", async ({ page }, info) => {
  const { state } = await mockApi(page);
  state.row.state = "needs_input";
  state.row.inputRequest = { tool: "AskUserQuestion", question: "部署到哪个环境？", at: now };
  state.row.observation = { source: "transcript", reason: "记录中存在尚未回答的 AskUserQuestion", at: now };
  const old = { ...structuredClone(row), sessionId: "old-input", title: "昨天尚未回答的问题", state: "needs_input" as const, inputRequest: { tool: "AskUserQuestion", question: "允许执行吗？", at: now }, progress: { done: 0, total: 0 }, lastActiveAt: new Date(Date.parse(now) - 25 * 3600000).toISOString() };
  old.changes = { since: new Date(Date.parse(now) - 3600000).toISOString(), files: [], checks: old.evidence.filter(e => e.kind === "test"), steps: old.trail };
  old.observation = { source: "transcript", reason: "记录中存在尚未回答的 AskUserQuestion", at: now };
  state.outside = [old];
  await page.route("**/api/ledger/old-input", route => route.fulfill({ json: { success: true, data: old } }));
  await ready(page);
  const card = page.getByRole("button", { name: row.title, exact: true });
  await expect(card).toContainText("部署到哪个环境？");
  await expect(page.getByText(/最近 24 小时 · 1 个等你回答或审批/)).toBeVisible();
  await expect(page.getByText("最近扫描 刚刚", { exact: true })).not.toBeVisible();
  await expect(page.getByText(/实时通知已安装，尚未收到事件/)).not.toBeVisible();
  await page.getByText("监测状态", { exact: true }).click();
  await expect(page.getByText("最近扫描 刚刚", { exact: true })).toBeVisible();
  await expect(page.getByText(/实时通知已安装，尚未收到事件/)).toBeVisible();
  await expect(page.getByText(/未查看指尚未在 Keepline 中打开/)).toHaveCount(0);
  await page.getByText(/时间范围外还有 1 个会话等你回答或审批/).click();
  await page.getByRole("button", { name: `${old.title} · 等你回答或审批`, exact: true }).click();
  const drawer = page.getByRole("dialog", { name: `会话详情：${old.title}` });
  await expect(drawer.getByText(/自上次查看后/)).toBeVisible();
  await drawer.getByText("状态依据 · 执行记录", { exact: true }).click();
  await expect(drawer.getByText("记录中存在尚未回答的 AskUserQuestion", { exact: true })).toBeVisible();
  await expect(page.getByRole("spinbutton", { name: "最近活动小时数" })).toHaveValue("24");
  await page.screenshot({ path: info.outputPath("observability.png") });
});

test("监测状态读取失败明确显示，已有会话仍可查看", async ({ page }) => {
  const { state } = await mockApi(page);
  state.failMonitor = true;
  await ready(page);
  await expect(page.getByText(/无法确认监测状态：模拟监测状态读取失败/)).toBeVisible();
  await expect(page.getByText(/最近 24 小时 · 待处理数量尚未确认/)).toBeVisible();
  await expect(page.getByRole("button", { name: row.title, exact: true })).toBeVisible();
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
  await expect(page.getByRole("status").filter({ hasText: "暂无会话" })).toContainText("暂无会话");
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
  await page.getByRole("button", { name: "按紧急度", exact: true }).click();
  const needs = page.locator("section").filter({ has: page.getByRole("heading", { name: "需要你", exact: true }) });
  const card = needs.getByRole("button", { name: row.title, exact: true });
  await expect(card).toHaveCSS("border-color", "rgb(247, 199, 125)");
  await expect(card).toHaveCSS("background-color", "rgb(255, 252, 247)");
  await expect(card.getByText("◐ 新回复", { exact: true })).toHaveCSS("background-color", "rgb(255, 241, 219)");
  await expect(card.getByText("◐ 新回复", { exact: true })).toHaveCSS("color", "rgb(180, 83, 9)");
  await expect(needs.getByRole("button", { name: "查看回复", exact: true })).toBeVisible();
  await needs.getByRole("button", { name: "查看回复", exact: true }).click();
  await expect(page.getByRole("dialog", { name: `会话详情：${row.title}` })).toBeVisible();
  await expect(page.getByRole("dialog").getByText("◐ 新回复", { exact: true })).toHaveCSS("background-color", "rgb(255, 241, 219)");
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
test("默认最新活动优先，跨状态和布局保持时间顺序", async ({ page }) => {
  await mockApi(page);
  const samples = [
    { ...structuredClone(row), sessionId: "old-input", title: "旧的等待输入", state: "needs_input" as const, lastActiveAt: new Date(Date.parse(now) - 3 * 3600000).toISOString() },
    { ...structuredClone(row), sessionId: "new-ended", title: "最新已结束", state: "ended" as const, lastActiveAt: now },
    { ...structuredClone(row), sessionId: "new-running", title: "较新运行中", state: "running" as const, offPlan: [], lastActiveAt: new Date(Date.parse(now) - 60000).toISOString() },
  ];
  await page.route("**/api/ledger?*", route => route.fulfill({ json: { success: true, data: samples } }));
  await page.goto("/");
  await expect(page.getByRole("button", { name: "最新已结束", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "最新活动", exact: true })).toHaveAttribute("aria-pressed", "true");
  const cards = page.getByRole("button", { name: /^(旧的等待输入|最新已结束|较新运行中)$/ });
  for (const layout of ["网格", "列表", "看板"]) {
    await page.getByRole("button", { name: layout, exact: true }).click();
    await expect(cards).toHaveCount(3);
    await expect(cards.nth(0)).toHaveAttribute("aria-label", "最新已结束");
    await expect(cards.nth(1)).toHaveAttribute("aria-label", "较新运行中");
    await expect(cards.nth(2)).toHaveAttribute("aria-label", "旧的等待输入");
  }
  await expect(page.getByText(/3 条会话记录/)).toBeVisible();
});

for (const width of [1440, 390]) test(`退出状态与回合状态独立，未知记录不计入在线 · ${width}`, async ({ page }) => {
  await page.setViewportSize({ width, height: 1000 });
  await mockApi(page);
  const samples = [
    { ...structuredClone(row), sessionId: 'online', title: '在线但已回复', processState: 'online' as const, state: 'review' as const, unread: true, offPlan: [], progress: { done: 0, total: 0 } },
    { ...structuredClone(row), sessionId: 'closed', title: '退出后保留回复', processState: 'exited' as const, state: 'review' as const, unread: true, offPlan: [], progress: { done: 0, total: 0 } },
    { ...structuredClone(row), sessionId: 'unknown', title: '共享进程无法确认', processState: 'unknown' as const, state: 'running' as const, offPlan: [] },
  ];
  await page.route('**/api/ledger?*', route => route.fulfill({ json: { success: true, data: samples } }));
  await page.goto('/');
  await expect(page.getByText(/1 个在线，1 个已退出，1 个在线状态待确认/)).toHaveCount(1);
  const closed = page.getByRole('button', { name: '退出后保留回复', exact: true });
  await expect(closed).toContainText('已退出 · 新回复');
  await expect(closed).toContainText('查看回复');
  await expect(page.getByRole('button', { name: '共享进程无法确认', exact: true })).toContainText('无法确认在线');
  await page.getByRole('button', { name: '按紧急度', exact: true }).click();
  await expect(closed).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
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
  await page.getByRole("button", { name: "查看其余 3 条历史会话 ↓" }).click();
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
  // This single case renders every view at multiple sizes and compares PNGs.
  test.setTimeout(90_000);
  // Render the supplied HTML itself, preserving its template and styles. Only data is normalized.
  const { state, goal, task } = await mockApi(page);
  await page.route("**/api/goals", (r) =>
    r.fulfill({ json: { success: true, data: [] } }),
  );
  await page.route("**/api/goals/todos", (r) =>
    r.fulfill({ json: { success: true, data: [] } }),
  );
  await ready(page);
  // Match the reference grouping; the product default is now recent activity.
  await page.getByRole("button", { name: "按紧急度", exact: true }).click();
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
  // Version is release data, like the session titles above; preserve the original
  // reference styles and the zero-pixel sidebar comparison at the current version.
  const appVersion = JSON.parse(readFileSync(resolve("../../../menubar-tauri/src-tauri/tauri.conf.json"), "utf8")).version as string;
  html = html.replace(">v0.4<", `>v${appVersion}<`);
  await expect(page.locator("nav").getByText(`v${appVersion}`, { exact: true })).toBeVisible();
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
      async ([a, b, strictHeight]) => {
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
          strictHeight && actual.height !== reference.height
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
        for (let i = 0; i < Math.max(aa.length, bb.length); i += 4)
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
          height: Math.max(actual.height, reference.height),
        };
      },
      [actualPng.toString("base64"), referencePng.toString("base64"), exact] as const,
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
        ? page.getByRole("button", { name: row.title, exact: true })
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
      action: "查看待响应会话",
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
    // A complete checklist still awaits the user's acceptance. Keep the same
    // attention color as other review rows; the older reference used gray.
    expect(await actualTag.evaluate(palette)).toEqual("complete" in variant
      ? { background: "rgb(255, 241, 219)", color: "rgb(180, 83, 9)" }
      : await referenceTag.evaluate(palette));
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
  await page.route("**/api/goals/todos", (r) =>
    r.fulfill({ json: { success: true, data: goal.todos } }),
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

test("交互修复：修饰键和未选中的会话不会触发验收", async ({ page }) => {
  const { state, calls } = await mockApi(page);
  state.row.state = "review";
  state.row.progress = { done: 3, total: 3 };
  await ready(page);
  await page.locator("body").click({ position: { x: 1400, y: 900 } });
  for (const key of ["Control+a", "Meta+a", "Alt+a", "a"])
    await page.keyboard.press(key);
  expect(calls.filter(call => call.path.endsWith("/acceptances"))).toHaveLength(0);
  await page.getByRole("button", { name: row.title, exact: true }).focus();
  await page.keyboard.press("a");
  await expect.poll(() => calls.filter(call => call.path.endsWith("/acceptances")).length).toBe(1);
});

test("交互修复：键盘按当前分组顺序选择会话", async ({ page }) => {
  await mockApi(page);
  const urgent = { ...structuredClone(row), sessionId: "urgent", title: "较早等待审批", state: "needs_input", lastActiveAt: "2026-10-07T12:00:00+08:00" };
  await page.route("**/api/ledger?*", route => route.fulfill({ json: { success: true, data: [row, urgent] } }));
  await page.route("**/api/ledger/urgent", route => route.fulfill({ json: { success: true, data: urgent } }));
  await ready(page);
  await page.getByRole("button", { name: "按紧急度", exact: true }).click();
  await expect(page.locator('[class*="sessionCard"]').first()).toHaveAttribute("aria-label", urgent.title);
  await page.locator("body").click({ position: { x: 1400, y: 900 } });
  await page.keyboard.press("j");
  await expect(page.locator('[data-session-id="urgent"]')).toBeFocused();
  await page.keyboard.press("j");
  await expect(page.locator(`[data-session-id="${row.sessionId}"]`)).toBeFocused();
  await page.keyboard.press("k");
  await expect(page.locator('[data-session-id="urgent"]')).toBeFocused();
  await page.locator("body").click({ position: { x: 1400, y: 900 } });
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/sessionId=urgent/);
  await expect(page.getByRole("dialog", { name: `会话详情：${urgent.title}` })).toBeVisible();
});

test("交互修复：悬停冻结位置但实时更新内容，移开后恢复排序", async ({ page }) => {
  const { state } = await mockApi(page);
  const peer = { ...structuredClone(row), sessionId: "peer", title: "另一个会话", lastActiveAt: "2026-10-07T12:00:00+08:00" };
  await page.route("**/api/ledger?*", route => route.fulfill({ json: { success: true, data: [state.row, peer] } }));
  let socket: { send: (data: string) => void };
  await page.routeWebSocket(/\/ws\?/, ws => { socket = ws; });
  await ready(page);
  const cards = page.locator('[class*="sessionCard"]');
  await page.getByRole("button", { name: row.title, exact: true }).hover();
  peer.lastActiveAt = now;
  state.row.activity = { ...state.row.activity!, action: "悬停期间的新动作" };
  state.row.state = "needs_input";
  state.row.inputRequest = { tool: "AskUserQuestion", question: "悬停期间的新问题", at: now };
  socket!.send(JSON.stringify({ type: "ledger:update", timestamp: now }));
  await page.clock.runFor(250);
  await expect(page.getByRole("button", { name: row.title, exact: true })).toContainText("悬停期间的新问题");
  await expect(page.getByRole("button", { name: row.title, exact: true }).getByRole("button", { name: "查看待响应会话", exact: true })).toBeVisible();
  await expect(cards.first()).toHaveAttribute("aria-label", row.title);
  await page.mouse.move(0, 0);
  await expect(cards.first()).toHaveAttribute("aria-label", peer.title);
});

test("交互修复：待办和目标实际响应项目与搜索筛选", async ({ page }) => {
  const { task, goal } = await mockApi(page);
  const otherTask = { ...task, id: "other-task", parentId: "other-goal", title: "另一个项目的任务", projectRoot: "/project/other" };
  const otherGoal = { ...goal, id: "other-goal", title: "另一个项目的目标", projectRoot: "/project/other", todos: [{ ...goal.todos[0], ...otherTask }] };
  await page.route("**/api/goals", route => route.fulfill({ json: { success: true, data: [{ ...goal, projectRoot: task.projectRoot }, otherGoal] } }));
  await page.route("**/api/work-items", route => route.fulfill({ json: { success: true, data: { items: [task, otherTask] } } }));
  await page.route("**/api/goals/todos", route => route.fulfill({ json: { success: true, data: [goal.todos[0], otherGoal.todos[0]] } }));
  await ready(page);
  await page.getByRole("button", { name: "待办", exact: true }).click();
  await expect(page.getByRole("button", { name: otherTask.title, exact: true })).toBeVisible();
  await page.locator(`button[title="${task.projectRoot}"]`).click();
  await expect(page.getByRole("heading", { name: "待办", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: task.title, exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: otherTask.title, exact: true })).toHaveCount(0);
  await page.getByRole("textbox", { name: "搜索", exact: true }).fill("不存在的搜索词");
  await expect(page.getByRole("button", { name: task.title, exact: true })).toHaveCount(0);
  await expect(page.getByText("没有符合当前筛选的待办。可清空搜索或切换项目。")).toBeVisible();
  await page.getByRole("textbox", { name: "搜索", exact: true }).fill("生成恢复接口文档");
  await expect(page.getByRole("button", { name: task.title, exact: true })).toBeVisible();
  await page.getByRole("button", { name: "目标", exact: true }).click();
  await expect(page.getByRole("button", { name: goal.title, exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: otherGoal.title, exact: true })).toHaveCount(0);
  await page.getByRole("textbox", { name: "搜索", exact: true }).fill("");
  await page.locator('button[title="/project/other"]').click();
  await expect(page.getByRole("heading", { name: "目标", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: otherGoal.title, exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: goal.title, exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "回顾", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "搜索", exact: true })).toHaveCount(0);
});

for (const layout of ["网格", "列表", "看板"]) test(`交互修复：${layout} 保留停止、追问和验收操作`, async ({ page }) => {
  const { state } = await mockApi(page);
  await ready(page);
  await page.getByRole("button", { name: layout, exact: true }).click();
  const more = page.getByRole("button", { name: `更多操作：${row.title}`, exact: true });
  await more.click();
  for (const action of ["查看会话记录", "看进度账", "复制追问", "停止进程"])
    await expect(page.getByRole("menuitem", { name: action, exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);
  state.row.state = "review";
  await page.reload();
  await more.click();
  for (const action of ["验收通过", "带着缺口验收"])
    await expect(page.getByRole("menuitem", { name: action, exact: true })).toBeVisible();
});

test("交互修复：停止需要确认，取消不发送请求，失败可原地重试", async ({ page }) => {
  const { calls } = await mockApi(page);
  let fail = true;
  await page.route(`**/api/sessions/${row.sessionId}/stop`, async route => {
    calls.push({ path: new URL(route.request().url()).pathname, method: "POST", data: {} });
    await route.fulfill({ status: fail ? 500 : 200, json: fail ? { success: false, error: "模拟停止失败" } : { success: true } });
  });
  await ready(page);
  const openStop = async () => {
    await page.getByRole("button", { name: `更多操作：${row.title}`, exact: true }).click();
    await page.getByRole("menuitem", { name: "停止进程", exact: true }).click();
  };
  await openStop();
  const dialog = page.getByRole("dialog", { name: "停止会话进程" });
  await expect(dialog).toContainText(row.title);
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  expect(calls.filter(call => call.path.endsWith("/stop"))).toHaveLength(0);
  await openStop();
  await dialog.getByRole("button", { name: "停止进程", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText("模拟停止失败");
  fail = false;
  await dialog.getByRole("button", { name: "停止进程", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  expect(calls.filter(call => call.path.endsWith("/stop"))).toHaveLength(2);
});

test("交互修复：浏览器前进后退恢复页面、抽屉与完整进度账", async ({ page }) => {
  await mockApi(page);
  await ready(page);
  await page.getByRole("button", { name: "待办", exact: true }).click();
  await page.getByRole("button", { name: "目标", exact: true }).click();
  await page.goBack();
  await expect(page.getByRole("heading", { name: "待办", exact: true })).toBeVisible();
  await page.goForward();
  await expect(page.getByRole("heading", { name: "目标", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "总览", exact: true }).click();
  await page.getByRole("textbox", { name: "搜索", exact: true }).fill("OpenAPI");
  await page.locator('button[title="/project/keepline"]').click();
  await page.getByRole("button", { name: row.title, exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.goBack();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.goForward();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByRole("button", { name: "完整进度账 →", exact: true }).click();
  await expect(page.getByRole("heading", { name: "时间线", exact: true })).toBeVisible();
  await page.goBack();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.goForward();
  await expect(page.getByRole("heading", { name: "时间线", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "← 总览", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("textbox", { name: "搜索", exact: true })).toHaveValue("OpenAPI");
  await expect(page).toHaveURL(/project=%2Fproject%2Fkeepline/);
  await page.getByRole("button", { name: row.title, exact: true }).click();
  await page.getByRole("dialog").getByRole("button", { name: "查看会话记录", exact: true }).click();
  await expect(page).toHaveURL(/view=sessions/);
  await page.goBack();
  await expect(page.getByRole("dialog")).toContainText(row.title);
  await expect(page.getByRole("textbox", { name: "搜索", exact: true })).toHaveValue("OpenAPI");
});

test("交互修复：窄屏收起辅助导航，详情锁定背景和键盘焦点", async ({ page }, info) => {
  await mockApi(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await ready(page);
  const nav = page.getByRole("navigation", { name: "主导航" });
  expect((await nav.boundingBox())!.height).toBeLessThan(130);
  await expect(page.getByRole("button", { name: "设置", exact: true })).toBeHidden();
  await page.getByRole("button", { name: "展开项目和设置", exact: true }).click();
  await expect(page.getByRole("button", { name: "设置", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "收起项目和设置", exact: true }).click();
  const card = page.getByRole("button", { name: row.title, exact: true });
  await card.click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toHaveAttribute("aria-modal", "true");
  await page.screenshot({ path: info.outputPath("mobile-drawer-fixed.png"), animations: "disabled" });
  await dialog.getByRole("button", { name: "关闭", exact: true }).focus();
  await page.keyboard.press("Shift+Tab");
  await expect(dialog.getByRole("button", { name: "完整进度账 →", exact: true })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(dialog.getByRole("button", { name: "关闭", exact: true })).toBeFocused();
  expect(await page.evaluate(() => document.body.style.overflow)).toBe("hidden");
  await expect(page.locator('nav[aria-label="主导航"]')).toHaveAttribute("inert", "");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(nav).not.toHaveAttribute("inert");
  await expect(card).toBeFocused();
  expect(await page.evaluate(() => document.body.style.overflow)).not.toBe("hidden");
});

test("错误与草稿修复：清单插入重排不串用完成状态", async ({ page }) => {
  const { task, calls } = await mockApi(page);
  task.acceptance = [
    { id: "check-1", text: "已完成的文档", completed: true },
    { id: "check-2", text: "待完成的测试", completed: false },
  ];
  await page.route("**/api/goals/todos", route => route.fulfill({ json: { success: true, data: [{ ...task, checklist: [], sessions: [], readyToComplete: false }] } }));
  await ready(page);
  await page.getByRole("button", { name: "待办", exact: true }).click();
  await page.getByRole("button", { name: task.title, exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "编辑目标或待办" });
  await dialog.getByRole("textbox", { name: "验收清单（每行一项）" }).fill("待完成的测试\n新增安全测试\n已完成的文档\n已完成的文档");
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  await expect.poll(() => calls.find(c => c.path === "/api/work-items/todo-1" && c.method === "PATCH")).toBeTruthy();
  const checks = calls.find(c => c.path === "/api/work-items/todo-1" && c.method === "PATCH")!.data!.acceptance as typeof task.acceptance;
  expect(checks[0]).toEqual(task.acceptance[1]);
  expect(checks[1]).toMatchObject({ text: "新增安全测试", completed: false });
  expect(checks[2]).toEqual(task.acceptance[0]);
  expect(checks[3]).toMatchObject({ text: "已完成的文档", completed: false });
  expect(new Set(checks.map(check => check.id)).size).toBe(4);
});

for (const width of [1440, 390]) test(`错误与草稿修复：详情失败仍可重试或关闭 · ${width}`, async ({ page }, info) => {
  await mockApi(page);
  await page.setViewportSize({ width, height: 844 });
  let fail = true;
  await page.route("**/api/ledger/ui-session", route => route.fulfill(fail
    ? { status: 500, json: { success: false, error: "详情读取失败" } }
    : { json: { success: true, data: row } }));
  await ready(page);
  const card = page.getByRole("button", { name: row.title, exact: true });
  await card.click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("alert")).toContainText("详情读取失败");
  await page.screenshot({ path: info.outputPath(`detail-error-${width}.png`), animations: "disabled" });
  await dialog.getByRole("button", { name: "重试", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText("详情读取失败");
  fail = false;
  await dialog.getByRole("button", { name: "重试", exact: true }).click();
  await expect(dialog).toContainText(row.title);
  await dialog.getByRole("button", { name: "关闭", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("navigation", { name: "主导航" })).not.toHaveAttribute("inert");
  await expect(card).toBeFocused();
});

test("错误与草稿修复：保存失败在编辑器内显示，后台同步不清除，可重试", async ({ page }) => {
  const { task } = await mockApi(page);
  let fail = true;
  await page.route("**/api/work-items/todo-1", route => route.fulfill(fail
    ? { status: 500, json: { success: false, error: "待办保存失败" } }
    : { json: { success: true, data: task } }));
  await ready(page);
  await page.getByRole("button", { name: "待办", exact: true }).click();
  await page.getByRole("button", { name: task.title, exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "编辑目标或待办" });
  await dialog.getByRole("textbox", { name: "标题", exact: true }).fill("保留修改的任务");
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText("待办保存失败");
  await page.clock.runFor(31000);
  await expect(dialog.getByRole("alert")).toContainText("待办保存失败");
  await expect(dialog.getByRole("textbox", { name: "标题", exact: true })).toHaveValue("保留修改的任务");
  fail = false;
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  await expect(dialog).toHaveCount(0);
});

test("错误与草稿修复：设置保存失败恢复服务器值且保留提示", async ({ page }) => {
  const { state } = await mockApi(page);
  await ready(page);
  await page.getByRole("button", { name: "设置", exact: true }).click();
  state.failSettings = true;
  const original = state.settings.focus.minutes;
  await page.locator('details').filter({ has: page.locator('input[aria-label="专注分钟数"]') }).locator("summary").click();
  const input = page.getByRole("spinbutton", { name: "专注分钟数" });
  await input.fill("99");
  await input.blur();
  await expect(page.getByRole("alert")).toContainText("模拟设置保存失败");
  await expect(input).toHaveValue(String(original));
  expect(state.settings.focus.minutes).toBe(original);
  await page.clock.runFor(31000);
  await expect(page.getByRole("alert")).toContainText("模拟设置保存失败");
  state.failSettings = false;
  await input.fill("99");
  await input.blur();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(input).toHaveValue("99");
  expect(state.settings.focus.minutes).toBe(99);
});

test("错误与草稿修复：Escape 与取消先确认放弃，继续编辑保留草稿", async ({ page }) => {
  await mockApi(page);
  await ready(page);
  await page.getByRole("button", { name: "待办", exact: true }).click();
  await page.getByRole("button", { name: "＋ 加待办", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "编辑目标或待办" });
  const title = dialog.getByRole("textbox", { name: "标题", exact: true });
  await title.fill("没有保存的详细任务");
  page.once("dialog", d => { expect(d.message()).toContain("未保存"); void d.dismiss(); });
  await page.keyboard.press("Escape");
  await expect(title).toHaveValue("没有保存的详细任务");
  page.once("dialog", d => void d.accept());
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await page.getByRole("button", { name: "＋ 加待办", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "标题", exact: true })).toHaveValue("");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
});

test("错误与草稿修复：新建待办与目标继承当前项目", async ({ page }) => {
  const { goal } = await mockApi(page);
  Object.assign(goal, { projectRoot: "/project/keepline" });
  await ready(page);
  await page.locator('button[title="/project/keepline"]').click();
  await page.getByRole("button", { name: "待办", exact: true }).click();
  await page.getByRole("button", { name: "＋ 加待办", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "项目目录", exact: true })).toHaveValue("/project/keepline");
  await page.getByRole("button", { name: "取消", exact: true }).click();
  await page.getByRole("button", { name: "目标", exact: true }).click();
  await page.getByRole("button", { name: "＋ 加目标", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "项目目录", exact: true })).toHaveValue("/project/keepline");
});

for (const kind of ["后续提示", "要求"]) test(`错误与草稿修复：${kind}显示错误并保护修改`, async ({ page }) => {
  await mockApi(page);
  await ready(page);
  await page.getByRole("button", { name: row.title, exact: true }).click();
  await page.getByRole("button", { name: "完整进度账 →", exact: true }).click();
  if (kind === "后续提示") {
    await page.getByRole("button", { name: "复制追问 (C)", exact: true }).click();
    await page.evaluate(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async () => { throw new Error("复制失败"); } } }));
  } else {
    await page.getByRole("button", { name: "编辑要求", exact: true }).click();
    await page.route("**/api/ledger/ui-session/items", route => route.fulfill({ status: 500, json: { success: false, error: "保存要求失败" } }));
  }
  const dialog = page.getByRole("dialog");
  const input = kind === "后续提示" ? dialog.getByRole("textbox") : dialog.getByRole("textbox", { name: "要求标题", exact: true }).first();
  await input.fill("修改后尚未保存的内容");
  await dialog.getByRole("button", { name: kind === "后续提示" ? "复制" : "保存并确认", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText(kind === "后续提示" ? "复制失败" : "保存要求失败");
  page.once("dialog", d => void d.dismiss());
  await page.keyboard.press("Escape");
  await expect(input).toHaveValue("修改后尚未保存的内容");
  page.once("dialog", d => void d.accept());
  await dialog.getByRole("button", { name: kind === "后续提示" ? "关闭" : "取消", exact: true }).click();
  await expect(dialog).toHaveCount(0);
});
