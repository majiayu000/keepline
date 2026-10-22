# 体验走查（2026-10-22，隔离 runtime）

**决定：13 个问题全部处理：第一轮修 4 个（2026-10-22 上午），第二轮修其余 9 个（下午，其中 #9 已被第一轮的修复顺带解决）。** 背景：用户 2026-10-22 决定先做功能和体验，性能放到首版之后。

## 方法与边界

- 本会话没有辅助功能 / Computer Use 权限，**不能真实点击**。走查用客户端的 `--visual-evidence` 模式，在隔离 runtime 中把各页面的真实 GPUI 状态渲染成原生截图（1440×900 @2x），逐页检查。
- 夹具：[waiting-capture 的 40 会话夹具](../waiting-capture-20261021/fixture.py)，含 5 个空间、3 条待办，以及等待、出错、待核对、暂停、运行、很久没动、暂存、完成各种状态，其中一个画面只在屏幕上出现审批。首次打开用全新的空 runtime。全程没有模型调用，不读用户日志和凭据。
- 覆盖的流程：首次打开（[first-open/](first-open/)）、主工作区与“你在这里”（Main）、鸟瞰（Overview）、新建待办（TodoNew、QuickTask）、待办详情（TodoDetail）、从待办开始（StartPick、TodoStart）、屏幕审批（ScreenWait）、清理很久没动的会话（Cleanup）、菜单栏面板（Menubar）。修复前的截图在 [native/](native/)，修复后的在 [after/](after/)。
- **没有真实走过的点击链**：从鸟瞰点卡片继续、设置结局、暂存到期回到未闭环、从菜单栏直达画面。这些状态有截图，逻辑有自动测试（M2/M3/M4 及 workspace 测试），但点击本身需要用户或 Computer Use 复验。

## 问题清单（按严重程度）

