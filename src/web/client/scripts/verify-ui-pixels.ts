import { chromium, type Page } from "@playwright/test";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { now, row, settings } from "../tests/ui/fixtures";

// Independent acceptance: compare the served production build with the supplied
// archive. Never alter reference styles/copy or production DOM to make it pass.
const client = resolve(import.meta.dir, "..");
const repo = resolve(client, "../../..");
const origin = process.env.KEEPLINE_PIXEL_URL ?? "http://127.0.0.1:3377";
const archive =
  process.env.KEEPLINE_UI_ARCHIVE ??
  "/Users/lifcc/Downloads/库的UI设计需求.zip";
const output = resolve(
  process.env.KEEPLINE_PIXEL_OUTPUT ??
    join(repo, "specs/ui-v2/pixel-acceptance"),
);
const chrome =
  process.env.PLAYWRIGHT_EXECUTABLE_PATH ??
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
mkdirSync(output, { recursive: true });
const sha = (data: string | Buffer) =>
  createHash("sha256").update(data).digest("hex");
const archived = new Map<string, Buffer>();
function fromArchive(name: string) {
  if (!archived.has(name))
    archived.set(name, execFileSync("unzip", ["-p", archive, name]));
  return archived.get(name)!;
}
const original = fromArchive("Keepline App v3.dc.html").toString();
function treeHash(dir: string): string {
  const files: string[] = [];
  const walk = (d: string) => {
    for (const entry of readdirSync(d, { withFileTypes: true })) {
      const p = join(d, entry.name);
      if (entry.isDirectory()) walk(p);
      else if (entry.isFile()) files.push(p);
    }
  };
  walk(dir);
  return sha(
    files
      .sort()
      .map((p) => `${p.slice(dir.length)}:${sha(readFileSync(p))}`)
      .join("\n"),
  );
}
const productionBefore = treeHash(join(repo, "public/dist"));
const indexBefore = await (await fetch(origin)).text();
const indexMatchesDisk =
  indexBefore === readFileSync(join(repo, "public/dist/index.html"), "utf8");
