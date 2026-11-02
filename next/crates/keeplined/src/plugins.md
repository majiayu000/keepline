# 改 Keepline：插件和外观

给 Coding Agent 看：用户让你给 Keepline 加个命令、换个左栏排法、在窗格上显示点什么、事件来了发通知、换完成动画或改状态颜色时，照这份做。全部是写文件，不用编译 Keepline。

## 在哪

配置目录是 `${XDG_CONFIG_HOME:-$HOME/.config}/keepline`，下面简称 `<配置>`。

- 插件：`<配置>/plugins/<id>/`，一个目录一个插件。
- 状态颜色：`<配置>/colors.json`。
- 快捷键、终端字体、完成动画、钉住提问这些在 App 的「设置」里改，别手改它们的文件。

## 规矩

- **一个想法一个插件**，新建目录，别把新东西塞进已有插件。
- 别改用户已有的插件，也别改 `plugins/disabled.json`（那是设置页的开关），用户明确要求时才动。
- 改完一定跑 `keeplined plugins`，全是 ✓ 才算写好；有 ✗ 就按它说的改，再跑。
- 插件在 Keepline **下次启动时**生效。跟用户说清楚：重启 Keepline 后在哪能看到；设置 › 插件能看到每个插件生效了没有，也能关掉。
- 插件只能用下面列的这几类。做不到的（自己画界面、改会话状态、读对话原文）就直说做不到，别改 Keepline 的安装文件。

## 步骤

```sh
d="${XDG_CONFIG_HOME:-$HOME/.config}/keepline/plugins/git-tools"
mkdir -p "$d"
# 写 $d/plugin.json 和它提供的文件（见下）
keeplined plugins            # 检查；有问题以 1 退出
```

## plugin.json

```json
{
  "api": 1,
  "id": "git-tools",
  "name": "Git 小工具",
  "version": "0.1.0",
  "description": "右键菜单里看 git 状态和改动",
  "provides": ["actions"]
}
```

- `api` 固定写 `1`。`id` 只用小写字母、数字和 `-`，不超过 40 个字，最好和目录名一样；和别的插件重名时后一个不加载。
- `name` 必填；`version`、`description` 可选；不认识的字段整份拒绝。
- `provides` 至少一种：`actions` `sidebar` `decorations` `hooks` `rules` `effects` `agent`。写了哪种就必须有那种的文件；有文件没写进 `provides` 的不会加载。
- 每个类型文件里也都写 `"api": 1`（`agent.json` 除外，见最后），不认识的字段同样整份拒绝。

## actions：右键菜单和 ⌘P 里的命令（`actions.json`）

```json
{
  "api": 1,
  "actions": [
    {"id": "git-diff", "title": "看这里的改动", "command": "git -C {cwd} diff --stat && git -C {cwd} diff", "run": "split"},
    {"id": "resume-tab", "title": "在新标签里接着这个 Claude 会话", "command": "claude --resume {session_id}", "run": "tab", "when": "agent"}
  ]
}
```

- `command` 里能用 `{cwd}` `{session_id}` `{transcript}` `{task}` `{agent}`，代入时会自动加引号，别自己再包引号。
- `run`：`split`（默认，在旁边开一块分屏跑）、`tab`（新标签）、`paste`（打进当前终端但不回车，让用户看了再发）。
- `when`：`always`（默认）、`agent`（窗格里有 Agent 会话）、`shell`（只是 shell）、`task`（标签挂着待办）。

## sidebar：左栏的排法（`sidebar.json`）

```json
{
  "api": 1,
  "presets": [
    {"id": "focus", "name": "专注", "group": "status", "fold": ["running", "done", "stopped", "fresh", "stale"], "detail": "always", "limit": 8}
  ]
}
```

- `group`：`flat`（一列，需要你的在前）、`status`（按状态分段）、`directory`（需要你的置顶，其余按目录分）。
- `fold`：哪些段收成一行，点开才展开。可选 `running` `done`（等你验收）`stopped` `fresh` `stale`；`attention`（需要你）不能收。
- `detail`：会话行什么时候显示第二行：`never`（默认）、`attention`（需要你的和选中的）、`always`。
- `limit`：左栏最多列几行，剩下的进鸟瞰，默认 12。
- 新排法出现在左栏顶部的切换里，和内置的「经典 / 状态 / 目录」并列，不会替换它们。
- 还有 `"group": "wasm"` 加 `"wasm": "<文件>.wasm"`，用沙盒里的 WebAssembly 自由排；要用 Rust 编译，没被要求就别用。

## decorations：窗格上显示一小段命令输出（`decorations.json`）

```json
{
  "api": 1,
  "decorations": [
    {"id": "node", "command": "[ -f package.json ] && node -v", "place": "title", "refresh_s": 60},
    {"id": "last-commit", "command": "git log -1 --format='最近提交 %cr：%s' 2>/dev/null", "place": "tip", "refresh_s": 30}
  ]
}
```

