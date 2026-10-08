import { test, expect, type Page } from "@playwright/test";
import type { Goal, Todo } from "../../src/pages/ledger/types";
import { now, row, settings } from "./fixtures";

async function mapApi(page: Page, open = true) {
  await page.clock.install({ time: new Date(now) });
  await page.addInitScript(() =>
    localStorage.setItem("terminal_token", "ui-contract-test"),
  );
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const calls: Array<{
    path: string;
    method: string;
    projectMap: string | null;
  }> = [];
  const todo = (
    id: string,
    title: string,
    extra: Partial<Todo> = {},
  ): Todo => ({
    id,
    title,
    kind: "todo",
    level: "task",
    parentId: "map-goal",
    status: "planned",
    statusSource: "user",
    createdAt: now,
    updatedAt: now,
    projectRoot: "/project/keepline",
    readyToComplete: false,
    acceptance: [
      { id: `${id}-check`, text: `${title}通过测试`, completed: false },
    ],
    checklist: [
      {
        id: `${id}-check`,
        text: `${title}通过测试`,
        evidenced: false,
        satisfied: false,
      },
    ],
    sessions: [],
    ...extra,
  });
  const goal: Goal = {
    id: "map-goal",
    title: "发布项目进度图",
    kind: "todo",
    level: "goal",
    status: "active",
    statusSource: "user",
    createdAt: now,
    updatedAt: now,
    projectRoot: "/project/keepline",
    outcome: "交付项有可核验的进展",
    stale: false,
    weeklyMovement: 1,
    progress: { done: 1, total: 4, active: 1 },
    todos: [
      todo("finished", "数据接入", { status: "done", completedAt: now }),
      todo("waiting", "系统通知", {
        sessions: [
          {
            runtime_session_id: "old-session",
            title: "通知权限",
            status: "needs_input",
            needsInput: true,
            statusReason: "等待批准系统通知权限",
          },
        ],
      }),
      todo("review", "导出报告", {
        checklist: [
          {
            id: "review-check",
            text: "导出报告通过测试",
            evidenced: true,
            satisfied: false,
          },
        ],
        sessions: [
          {
            runtime_session_id: row.sessionId,
            title: row.title,
            status: "idle",
          },
        ],
      }),
      todo("unstarted", "登录自启"),
    ],
    recent: [
      {
        id: "map-test",
        kind: "test",
        title: "通知适配测试通过",
        at: now,
        todoId: "waiting",
        sessionId: "old-session",
      },
    ],
  };
  await page.route("**/api/**", async (route) => {
    const req = route.request(),
      path = new URL(req.url()).pathname,
      method = req.method();
    calls.push({
      path,
      method,
      projectMap: new URL(req.url()).searchParams.get("projectMap"),
    });
    let data: unknown = {};
    if (path === "/api/auth/status")
      data = { setupComplete: true, authenticated: true, username: "local" };
    else if (path === "/api/ledger") data = [row];
    else if (path === "/api/settings/ledger") data = settings;
    else if (path === "/api/goals")
      data = [
        {
          ...goal,
          recent:
            new URL(req.url()).searchParams.get("projectMap") === goal.id
              ? goal.recent
              : [],
        },
      ];
    else if (path === "/api/goals/todos") data = goal.todos;
    else if (path === "/api/work-items") data = { items: goal.todos };
    else if (path.endsWith("/follow-up"))
      data = { text: "请完成通知适配并运行测试。" };
    else if (
      path.startsWith("/api/goals/todos/") &&
      path.endsWith("/complete")
    ) {
      const completed = goal.todos.find((t) => t.id === path.split("/")[4])!;
      completed.status = "done";
      completed.readyToComplete = false;
      goal.progress.done = goal.todos.filter((t) => t.status === "done").length;
      data = completed;
    } else if (path.startsWith("/api/ledger/"))
      data = {
        ...row,
        sessionId: path.split("/")[3],
        title: path.includes("old-session") ? "通知权限" : row.title,
      };
    await route.fulfill({ json: { success: true, data } });
  });
  await page.goto("/?view=goals");
  await page
    .getByRole("button", { name: "查看项目进度图", exact: true })
    .waitFor();
  await expect(
    page.getByRole("region", { name: `项目进度图：${goal.title}` }),
  ).toHaveCount(0);
  if (open) await openMap(page);
  return { calls, goal, errors };
}

async function openMap(page: Page) {
  await page
    .getByRole("button", { name: "查看项目进度图", exact: true })
    .click();
  await page
    .getByRole("region", { name: "项目进度图：发布项目进度图" })
    .waitFor();
}

