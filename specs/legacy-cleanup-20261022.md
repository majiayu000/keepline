# 旧产品线遗留状态清理清单（2026-10-22）

**这份清单支撑的决定**：旧产品线（Bun 服务 + React Web 面板 + Tauri 菜单栏）留下的未提交改动、PR、worktree、分支和 stash，各自是提交、关闭还是删除，用户一次拍板。
**判断依据**：[产品卡](product-card.md) 第 9 节——Bun 不再常驻；Tauri 与 React 不再作为新 App 的界面，迁移完成前继续承担观察用途；菜单栏由新架构承担，收窄为计数与直达；1.2.0 是否发布由用户决定。
**放弃条件**：某项证据不足以判断时归入「拿不准」，不删除。
**边界**：本清单只做盘点和建议。删除、丢弃、关 PR 都要等用户逐类同意后再执行。不 push main，不改 `.env` 等配置。`feat/agent-workbench`（#147）和它的工作目录不在清理范围内。

证据采集时间：2026-10-22，`main@0d32d2b`（与 `origin/main` 一致），采集前执行过 `git fetch --prune`。「未合并补丁」用 `git cherry` 算，按补丁内容比较，所以压缩合并（squash）过的提交不算未合并；压缩合并的情况另外按提交标题或测试文件是否在 main 中核对。

## 1. main 工作目录的未提交改动

### 内容

这些改动是同一批工作，[maintenance-20261019.md](progress-ledger/maintenance-20261019.md) 记录了它的过程和验证：

| 文件 | 内容 | 判断 |
|---|---|---|
| `LedgerPage.tsx`、`LedgerOverview.tsx`、`LedgerDrawer.tsx`、`Workspace.module.css`、新文件 `LedgerRecap.tsx` | 「轻量接手」：概览卡片预览 Agent 最新回复；主操作只打开详情，不再自动验收或生成追问；详情抽屉显示最新回复、待回答问题、执行与检查、文件变化，用户消息和要求清单默认折叠 | 完整。`LedgerObservation` 只是不在抽屉里用了，`LedgerAgentPanel` 仍在用它，没有留下死代码 |
| `specs/progress-ledger/product.md` | 补「Lightweight handoff (2026-10-18)」一节，和上面的界面改动对应 | 完整 |
| `session-navigation.spec.ts`、`workspace.spec.ts` | 断言改成「先查看、再明确操作」，另新增 4 个轻量接手用例（含 1440 / 390 宽度） | 完整 |
| `package.json`、`src/web/client/package.json`、`src/index.ts`、`src/local-api/routes/meta.ts` | 版本统一为 1.2.0，CLI 和 `/meta` 从根 `package.json` 读版本；`apiVersion` 仍为 `1.0` | 完整 |
| `README.md`、`README_CN.md` | 改为以进度账本为主，删掉旧的主题宣传、费用承诺和截图 | **部分过时**：「当前重心是 1.2.0 Tauri 菜单栏候选」「终端工作台 0/46、未实现」写于 10-19，早于 10-20 定案的产品卡 |
| 新文件 `specs/progress-ledger/maintenance-20261019.md` | 这批改动的维护记录 | 完整 |
| 新文件 `specs/product-card.md`、`specs/terminal-workbench/{product,tech,tasks}.md` | 与 `feat/agent-workbench` 上的提交 `29e0048` 逐字节一致；分支上已有更新版本（最新 10-22） | 不属于这批改动。产品卡必须保留 |

这批改动的轻量接手代码在任何分支里都没有，只存在于 main 工作目录。

### 验证（本会话）

运行时本机 load average 在 500–530 之间，另有 ffmpeg 和多个 Python 任务在跑。