| # | 严重度 | 问题 | 处理 |
|---|---|---|---|
| 1 | 高 | 从待办开始时，如果 Agent 命令不在 PATH 里（`zsh: command not found: codex`），提示条一直显示“就绪后放入待办内容”，约 60 秒后才变成“没准备好”，用户不知道原因。根因：交接逻辑读屏幕最后 40 行，画面高于 40 行、shell 又在顶部时读到的全是空行，所以既看不到 shell 的报错，也看不到 Agent 的信任/登录菜单。 | **已修** 15e9f53：改为读整个可见区并去掉末尾空行；识别 zsh/bash/fish 的“找不到命令”，提示“找不到 Codex 命令…先安装它或把它加进 PATH，再点「再试一次」”，“再试一次”会重新运行命令。见 [after/todostart-native.png](after/todostart-native.png)。 |
| 2 | 高 | 在 Keepline 画面里用 `--session-id` 启动的会话，只因工作目录在 /tmp 就被当成“临时目录会话”隐藏：不进“需要你”，没有角标和菜单栏条目，等审批也不提示。复验 A04 时发现。 | **已修** 510c77a：临时目录只隐藏非 Keepline 启动的会话；子代理、SDK、`-p` 照旧隐藏。 |
| 3 | 中 | 用户 Cmd+V 多行文本到没开 bracketed paste 的程序（如 cat）时被直接拒绝（A03）。 | **已修** ac70752：先确认，再粘贴，与 Ghostty 的粘贴保护一致；Keepline 自己的预填仍然拒绝多行。 |
| 4 | 中 | “你在这里”卡、卡片菜单和待办详情在没有结局时也显示“撤销结局”，结局已设定时还会重复提供同一个结局。 | **已修** 9c4e3fc。见 [after/main-native.png](after/main-native.png)。 |
| 5 | 中 | 首次打开只有一个空 shell 和空的待办区，没有任何引导：用户不知道要在画面里运行 codex / claude，还是先建待办。 | **已修** 2453a9a：没有待办的空间在待办区显示一句引导（可直接在终端里运行 codex / claude，Keepline 自动跟踪）和“＋ 新建待办”。前 [first-open/main-native.png](first-open/main-native.png) → 后 [after/first-open-native.png](after/first-open-native.png)。 |
| 6 | 中 | 待办“开始”的 Agent 选择器（选 Agent：Codex / Claude Code / 取消）出现在待办列表下方，看不出是为哪条待办选的。见 [native/startpick-native.png](native/startpick-native.png)。 | **已修** cdb2efc：选择器本来就插在对应待办行下面，问题是只写“选 Agent”。现在写“「待办标题」用哪个 Agent”。前 [native/startpick-native.png](native/startpick-native.png) → 后 [after/startpick-native.png](after/startpick-native.png)。 |
| 7 | 中 | 从 Finder 或 Dock 启动时，进程的 cwd 是 `/`，默认空间“工作空间”的目录可能就是 `/`，新画面也从 `/` 开始。 | **已确认并修复** 6d3f108：隔离测试包经 `open -n`（与 Finder、Dock 一样走 LaunchServices）启动时，默认空间和第一个终端都在 `/`（[cwd-before.json](after/cwd-before.json)）。修复后都在家目录（[cwd-after.json](after/cwd-after.json)），复现脚本见 [cwd-probe.py](cwd-probe.py)。从 shell 启动时仍沿用当时的目录；已经建好的空间不改。 |
| 8 | 低 | “需要你”为 0 时，“跳到下一个等你的 →”仍然显示。 | **已修** 2453a9a：只在有会话需要你时才显示。后 [after/first-open-native.png](after/first-open-native.png)。 |
| 9 | 低 | 待办详情有 7 个操作按钮，换行后“返回画面”单独占一行。 | **已修** 9c4e3fc：去掉误显示的“撤销结局”后只剩 6 个按钮，一行放得下，不需要另改。前 [native/tododetail-native.png](native/tododetail-native.png) → 后 [after/tododetail-native.png](after/tododetail-native.png)。 |
| 10 | 低 | 标签标题用等宽字体（`CLI 问题反馈` 字距偏宽），与左栏、标题栏不一致。 | **已修** 0331068：标签改用界面字体。Main 设计稿是界面字体，TodoStart 稿用的是等宽字体，两份稿子本身不一致，这里以 Main 为准。后 [after/main-native.png](after/main-native.png)。 |
| 11 | 低 | 清理页的“你要它”与标题重复，信息量为零。 | **已修** ef0587d：“你要它”与标题相同时不再显示这一行。前 [native/cleanup-native.png](native/cleanup-native.png) → 后 [after/cleanup-native.png](after/cleanup-native.png)。 |
| 12 | 低 | 菜单栏面板固定 360×480，内容只有两条时下半部分空白。 | **已修** 82875c6：面板高度跟随内容，最高到设计高度 480。前 [native/menubar-native.png](native/menubar-native.png) → 后 [after/menubar-native.png](after/menubar-native.png)。 |
| 13 | 低 | 菜单栏写“0 个出错”，鸟瞰却有 4 个“出错了”。两者差在外部会话（菜单栏按设计只计托管会话），但面板上没有说明。 | **已修（只改文案）** 9adfa7b：页脚加一行“等你、出错只计 Keepline 里的画面；外部会话在鸟瞰里看”，计数口径不变。后 [after/menubar-native.png](after/menubar-native.png)。 |

## 检查

每个修复提交前都跑了 `cargo fmt --check`、`cargo clippy --workspace --all-targets -D warnings` 和 `cargo test --workspace`。最后一次全套是 107 项通过加 1 个既有 ignored，连续两遍全绿。机器高负载（load average 100 以上）时，集成测试里有几项会偶发超时；单独跑或负载降下来后都能通过，换回 HEAD 的源码也能复现同样的失败，详见 [A15 记录](../a15-real-replies-20261022/README.md)。

## 第二轮（#5–#13）

- 修复前的截图沿用第一轮的 [native/](native/) 和 [first-open/](first-open/)，修复后的放在 [after/](after/)；截图都由同一个 40 会话夹具、或全新的空 runtime 原生渲染。
- 每个主题单独提交；提交前都只把该主题的改动放回工作区，再跑 fmt、clippy -D warnings 和 `cargo test --workspace`。
- 当时本机负载在 30–85 之间，集成测试每轮有一项偶发失败：`two_cli_conversations…`（等不到 hook 状态文件）、`fsevents_delivers…`、`forty_idle_terminals…`（失败特征与修改之前那次一样）。它们单独跑都通过（`two_cli_conversations…` 连续 3 次）。全部改动叠加后的最终版本全套 107 项通过（另 1 项 ignored），这也是截图所用的版本。
- 仍需用户真实点一遍的：首次打开的“＋ 新建待办”、Agent 选择器的点选、菜单栏面板的弹出高度，以及从 Finder 真正双击启动后的默认目录。本轮的 `open -n` 与 Finder 走同一条 LaunchServices 路径，但不等于 Finder 双击本身。