test("project maps are optional and fetch recent progress only after selection", async ({
  page,
}) => {
  const { calls } = await mapApi(page, false);
  expect(calls.filter((c) => c.projectMap)).toHaveLength(0);
  await openMap(page);
  await expect
    .poll(() => calls.filter((c) => c.projectMap === "map-goal").length)
    .toBeGreaterThan(0);
  await page
    .getByRole("button", { name: "收起项目进度图", exact: true })
    .click();
  await expect(
    page.getByRole("region", { name: "项目进度图：发布项目进度图" }),
  ).toHaveCount(0);
  await page.reload();
  await expect(
    page.getByRole("button", { name: "查看项目进度图", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("region", { name: "项目进度图：发布项目进度图" }),
  ).toHaveCount(0);
});

test("project map distinguishes evidence from acceptance and opens older linked sessions", async ({
  page,
}, info) => {
  const { calls, errors } = await mapApi(page);
  const map = page.getByRole("region", { name: "项目进度图：发布项目进度图" });
  await expect(map.getByText("离目标交付还差 3 项")).toBeVisible();
  const review = map.getByRole("article").filter({
    has: page.getByRole("button", { name: "导出报告", exact: true }),
  });
  await expect(review.getByText("有证据 · 待验收")).toBeVisible();
  await expect(review.getByRole("button", { name: "标记完成" })).toHaveCount(0);
  await expect(
    map
      .getByRole("region", { name: "建议下一步" })
      .getByText("核对「导出报告」的执行证据"),
  ).toBeVisible();
  await expect(
    map
      .getByRole("region", { name: "需要你处理" })
      .getByText("等待批准系统通知权限"),
  ).toBeVisible();
  await page.screenshot({
    path: info.outputPath("project-map-desktop.png"),
    fullPage: true,
  });
  await map
    .getByRole("region", { name: "最近进展" })
    .getByRole("button", { name: /通知适配测试通过/ })
    .click();
  await expect(
    page.getByRole("dialog", { name: "会话详情：通知权限" }),
  ).toBeVisible();
  expect(errors).toEqual([]);
  expect(
    calls.some(
      (c) => c.path === "/api/ledger/old-session" && c.method === "GET",
    ),
  ).toBe(true);
  expect(calls.some((c) => c.path.endsWith("/complete"))).toBe(false);
  await page.reload();
  await expect(
    page.getByRole("dialog", { name: "会话详情：通知权限" }),
  ).toBeVisible();
});

test("old stopped attempts do not block active work, and completion requires a click", async ({
  page,
}) => {
  const { goal, calls } = await mapApi(page);
  const review = goal.todos[2];
  review.checklist[0].evidenced = false;
  review.sessions.push({
    runtime_session_id: "old-failed",
    title: "旧尝试",
    status: "lost",
  });
  await page.reload();
  await openMap(page);
  const card = page.getByRole("article").filter({
    has: page.getByRole("button", { name: "导出报告", exact: true }),
  });
  await expect(card.getByText("进行中", { exact: true })).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "需要你处理 · 1" }),
  ).toBeVisible();
  review.checklist[0].evidenced = true;
  review.checklist[0].satisfied = true;
  review.readyToComplete = true;
  await page.reload();
  await openMap(page);
  await expect(card.getByText("可标记完成", { exact: true })).toBeVisible();
  expect(calls.some((c) => c.path.endsWith("/complete"))).toBe(false);
  await card.getByRole("button", { name: "标记完成", exact: true }).click();
  await expect(page.getByText("离目标交付还差 2 项")).toBeVisible();
  expect(
    calls.filter((c) => c.path.endsWith("/complete") && c.method === "POST"),
  ).toHaveLength(1);
});

test("project map follow-up and empty-goal controls work without automatically dispatching", async ({
  page,
}) => {
  const { calls } = await mapApi(page);
  const card = page.getByRole("article").filter({
    has: page.getByRole("button", { name: "导出报告", exact: true }),
  });
  await card.getByRole("button", { name: "生成续做提示" }).click();
  await expect(
    page.getByRole("dialog", { name: "编辑后续提示" }).getByRole("textbox"),
  ).toHaveValue("请完成通知适配并运行测试。");
  expect(calls.some((c) => c.path.endsWith("/follow-up"))).toBe(true);
  expect(calls.some((c) => c.path.endsWith("/dispatch"))).toBe(false);
});

test("project map fits narrow screens and states when no deliverables exist", async ({
  page,
}, info) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const { goal } = await mapApi(page);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: info.outputPath("project-map-mobile.png"),
    fullPage: true,
  });
  await page.route("**/api/goals*", (r) =>
    r.fulfill({
      json: {
        success: true,
        data: [
          {
            ...goal,
            todos: [],
            recent: [],
            progress: { done: 0, total: 0, active: 0 },
          },
        ],
      },
    }),
  );
  await page.reload();
  await openMap(page);
  await expect(page.getByText("尚未定义交付项")).toBeVisible();
  await expect(page.getByText("全部交付项已验收")).toHaveCount(0);
  await page
    .getByRole("region", { name: "建议下一步" })
    .getByRole("button", { name: "补充待办" })
    .click();
  await expect(
    page.getByRole("textbox", { name: "标题", exact: true }),
  ).toBeVisible();
});