| 检查 | 结果 |
|---|---|
| `bun run typecheck`（根目录与客户端） | 通过 |
| `bun run test:ui`（含 UI 类型检查） | **80/80 通过**，含新增的 4 个轻量接手用例 |
| `bun run test`（后端全量，默认 5 秒超时） | 1144 通过、54 失败、16 跳过。54 个失败分布在 14 个文件，全是超时（5–20 秒） |
| 上面 14 个失败文件放宽到 120 秒超时后重跑 | 182 通过，1 失败 |
| 剩下的 `service-realtime.test.ts` | 测试里写死了 5 秒的通知期限，放宽外层超时不影响它。改动版 0/6 通过；在临时建的干净 `main@0d32d2b` worktree 上 1/2 通过（临时 worktree 已删除）。这个测试不涉及本批改动的代码（本批后端只改了版本号的读取）。在这种负载下，没法确定失败是负载造成的还是和改动有关，**待验证**：负载正常后重跑，通过了再合入 PR |
| `local-api-v1` + `architecture-imports`（覆盖 `/meta` 与导入边界） | 7/7 通过 |

### 建议

1. **提交到新分支并开 PR**（建议分支名 `chore/ledger-handoff-1.2.0`）：包含 13 个已修改文件、`LedgerRecap.tsx` 和 `maintenance-20261019.md`。提交前把中英文 README 的「当前重心」一段改为指向产品卡和新工作台，并删掉「0/46、未实现」这种已过时的描述。理由：轻量接手是旧界面在迁移期内继续承担观察用途时用得上的改进，测试齐全，丢掉要重做。
   - 做法：用临时 index 生成提交并建分支，不切换 main 工作目录的分支，也不动其他会话。
2. `specs/product-card.md` 和 `specs/terminal-workbench/` **保留原样，不进这个 PR**，它们会随 #147 合入。注意：#147 合入 main 前，要先移走 main 工作目录里这几份未跟踪的副本，否则 `git merge` 会因为「未跟踪文件将被覆盖」而中止。副本和 `29e0048` 一致，已有提交保存。

## 2. #145 与 1.2.0 菜单栏候选