- `command` 用 `sh -c` 在窗格目录里跑，显示输出的第一行；退出码不是 0 或没输出就什么都不显示，所以不适用的目录让它安静失败。
- 能读到 `KEEPLINE_CWD` `KEEPLINE_AGENT` `KEEPLINE_SESSION`，以及 shell 上一条命令的 `KEEPLINE_LAST_EXIT` `KEEPLINE_LAST_DURATION_MS`（还没有时为空）。
- `place`：`title`（默认，窗格标题栏）或 `tip`（悬停标题时的卡片里单独一行）。
- `tone`（只对标题栏）：`muted`（默认）、`ok`、`warn`、`error`，跟着状态颜色。
- `when`：`always`（默认）、`agent`、`shell`。`refresh_s` 多久重跑一次，默认 10；`timeout_ms` 默认 1000。命令要快，别在这里联网。

## hooks：会话变化或命令跑完时执行（`hooks.json`）

```json
{
  "api": 1,
  "hooks": [
    {"id": "notify", "on": ["waiting", "error", "done"], "command": "osascript -e \"display notification \\\"$KEEPLINE_DETAIL\\\" with title \\\"$KEEPLINE_TITLE\\\"\"", "timeout_ms": 5000}
  ]
}
```

- `on`：`started` `waiting` `error` `done`（会话），`command_done` `command_failed`（shell 里敲的命令跑完）。
- `command`（`sh -c`）和 `webhook`（POST 一个 JSON）二选一或都写。事件以 JSON 从标准输入给，也有环境变量：`KEEPLINE_EVENT` `KEEPLINE_SESSION` `KEEPLINE_AGENT` `KEEPLINE_TITLE` `KEEPLINE_CWD` `KEEPLINE_SPACE` `KEEPLINE_DETAIL` `KEEPLINE_TERMINAL`，命令事件还有 `KEEPLINE_EXIT_CODE` `KEEPLINE_DURATION_MS`。
- `timeout_ms` 默认 5000，最多 30000；`min_ms` 只对命令事件，跑得比它快的不报。
- hook 只能知道，不能改会话；拿不到对话原文，也拿不到命令行本身。Keepline 没开时后台也会跑 hook。

## rules：哪些命令算跑过验证（`rules.json`）

```json
{"api": 1, "verify": [["just", "test"], ["./scripts/ci.sh"], ["task", "*"]]}
```

每条是命令开头的几个词，按最后一段路径比（`./node_modules/.bin/vitest` 算 `vitest`），`*` 匹配任意一个词。Agent 跑了这些命令，退出码和测试数会算作验证证据。只能加，不能去掉内置的。

## effects：标记完成时的动画（`effects.json`）

```json
{"api": 1, "complete": {"style": "stamp", "duration_ms": 2400, "land": "sidebar"}}
```

- `style`：`seal`（印章，默认）、`stamp`（落印）、`sweep`（扫光合上）、`dissolve`（化进侧栏）、`fade`（只淡出）。
- `duration_ms` 在 600–4000 之间；`land`：`sidebar`（落到侧栏那一行）或 `none`。
- 多个插件都设时第一个生效；用户在设置里选的优先于所有插件；系统开了「减弱动态效果」时一律只淡出。

## agent：让 Keepline 认识一个新 CLI（`agent.json` 或 `agents/*.json`）

这类最复杂：声明 CLI 的可执行文件、启动和恢复命令，以及怎么从它的会话记录（JSONL 或 SQLite）里读出回合状态。用户要接一个新 CLI 时才写。先找一个现成的例子照着改（Keepline 源码里的 `next/plugins/opencode/agent.json`），写完同样用 `keeplined plugins` 检查。

## 状态颜色（`<配置>/colors.json`）

```json
{
  "status": {"working": "#e8c547", "done": "#4cc38a"},
  "seen": {"title": "#b9b7b4", "status": "#7d7c7a", "dot": "ring"}
}
```

`status` 的键：`waiting` `error` `review` `paused` `running` `stale` `working` `done` `ready`；`seen` 是看过的会话行，`dot` 取 `ring` `fill` `fade`。缺的、读不了的保持默认。同样下次启动生效。

## 例子

用户说「右键菜单里加一个用 VS Code 打开这个目录」：

```sh
d="${XDG_CONFIG_HOME:-$HOME/.config}/keepline/plugins/open-in-vscode"
mkdir -p "$d"
cat > "$d/plugin.json" <<'JSON'
{"api": 1, "id": "open-in-vscode", "name": "用 VS Code 打开", "provides": ["actions"]}
JSON
cat > "$d/actions.json" <<'JSON'
{"api": 1, "actions": [{"id": "code", "title": "用 VS Code 打开这个目录", "command": "code {cwd}", "run": "paste"}]}
JSON
keeplined plugins
```

全是 ✓ 后告诉用户：重启 Keepline，在窗格上右键或 ⌘P 搜「VS Code」就能看到。