const errors: Array<{ side: string; message: string }> = [];
const results: any[] = [];
const browser = await chromium.launch({
  executablePath: chrome,
  headless: true,
});
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
  stale: false,
  weeklyMovement: 0,
  todos: [
    {
      ...task,
      readyToComplete: false,
      checklist: [
        {
          id: "check-1",
          text: "生成恢复接口文档",
          evidenced: false,
          satisfied: false,
        },
      ],
      sessions: [],
    },
  ],
  progress: { done: 0, total: 1, active: 0 },
  recent: [],
};
const referenceGoal = {
  id: goal.id,
  title: goal.title,
  outcome: goal.outcome,
  weekly: 0,
  todos: [{ title: task.title, check: [0], names: ["生成恢复接口文档"] }],
  suggest: [],
};
function sample(r: typeof row, i: number) {
  const localAt = new Date(Date.parse(r.lastActiveAt) + 8 * 60 * 60 * 1000);
  return {
    id: i + 100,
    title: r.title,
    project: r.projectRoot.split("/").at(-1),
    state:
      r.state === "accepted" || r.state === "ended"
        ? "ended"
        : r.state === "stopped"
          ? "running"
          : r.state,
    accepted: r.state === "accepted",
    stalled: r.state === "stopped",
    offPlan: r.offPlan.length > 0,
    gap: r.state === "review" && r.progress.done < r.progress.total,
    agent: r.runtimeId === "codex" ? "Codex" : "Claude Code",
    start: 12 * 60 + 5,
    lastAt: localAt.getUTCHours() * 60 + localAt.getUTCMinutes(),
    ask: r.asks[0]?.text ?? "",
    items: r.items.map((it) => it.status),
    now: r.activity?.action,
    ev: "openapi lint · 0 错误 · 成功",
    said: r.activity?.lastMessage,
    ctx: null,
    action:
      r.state === "needs_input"
        ? "跳到 agent 回答"
        : r.offPlan.length
          ? "看偏离"
          : r.state === "review"
            ? r.progress.done < r.progress.total
              ? "生成追问"
              : "验收 (A)"
            : null,
  };
}
function referenceHtml(rows: (typeof row)[], withGoal: boolean) {
  const names = Object.fromEntries(
    rows.map((r, i) => [i + 100, r.items.map((it) => it.title)]),
  );
  // Only fixture data is replaced. The template, CSS and action/copy logic stay intact.
  const review = [
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
  return original
    .replace(
      "  renderVals() {",
      `  data(){return ${JSON.stringify(rows.map(sample))}}\n  goals(){return ${JSON.stringify(withGoal ? [referenceGoal] : [])}}\n  NAMES = ${JSON.stringify(names)};\n  CONSTRAINTS = {};\n  renderVals() {`,
    )
    .replace(
      /const reviewSections = \[[\s\S]*?\];\n    const tog/,
      `const reviewSections = ${JSON.stringify(review)};\n    const tog`,
    )
    .replace(
      "14 个会话结束 · 9 个已验收 · 3 个还挂着 · 2 次偏离",
      "0 个已验收 · 1 个还挂着 · 0 次偏离",
    )
    .replace(
      '<script src="./support.js"></script>',
      '<script>window.__resources={"https://unpkg.com/react@18.3.1/umd/react.production.min.js":"/__pixel/react.js","https://unpkg.com/react-dom@18.3.1/umd/react-dom.production.min.js":"/__pixel/react-dom.js"}</script><script src="./support.js"></script>',
    );
}
async function pair(
  rows: (typeof row)[] = [structuredClone(row)],
  withGoal = false,
  width = 1440,
  height = 1000,
) {
  const context = await browser.newContext({
    viewport: { width, height },
    deviceScaleFactor: 1,
    timezoneId: "Asia/Shanghai",
  });
  const actual = await context.newPage(),
    reference = await context.newPage();
  await actual.clock.install({ time: new Date(now) });
  await actual.addInitScript(() =>
    localStorage.setItem("terminal_token", "pixel-acceptance-fixture"),
  );
  actual.on("pageerror", (e) =>
    errors.push({ side: "actual", message: e.message }),
  );
  reference.on("pageerror", (e) =>
    errors.push({ side: "reference", message: e.message }),
  );
  await actual.route("**/api/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    // Opening a detail records a viewing heartbeat. Fulfill locally as fixture
    // data, just like GETs; no request from this browser may reach the real API.
    if (route.request().method() !== "GET")
      return route.fulfill({ json: { success: true, data: row } });
    let data: unknown = {};
    if (path === "/api/auth/status")
      data = { setupComplete: true, authenticated: true, username: "local" };
    else if (path === "/api/ledger") data = rows;
    else if (path === "/api/goals") data = withGoal ? [goal] : [];
    else if (path === "/api/work-items")
      data = { items: withGoal ? [task] : [] };
    else if (path === "/api/settings/ledger") data = settings;
    else if (path === "/api/ledger/review")
      data = {
        open: [row],
        accepted: [],
        offPlan: [],
        corrections: [],
        goals: withGoal ? [goal] : [],
        unattributedRuntimeShare: 0.2,
      };
    else if (path.startsWith("/api/ledger/"))
      data = rows.find((r) => path.includes(r.sessionId)) ?? row;
    else if (path === "/api/sessions")
      data = {
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
      data = { projects: [], stats: { total: 0, active: 0 } };
    await route.fulfill({ json: { success: true, data } });
  });
  const html = referenceHtml(rows, withGoal);
  await reference.route("**/__pixel/**", async (route) => {
    const path = decodeURIComponent(
      new URL(route.request().url()).pathname.slice("/__pixel/".length),
    );
    if (path === "index.html")
      return route.fulfill({ contentType: "text/html", body: html });
    if (path === "react.js" || path === "react-dom.js")
      return route.fulfill({
        contentType: "text/javascript",
        body: readFileSync(
          join(
            client,
            "node_modules",
            path === "react.js"
              ? "react/umd/react.production.min.js"
              : "react-dom/umd/react-dom.production.min.js",
          ),
        ),
      });
    if (path.startsWith("assets/"))
      return route.fulfill({
        body: readFileSync(join(client, "src", path)),
        contentType: path.endsWith(".ttf") ? "font/ttf" : "font/woff2",
      });
    return route.fulfill({
      body: fromArchive(path),
      contentType: path.endsWith(".css") ? "text/css" : "text/javascript",
    });
  });
  const localResources = (file: string) =>
    readFileSync(join(client, "src", file), "utf8").replaceAll(
      "./assets/",
      `${origin}/__pixel/assets/`,
    );
  await reference.route("https://fonts.googleapis.com/**", (route) =>
    route.fulfill({
      contentType: "text/css",
      body: localResources("fonts.css"),
    }),
  );
  await reference.route("https://unpkg.com/@phosphor-icons/**", (route) =>
    route.fulfill({
      contentType: "text/css",
      body: localResources("icons.css"),
    }),
  );
  await actual.goto(origin + "/?view=overview");
  await reference.goto(origin + "/__pixel/index.html");
  await actual.getByRole("button", { name: "网格", exact: true }).waitFor();
  await reference.getByRole("button", { name: "网格", exact: true }).waitFor();
  await Promise.all([
    actual.evaluate(() => document.fonts.ready),
    reference.evaluate(() => document.fonts.ready),
  ]);
  return { actual, reference, close: () => context.close() };
}
async function nav(p: Page, label: string, ref = false) {
  const control = ref
    ? p
        .getByRole("button")
        .filter({ has: p.getByText(label, { exact: true }) })
        .first()
    : p.getByRole("button", { name: label, exact: true }).first();
  await control.click();
  await p.evaluate(() => document.fonts.ready);
}
async function capture(
  id: string,
  label: string,
  actual: Page,
  reference: Page,
  note = "",
  selectors?: [string, string],
) {
  await actual.mouse.move(0, 0);
  await reference.mouse.move(0, 0);
  const a = selectors
    ? await actual.locator(selectors[0]).screenshot({ animations: "disabled" })
    : await actual.screenshot({ animations: "disabled" });
  const b = selectors
    ? await reference
        .locator(selectors[1])
        .screenshot({ animations: "disabled" })
    : await reference.screenshot({ animations: "disabled" });
  writeFileSync(join(output, `${id}-actual.png`), a);
  writeFileSync(join(output, `${id}-reference.png`), b);
  const metric = await actual.evaluate(
    async ([aa, bb]) => {
      async function decode(s: string) {
        const im = new Image();
        im.src = "data:image/png;base64," + s;
        await im.decode();
        const canvas = document.createElement("canvas");
        canvas.width = im.width;
        canvas.height = im.height;
        const ctx = canvas.getContext("2d")!;
        ctx.drawImage(im, 0, 0);
        return {
          im,
          width: im.width,
          height: im.height,
          data: ctx.getImageData(0, 0, im.width, im.height).data,
        };
      }
      const [a, b] = await Promise.all([decode(aa), decode(bb)]);
      const width = Math.max(a.width, b.width),
        height = Math.max(a.height, b.height);
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      const ctx = canvas.getContext("2d")!,
        pixels = ctx.createImageData(width, height);
      let changed = 0,
        maxChannelDelta = 0,
        minX = width,
        minY = height,
        maxX = -1,
        maxY = -1;
      for (let y = 0; y < height; y++)
        for (let x = 0; x < width; x++) {
          const ai = (y * a.width + x) * 4,
            bi = (y * b.width + x) * 4,
            di = (y * width + x) * 4;
          let different =
            x >= a.width || x >= b.width || y >= a.height || y >= b.height;
          for (let c = 0; c < 4; c++) {
            const delta = Math.abs(
              (a.data[ai + c] ?? 0) - (b.data[bi + c] ?? 0),
            );
            if (delta) different = true;
            maxChannelDelta = Math.max(maxChannelDelta, delta);
          }
          if (different) {
            changed++;
            minX = Math.min(x, minX);
            minY = Math.min(y, minY);
            maxX = Math.max(x, maxX);
            maxY = Math.max(y, maxY);
          }
          const gray = Math.round(
            ((b.data[bi] ?? 255) +
              (b.data[bi + 1] ?? 255) +
              (b.data[bi + 2] ?? 255)) /
              3,
          );
          pixels.data.set(
            different ? [225, 29, 116, 255] : [gray, gray, gray, 110],
            di,
          );
        }
      ctx.putImageData(pixels, 0, 0);
      return {
        changed,
        total: width * height,
        ratio: changed / (width * height),
        dimensions: {
          actual: [a.width, a.height],
          reference: [b.width, b.height],
        },
        bounds: changed ? { minX, minY, maxX, maxY } : null,
        maxChannelDelta,
        diff: canvas.toDataURL("image/png"),
      };
    },
    [a.toString("base64"), b.toString("base64")],
  );
  const { diff, ...stats } = metric;
  writeFileSync(
    join(output, `${id}-diff.png`),
    Buffer.from(diff.split(",")[1], "base64"),
  );
  const inspect = (p: Page) =>
    p.evaluate(() => ({
      viewport: [innerWidth, innerHeight],
      document: [
        document.documentElement.scrollWidth,
        document.documentElement.scrollHeight,
      ],
      main: Array.from(document.querySelectorAll("main")).map((el) => ({
        width: el.clientWidth,
        height: el.clientHeight,
        scrollWidth: el.scrollWidth,
        scrollHeight: el.scrollHeight,
        scrollTop: el.scrollTop,
      })),
      font: getComputedStyle(document.body).fontFamily,
    }));
  const record = {
    id,
    label,
    status: stats.changed === 0 ? "PASS" : "FAIL",
    ...stats,
    note,
    scope: selectors ? "component" : "viewport",
    actualGeometry: await inspect(actual),
    referenceGeometry: await inspect(reference),
  };
  results.push(record);
  console.log(
    `${record.status} ${id}: ${stats.changed} / ${stats.total} (${(stats.ratio * 100).toFixed(3)}%)`,
  );
}
async function unsupported(
  id: string,
  label: string,
  actual: Page,
  note: string,
) {
  await actual.screenshot({
    path: join(output, `${id}-actual.png`),
    animations: "disabled",
  });
  results.push({ id, label, status: "NO_REFERENCE", note, screenshot: true });
}
async function scenario(id: string, fn: () => Promise<void>) {
  try {
    await fn();
  } catch (e) {
    results.push({ id, label: id, status: "ERROR", note: String(e) });
    console.error(id, String(e));
  }
}

try {
  await scenario("desktop-single", async () => {
    const p = await pair();
    try {
      await capture(
        "overview-single",
        "总览 / 网格紧凑 / 单会话",
        p.actual,
        p.reference,
      );
      for (const [button, id] of [
        ["详细", "detailed"],
        ["看板", "board"],
        ["列表", "list"],
      ]) {
        await nav(p.actual, button);
        await nav(p.reference, button);
        await capture(
          `overview-${id}`,
          `总览 / ${button} / 单会话`,
          p.actual,
          p.reference,
        );
      }
      await nav(p.actual, "网格");
      await nav(p.reference, "网格");
      await nav(p.actual, "按项目");
      await nav(p.reference, "按项目");
      await capture(
        "overview-project",
        "总览 / 按项目 / 单会话",
        p.actual,
        p.reference,
      );
      await p.actual
        .getByRole("button", { name: row.title, exact: true })
        .click();
      await p.reference.getByText(row.title, { exact: true }).click();
      await capture(
        "drawer-screen",
        "会话抽屉 / 完整屏幕",
        p.actual,
        p.reference,
        "保留原稿原始跳转按钮文字，没有把它改成实际应用文案。",
      );
      await capture(
        "drawer",
        "会话抽屉 / 抽屉本身",
        p.actual,
        p.reference,
        "原稿跳转按钮含运行环境名称，实际按钮为跳到 agent。",
        ['[role="dialog"]', "aside"],
      );
      await p.actual
        .getByRole("button", { name: "完整进度账 →", exact: true })
        .click();
      await p.actual
        .getByRole("heading", { name: "时间线", exact: true })
        .waitFor();
      await unsupported(
        "agent-panel",
        "完整进度账",
        p.actual,
        "压缩包仅有旧版 Agent Panel，未提供 v3 完整详情。当前详情与旧版的侧栏、字体和布局不同，不能验收为 v3 像素一致。",
      );
      await p.actual
        .getByRole("button", { name: "编辑要求", exact: true })
        .click();
      await unsupported(
        "requirements-editor",
        "要求编辑器",
        p.actual,
        "压缩包没有对应编辑器原稿，不能计入像素通过。",
      );
    } finally {
      await p.close();
    }
  });
  await scenario("work-pages", async () => {
    const p = await pair([structuredClone(row)], true);
    try {
      for (const [label, id] of [
        ["待办", "todos"],
        ["目标", "goals"],
        ["回顾", "review"],
        ["设置", "settings-1000"],
      ]) {
        await nav(p.actual, label);
        await nav(p.reference, label, true);
        if (label === "回顾")
          await p.actual
            .getByRole("heading", { name: "还挂着", exact: true })
            .waitFor();
        await capture(
          id,
          `${label} / 完整屏幕`,
          p.actual,
          p.reference,
          label === "目标"
            ? "目标默认显示列表，项目进度图按需打开；原稿没有该入口，保留真实差异。"
            : label === "设置"
              ? "未统一原稿的三处说明文案；保留真实差异。"
              : "",
        );
      }
      await p.actual.setViewportSize({ width: 1440, height: 1200 });
      await p.reference.setViewportSize({ width: 1440, height: 1200 });
      await capture(
        "settings-1200",
        "设置 / 四组可见 / 1440×1200",
        p.actual,
        p.reference,
      );
      await p.actual.setViewportSize({ width: 1440, height: 1000 });
      await p.reference.setViewportSize({ width: 1440, height: 1000 });
      await nav(p.actual, "待办");
      await nav(p.reference, "待办", true);
      await p.actual
        .getByRole("button", { name: "交给 Codex ▾", exact: true })
        .click();
      await p.reference
        .getByRole("button", { name: "交给 Codex ▾", exact: true })
        .click();
      await capture(
        "dispatch",
        "派发预览 / 完整屏幕",
        p.actual,
        p.reference,
        "原稿只读 pre，实际是可编辑 textarea；按钮、提示词和弹窗尺寸不做人工统一。",
      );
      await p.actual.getByRole("button", { name: "关闭", exact: true }).click();
      await p.actual
        .getByRole("button", { name: "＋ 加待办", exact: true })
        .click();
      await unsupported(
        "work-item-editor",
        "新建待办编辑器",
        p.actual,
        "压缩包没有对应编辑器原稿，不能计入像素通过。",
      );
    } finally {
      await p.close();
    }
  });
  const multi = [
    "needs_input",
    "running",
    "review",
    "running",
    "review",
    "accepted",
    "ended",
    "stopped",
  ].map((status, i) => {
    const r = structuredClone(row);
    r.sessionId = `pixel-${i}`;
    r.title = [
      "备份位置等待确认",
      "增量扫描偏离排查",
      "导航重构待核对",
      "生成 OpenAPI 文档",
      "桌面版识别待验收",
      "追加要求已验收",
      "列表闪烁修复结束",
      "数据库迁移已停下",
    ][i];
    r.runtimeId = i % 2 ? "claude-code" : "codex";
    r.state = status as typeof row.state;
    r.lastActiveAt = new Date(Date.parse(now) - (i + 1) * 60000).toISOString();
    if (i === 1)
      r.offPlan = [{ id: "off-1", at: r.lastActiveAt, callIds: ["call-1"] }];
    if (i === 4 || i === 5) {
      r.items.forEach((it) => {
        it.status = "done";
        it.evidenceIds = ["ev-1"];
      });
      r.progress.done = 3;
    }
    return r;
  });
  await scenario("multi-state", async () => {
    const p = await pair(multi);
    try {
      await capture(
        "overview-multi",
        "总览 / 八会话含无缺口待核对 / 紧急度",
        p.actual,
        p.reference,
        "覆盖上次八会话整页对照未包含的无缺口待核对；当前业务也将它归入需要你。",
      );
      for (const button of ["详细", "看板", "列表", "网格", "按项目"]) {
        await nav(p.actual, button);
        await nav(p.reference, button);
        if (button !== "网格")
          await capture(
            `multi-${button}`,
            `八会话 / ${button}`,
            p.actual,
            p.reference,
          );
      }
    } finally {
      await p.close();
    }
  });
  for (const [i, r] of multi.entries())
    await scenario(`state-${i}`, async () => {
      const p = await pair([r]);
      try {
        await nav(p.actual, "按项目");
        await nav(p.reference, "按项目");
        await capture(
          `state-${i}`,
          `状态卡片 / ${r.title}`,
          p.actual,
          p.reference,
          "仅验收卡片，不代表其紧急度分组正确。",
          ['div[role="button"][tabindex="0"]', 'div[role="button"]'],
        );
      } finally {
        await p.close();
      }
    });
  for (const [width, height] of [
    [1920, 1080],
    [390, 844],
  ])
    await scenario(`viewport-${width}`, async () => {
      const p = await pair([structuredClone(row)], true, width, height);
      try {
        await capture(
          `overview-${width}`,
          `总览 / ${width}×${height}`,
          p.actual,
          p.reference,
          width === 390
            ? "原稿没有移动端重排；实际应用实现了响应式导航。功能适配不等于原稿像素通过。"
            : "",
        );
        if (width === 390)
          for (const [label, id] of [
            ["待办", "todos"],
            ["目标", "goals"],
            ["设置", "settings"],
          ]) {
            await nav(p.actual, label);
            await nav(p.reference, label, true);
            await capture(
              `${id}-390`,
              `${label} / 390×844`,
              p.actual,
              p.reference,
            );
          }
      } finally {
        await p.close();
      }
    });
  await scenario("empty-and-fallback", async () => {
    const empty = await pair([]);
    try {
      await capture(
        "overview-empty",
        "总览 / 无会话",
        empty.actual,
        empty.reference,
      );
    } finally {
      await empty.close();
    }
    const r = structuredClone(row);
    r.items = [];
    r.progress = { done: 0, total: 0 };
    const p = await pair([r]);
    try {
      await capture(
        "overview-no-items",
        "总览 / 无确认要求项",
        p.actual,
        p.reference,
      );
    } finally {
      await p.close();
    }
  });
  await scenario("new-reply", async () => {
    const r = structuredClone(row);
    r.state = "review";
    r.items = [];
    r.progress = { done: 0, total: 0 };
    const p = await pair([r]);
    try {
      await capture(
        "overview-new-reply",
        "总览 / 新回复且无确认要求项",
        p.actual,
        p.reference,
        "用户截图中的灰色新回复：原稿无新回复名称；实际新增查看回复操作并将它放入需要你。双方仍采用灰色标签，差异不是白卡底色本身。",
      );
      await nav(p.actual, "按项目");
      await nav(p.reference, "按项目");
      await capture(
        "new-reply-card",
        "灰色新回复 / 单卡",
        p.actual,
        p.reference,
        "原稿待核对与实际新回复的状态文字和操作按钮不同。",
        ['div[role="button"][tabindex="0"]', 'div[role="button"]'],
      );
    } finally {
      await p.close();
    }
  });
  await scenario("original-preview", async () => {
    const p = await pair();
    try {
      // A separate screenshot keeps even the original demo data, with no data overrides.
      const raw = original.replace(
        '<script src="./support.js"></script>',
        '<script>window.__resources={"https://unpkg.com/react@18.3.1/umd/react.production.min.js":"/__pixel/react.js","https://unpkg.com/react-dom@18.3.1/umd/react-dom.production.min.js":"/__pixel/react-dom.js"}</script><script src="./support.js"></script>',
      );
      await p.reference.route("**/__pixel/index.html", (route) =>
        route.fulfill({ contentType: "text/html", body: raw }),
      );
      await p.reference.reload();
      await p.reference
        .getByText("hook 落盘按 agent 分目录并补备份测试", { exact: true })
        .waitFor();
      await p.reference.evaluate(() => document.fonts.ready);
      await p.reference.mouse.move(0, 0);
      await p.reference.screenshot({
        path: join(output, "original-unmodified.png"),
        animations: "disabled",
      });
    } finally {
      await p.close();
    }
  });
  for (const [id, label] of [
    ["sessions", "会话记录"],
    ["orchestrator", "Agent 任务板"],
    ["work", "工作项"],
    ["projects", "项目"],
    ["plans", "计划"],
    ["memory", "记忆"],
    ["analytics", "用量统计"],
  ]) {
    results.push({
      id: `aux-${id}`,
      label: `更多 / ${label}`,
      status: "NO_REFERENCE",
      note:
        id === "orchestrator"
          ? "压缩包 Task Board 是独立旧版设计；v3 没有对应页面。本次未进行该旧版的数据映射与像素验收。"
          : "压缩包没有对应的 v3 页面。本次未进行像素比较，不计为通过。",
    });
  }
} finally {
  await browser.close();
  const productionAfter = treeHash(join(repo, "public/dist"));
  const indexAfter = await (await fetch(origin)).text();
  const stable =
    indexMatchesDisk &&
    productionBefore === productionAfter &&
    sha(indexBefore) === sha(indexAfter);
  const report = {
    generatedAt: new Date().toISOString(),
    origin,
    browser: "Google Chrome",
    scale: 1,
    archive,
    archiveSha256: sha(readFileSync(archive)),
    referenceSha256: sha(original),
    productionBefore,
    productionAfter,
    indexSha256: sha(indexBefore),
    indexMatchesDisk,
    productionStable: stable,
    normalization: [
      "会话、目标、待办、回顾记录与计数使用相同测试数据",
      "时钟固定 2026-10-07 13:02 Asia/Shanghai",
      "字体、图标和 React 使用本地等价资源",
      "不修改原稿模板、CSS、按钮文字、设置说明和派发提示词",
      "动画截图时禁用；鼠标移出内容；不遮罩、不设置容差",
    ],
    verdict: !stable
      ? "INVALID_BUILD_CHANGED"
      : results.some((r) => r.status !== "PASS") || errors.length
        ? "NOT_ACCEPTED"
        : "ACCEPTED",
    summary: Object.fromEntries(
      ["PASS", "FAIL", "NO_REFERENCE", "ERROR"].map((status) => [
        status,
        results.filter((r) => r.status === status).length,
      ]),
    ),
    results,
    errors,
  };
  writeFileSync(join(output, "results.json"), JSON.stringify(report, null, 2));
  const esc = (s: unknown) =>
    String(s ?? "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll('"', "&quot;");
  const rows = results
    .map(
      (r) =>
        `<tr><td><a href="#${esc(r.id)}">${esc(r.label)}</a></td><td class="${r.status}">${r.status}</td><td>${r.changed?.toLocaleString() ?? "—"}</td><td>${r.ratio == null ? "—" : (r.ratio * 100).toFixed(4) + "%"}</td><td>${r.bounds ? `${r.bounds.minX},${r.bounds.minY} → ${r.bounds.maxX},${r.bounds.maxY}` : "—"}</td></tr>`,
    )
    .join("");
  const cards = results
    .map(
      (r) =>
        `<section id="${esc(r.id)}"><h2>${esc(r.label)} <span class="${r.status}">${r.status}</span></h2><p>${esc(r.note)}</p>${r.dimensions ? `<p>实际 ${r.dimensions.actual.join("×")} / 原稿 ${r.dimensions.reference.join("×")} · ${r.changed.toLocaleString()} 差异像素 · ${(r.ratio * 100).toFixed(4)}%</p><div class="comparison"><figure><figcaption>原稿</figcaption><img src="${encodeURI(r.id)}-reference.png"></figure><figure><figcaption>生产应用</figcaption><img src="${encodeURI(r.id)}-actual.png"></figure><figure><figcaption>差异（洋红色）</figcaption><img src="${encodeURI(r.id)}-diff.png"></figure></div>` : r.screenshot ? `<img class="unsupported" src="${encodeURI(r.id)}-actual.png">` : ""}</section>`,
    )
    .join("");
  writeFileSync(
    join(output, "index.html"),
    `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Keepline 像素验收</title><style>body{margin:0;background:#f7f8fa;color:#15171c;font:14px/1.6 system-ui,sans-serif}main{max-width:1500px;margin:auto;padding:30px}h1{font-size:28px}h2{font-size:19px}a{color:#4f5bd5}table{width:100%;border-collapse:collapse;background:white}th,td{padding:10px;text-align:left;border-bottom:1px solid #e6e8ec}.PASS{color:#18743c}.FAIL,.ERROR{color:#c62740}.NO_REFERENCE{color:#966106}section{padding:22px;background:white;border:1px solid #e6e8ec;margin-top:24px;scroll-margin:20px}.comparison{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:10px}figure{margin:0}figcaption{margin-bottom:6px;color:#6b7280}img{max-width:100%;height:auto;border:1px solid #ddd;cursor:zoom-in}.unsupported{max-width:800px}code{overflow-wrap:anywhere}@media(max-width:800px){.comparison{grid-template-columns:1fr}main{padding:16px}}</style><main><h1>Keepline 像素级验收：${report.verdict === "ACCEPTED" ? "通过" : "未通过"}</h1><p>生产页面：${esc(origin)} · 零容差 RGBA 比较 · Chrome · 1×缩放 · 构建${stable ? "保持不变" : "在验收期间发生变化，本轮结果无效"}</p><p>通过 ${report.summary.PASS} / 差异 ${report.summary.FAIL} / 缺少原稿 ${report.summary.NO_REFERENCE} / 执行错误 ${report.summary.ERROR}。每张截图保留完整视口和外壳；组件检查明确单列。只有 0 差异像素才通过，功能测试通过不等于像素验收通过。</p><p>本验收未修改产品代码。没有 v3 原稿的扩展页面、全部长页面滚动位置及其他浏览器不作为已通过。<a href="results.json">原始统计与构建指纹</a></p><details><summary>测试条件与数据归一化</summary><ul>${report.normalization.map((s) => `<li>${esc(s)}</li>`).join("")}</ul><p>ZIP SHA-256：<code>${report.archiveSha256}</code></p><p>生产构建 SHA-256：<code>${productionBefore}</code></p></details><table><thead><tr><th>验收项</th><th>状态</th><th>差异像素</th><th>比例</th><th>差异边界（x,y）</th></tr></thead><tbody>${rows}</tbody></table>${cards}</main><script>document.querySelectorAll('img').forEach(im=>im.onclick=()=>window.open(im.src,'_blank'))</script></html>`,
  );
  console.log(
    JSON.stringify({
      verdict: report.verdict,
      stable,
      summary: report.summary,
      errors,
      output,
    }),
  );
  if (report.verdict !== "ACCEPTED") process.exitCode = 1;
}