| 项 | 证据 |
|---|---|
| [#145](https://github.com/majiayu000/keepline/pull/145) | Draft，10-13 创建，10-15 后没有更新；head `ab15e9d`；CI 3/3 通过；可以干净合入 main。改动集中在 `menubar-tauri/src-tauri/src/ledger_app.rs`（+149/−22）和验收记录 |
| #145 的阻塞 | PR 正文说明，实际 App 报 `Notifications are not allowed for this application`：通知能否送达、点击能否跳转、冷启动 / 登录周期都没验收；临时签名包通不过 Gatekeeper |
| 1.2.0 候选 | `menubar-tauri` 版本已是 1.2.0，但没有 tag 和 release。最新 release 是 v1.1.4（2026-09-26） |
| 产品卡 | 菜单栏改由 GPUI 客户端或轻量原生组件承担，收窄为计数与直达；Tauri 不作为新 App 的界面。新工作台已有通知投递与点击路由的提交（`89457cb`，在 #147 里） |

**建议：关闭 #145（不合并），1.2.0 不发布。**

- 要合入 #145，先得在已弃用的界面上补完原生通知验收。验收需要正式签名和通知权限，工作量不小，换来的是一个按产品卡不再发布的菜单栏。
- 关闭 PR 不删远端分支 `harness/runtime-wf-github-issue-pr-7504df31`，以后要参照还在。
- 如果用户仍想发 1.2.0，就保留 #145，补完验收后再合入并走发布授权。这条路可选，但和产品卡的方向相反。

## 3. Worktree（共 24 个，含 main 自身）

列说明：「领先 main」= `main..HEAD` 的提交数；「未合并补丁」= `git cherry main` 里 `+` 的数量；「远端」= 是否有远端分支包含这个 HEAD；「占用」= 是否有进程以它为工作目录（`lsof`）。

### 3A. 可以安全删除（14 个）

删除方式：`git worktree remove`，没有改动的直接删。分支一起删，只删这里列出的本地分支。

| 路径 | 分支 / HEAD | 最后提交 | 未提交 | 合并证据 |
|---|---|---|---|---|
| `/private/tmp/claude-502/.../4b4d8ed4-.../scratchpad/main-base` | 游离 `0d32d2b` | 10-13 | 0 | 就是 main 当前提交 |
| `../keepline-main-integration` | 游离 `09836d3` | 10-06 | 0 | 是 main 的祖先 |
| `../keepline-dense-overview` | `feat/dense-agent-overview` | 09-03 | 0 | 是 main 的祖先（#113） |
| `../keepline-candidate-20261013` | `fix/desktop-candidate-20261013` | 10-13 | 0 | #144 已压缩合并为 `6b68db9`；远端有 |
| `../keepline-closeout` | `fix/ledger-closeout-20261007` | 10-08 | 0 | #141 已压缩合并为 `65cf2d0`；远端有 |
| `../keepline-menubar-pr` | `feat/menubar-panel-polish` | 10-11 | 0 | #143 以「已由 #144 整合」关闭；本地 1 个 rebase 前的旧提交 `4b143db` |
| `../keepline-codex-daemon` | `feat/codex-daemon-association` | 10-21 | 1（空目录 `next/.deps`） | 36 个提交都在 `origin/feat/agent-workbench` 中 |
| `../output/opensource-execution-20261015/keepline-worktree` | `codex/keepline-native-receipts-20261015` | 10-15 | 0 | HEAD `ab15e9d` 就是 #145 远端分支的 head |
| `../outputs/threads-20261015-queue/keepline-review` | 游离 `9d6dbe5` | 10-13 | 0 | 在 #145 远端分支中 |
| `~/Desktop/code/work/life/.codex/threads/20261007-owner-queue/worktrees/keeper140-actor1a68-combo` | `fix/keeper140-actor1a68-combo` | 10-08 | 0 | 是 main 的祖先（#140） |
| `.../keeper140-actor34b-combo` | `fix/keeper140-actor34b-combo-20261008` | 10-08 | 0 | 1 个补丁不同，但它的测试 `sed text payloads preserve prior successful checks` 已在 main |
| `.../keepline-contract-current` | `codex/keepline-contract-current` | 10-08 | 0 | 1 个补丁不同，但它的测试 `real git … output-file …` 已在 main |
| `.../keepline-four-current-boundary-fix` | `codex/keepline-four-current-boundary-fix` | 10-08 | 0 | 是 main 的祖先 |
| `.../keepline-three-current-fix` | `fix/ledger-three-current-boundaries-20261008` | 10-08 | 0 | 是 main 的祖先 |

（`keepline-codex-daemon` 的 `next/.deps` 是空目录，大小为 0。）

### 3B. 有未合并的工作，保留（7 个）

| 路径 | 分支 | 最后提交 | 未提交 | 为什么保留 |
|---|---|---|---|---|
| `../keepline-agent-workbench` | `feat/agent-workbench` | 10-22 | 0 | 正在用（#147），按要求保留 |
| `.claude/worktrees/a14-foreground-cpu-a4e2e5` | `claude/a14-foreground-cpu-a4e2e5` | 10-22 | 2 | 会话 a14-cpu 正在用，有进程以它为工作目录 |
| `../keepline-terminal-parity` | `feat/terminal-parity` | 10-20 | **10**（7 个 Rust 文件 +1659/−167，新增 `mouse.rs` 和验收证据目录） | 4 个提交已在工作台分支，但未提交的鼠标、选区、粘贴对齐工作在任何分支里都没有 |
| `../keepline-terminal-replies` | `feat/terminal-replies` | 10-21 | 0 | 43 个提交里有 42 个已进工作台；`457aec1`（终端查询应答、Cmd+K 等日常操作）不在工作台，也没推到远端 |
| `.claude/worktrees/inbox-closure-0fb864` | `claude/inbox-closure-0fb864` | 10-20 | 0 | 5 个提交（鸟瞰网格、「你在这里」抽屉、准确性抽查报告），没推远端，也不在工作台 |
| `.claude/worktrees/terminal-eval-04a203` | `claude/terminal-eval-04a203` | 10-20 | 0 | 10 个提交，没推远端；产品卡第 3.1 节把它的 `eval/terminal/` 当作定案依据引用 |
| `.claude/worktrees/terminal-p0-9def3b` | `claude/terminal-p0-9def3b` | 10-21 | 0 | 2 个提交（P0 打包 App 生命周期、xterm.js 镜像），没推远端 |

后五个都属于新工作台线，不是旧线遗留。要不要并入工作台，由负责 #147 的会话决定。建议至少把这几个本地独有的分支推到远端备份；推送要用户另外授权。

### 3C. 拿不准（2 个）

| 路径 | 分支 | 最后提交 | 未提交 | 证据 |
|---|---|---|---|---|
| `../keepline-realtime` | `feat/realtime-updates` | 10-06 | **66**（45 个修改 +1572/−1135，21 个未跟踪） | 3 个提交按标题都在 main。未提交的 115 个文件 main 里都有；其中 73 个与 #141 系列的整合提交 `7683118`（10-08 00:04）一致，其余 42 个是更早的草稿（例如 `package.json` 还写着 `claude-hub` 的仓库地址）。文件最后修改于 10-07 23:45，早于整合提交，基本可以确定已被 #141 吸收 |
| `~/Desktop/code/AI/agent/Claude-Code-Monitor-runtime-neutral-spec` | `docs/GH44-runtime-neutral-spec` | 06-18 | 1（未跟踪的 `specs/GH44/product.md`、`tech.md`） | 分支是 main 的祖先。两个文件 main 里都有但内容不同：工作目录版本修改于 06-22，main 版本 06-23 才最后更新，内容更多（tech.md 中 main 独有 292 行，草稿独有 83 行）。草稿基本可以确定是旧版本 |

**建议**：两个都先备份（`git diff` 加未跟踪文件打包）到 `~/Desktop/code/AI/tools/keepline-legacy-backup-20261022/`，再删 worktree 和分支。

## 4. 没有挂 worktree 的本地分支（25 个）

| 类别 | 分支 | 证据 |
|---|---|---|
| 可删：已是 main 的祖先 | `codex/spec-closure-orchestrator-tests`、`feature/cyberpunk-ui`、`feature/optimizations`、`feature/stash-integration-service`、`fix/GH74-lancedb-id-validation`、`fix/issue-85-orchestrator-task-source`、`fix/startup-summary-cache`、`implx/GH99-105-queue` | `merge-base --is-ancestor` 成立 |
| 可删：没有未合并补丁 | `docs/progress-ledger-spec`、`docs/project-workspaces-spec`、`feat/issue-63-attention-overview`、`feat/issues-64-66-orchestrator-complete`、`integrate/progress-ledger-main` | `git cherry` 没有 `+` |
| 可删：已压缩合并 | `feat/codex-hub-rebrand-detection`（→`16c062b`）、`feat/project-workspaces`（→`64e5914`）、`fix/issues-36-39`（→`bb57686`）、`fix/restart-reconciliation`（→`960277d`）、`fix/issue-75-hook-pipeline`（→`3fa1487`）、`backup/main-before-origin-main-20260624-215140`（→`8e626c1`、`0efb547`） | 提交标题能在 main 中找到对应的合并提交 |
| 可删：已被其他方式取代 | `feat/progress-ledger`（`685b371`、`f7ea194` 的 `specs/ui-v2` 证据已在 main，经 #141 合入）、`feat/GH47-runtime-registry`（`runtime.registry.test.ts` 已在 main）、`fix/lancedb-injection-and-completed-rollback`（#95 已关闭，由 #84 和 #89 取代）、`docs/agent-workbench-product-card`（`29e0048` 已在工作台分支） | 见括号 |
| 拿不准，倾向删 | `fix/work-items-panel-width`（1 个提交，07-02，一个文件 +49/−3，看板宽度修复；main 里没找到同名提交，旧 Workboard 界面按产品卡已搁置） | 先备份为 patch 再删 |
| **保留** | `wip/close-stops-terminals`（10-22，43 个未合并补丁，没推远端，新工作台线） | 今天还在用 |

## 5. Stash（3 个）

| Stash | 时间 | 来源 | 规模 | 和 main 对比 | 建议 |
|---|---|---|---|---|---|
| `stash@{0}` | 10-13 | `feat/progress-ledger`，切回 main 前存的 | 92 个文件，+3187/−589，含 29 个未跟踪文件 | 121 个文件 main 里都有，其中 65 个完全一致；其余是 10-11 的版本，之后被 #144（10-13）覆盖 | 备份后 drop |
| `stash@{1}` | 09-17 | `fix/restart-reconciliation`，向 main 发布启动改进前存的 | 6 个文件 +93/−8，含 2 个未跟踪文件 | 新增的代码行中，65 行在 main 找到、10 行没找到（初始扫描超时等写法后来改过）；#112 已合并 | 备份后 drop |
| `stash@{2}` | 06-24 | 旧 main | 41 个文件 +877/−109，含 17 个未跟踪文件 | 有 9 个文件 main 里没有：Web 终端面板（`TerminalPanel.tsx`、`terminal-websocket.ts`）、Workboard PRD 和原型 | 备份后 drop。Web 内嵌终端的方向已被产品卡否定（Tauri / WebView / xterm.js） |

备份方式：每个 stash 导出 `git stash show -p --include-untracked` 补丁到上面的备份目录，再 drop。

## 6. 待用户拍板的事项

1. main 改动：按第 1 节开新分支和 PR（含 README 定位修正）／原样保留不动／丢弃。
2. #145：关闭（不合并），1.2.0 不发布／保留，等补完验收。
3. 安全删除：3A 的 14 个 worktree 和对应分支，加第 4 节「可删」的 23 个分支。
4. 拿不准的：`keepline-realtime`、`runtime-neutral-spec`、`fix/work-items-panel-width`、3 个 stash，备份后删除／保留。
5. （可选，需另外授权推送）把 3B 里本地独有的分支推到远端备份。

## 执行记录（2026-10-22）

四类建议用户在会话里全部按推荐项确认。

| 事项 | 结果 |
|---|---|
| 备份 | `~/Desktop/code/AI/tools/keepline-legacy-backup-20261022/`：`realtime-tracked.diff`、`realtime-untracked.tgz`（70 个文件）、`runtime-neutral-spec-GH44.tgz`、`fix-work-items-panel-width.patch`、`stash{0,1,2}.patch` 及各自的 SHA 和说明；另有 `deleted-branches.txt` 和 `deleted-worktrees.txt`，记录删除前每个分支和 worktree 的 SHA |
| worktree | 删除 16 个（3A 的 14 个 + 3C 的 2 个），删除前复核过：改动数与盘点一致，没有进程以它们为工作目录。剩 8 个：main、`keepline-agent-workbench`、`keepline-terminal-parity`、`keepline-terminal-replies` 和 `.claude/worktrees/` 下的 4 个 |
| 本地分支 | 删除 37 个，远端分支都没动。剩 9 个：main、`feat/agent-workbench`、`feat/terminal-parity`、`feat/terminal-replies`、`wip/close-stops-terminals` 和 4 个 `claude/*` |
| stash | 先核对 SHA 与备份一致，再 drop 3 个，`git stash list` 已为空 |
| main 改动 | 中英文 README 的定位改为指向产品卡和 #147，删掉「0/46、未实现」，注明 1.2.0 菜单栏候选不计划发布。连同轻量接手、版本同步、维护记录和这份清单，提交到新分支 `chore/ledger-handoff-1.2.0` 并开 Draft PR；main 工作目录没有切换分支。`specs/product-card.md` 和 `specs/terminal-workbench/` 不在 PR 里，原样保留 |
| #145 | 关闭，没有合并，附关闭说明；远端分支保留 |

恢复方式：分支用 `git branch <名字> <deleted-branches.txt 里的 SHA>` 恢复（对象在 `git gc` 前都还在）；stash 和未提交改动用备份目录里的 patch 或 tgz 恢复。
