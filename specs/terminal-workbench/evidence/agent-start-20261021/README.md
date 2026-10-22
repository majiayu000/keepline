# 从待办开始：选 Agent、直接启动、内容进 Agent 输入框（2026-10-21）

依据 product.md 6.1 / 6.4（取消预填命令）。

- 要支撑的决定：从待办开始时，提示词放在哪里编辑、怎样交给 Agent。
- 起因：用户实测时，旧设计把待办拼成带引号的 shell 参数预填在命令行。用户在右引号后补写了内容，变成多余参数，codex 报错。
- 结论：**采用**。同类产品（Herdr、Claude Squad）都是先启动 Agent，再把文字放进 Agent 自己的输入框。

## 实现

- `Task::prompt()` 替代 `Task::command()`：只放填了的部分，空字段不留标题。启动命令去掉 `--no-daemon`；Claude 仍带 `--session-id`。
- `Pane::start_agent`：
  - shell 就绪后（打开了 bracketed paste，没有的 shell 等 2 秒静止）只输入启动命令并回车；
  - 等 shell 关掉 bracketed paste、Agent 再打开它，屏幕静止约 0.5 秒，且屏幕上没有带光标的编号菜单（目录信任、登录、审批）；
  - 然后用 bracketed paste 把内容放进 Agent 输入框，不追加回车；
  - 60 秒没就绪就不放。
- 只在启动期间每 250 毫秒检查一次，结束即停，空闲画面没有定时器。
- `Pane::place_text`：用户手动启动的 Agent 用「放入待办内容」按钮，走同样的就绪判断。
- 待办「开始」「再开一个会话」以及鸟瞰卡片上的「开始」，先弹「选 Agent」（Codex / Claude Code，上次用的排前面并高亮）。已有画面的待办点「开始」直接切回去。画面菜单里的 Agent 选项去掉「（仅预填）」。
- 画面上方的说明条随状态变化：
  - 「为待办 X 启动 Codex，就绪后放入待办内容」
  - 「已放入待办内容，检查后在 Agent 里按回车发送」
  - 「Agent 还没准备好，内容没有放进去」，附「再试一次」
- 说明条上另有「复制内容」「收起」，以及没写验收标准时的「补写」，旧的「改用另一个 Agent」「清空命令」已删除。

## 证据

- 原生全链路（[fixture.py](fixture.py) + [fake_codex.py](fake_codex.py)）：
  - 真实 keeplined、真实 PTY、`zsh -f`、GPUI 客户端；PATH 上放一个隔离的 `codex` 替身，它会打开 bracketed paste 并原样回显收到的内容，只有单独回车才显示「[已发送]」。
  - [native/todostart-native.png](native/todostart-native.png)：shell 里只出现 `codex`；替身启动后，标题、说明和三条验收标准出现在 Agent 输入行；没有「[已发送]」；说明条显示「已放入待办内容」。
  - [pick/startpick-native.png](pick/startpick-native.png)：点「开始」后在该待办下方出现 Agent 选择。
- 自动测试：
  - `prompt` 拼装、空字段不留标题、启动命令不带 `--no-daemon`；
  - 菜单识别：目录信任、Codex 允许菜单都会挡住放入，Agent 输入框和 Agent 回答里的编号列表不会。
- 全量 core 20、daemon 31 + 集成 27、client 25，共 103 项通过；fmt 与 clippy `-D warnings`（含客户端）通过。

## 未验证

- 用官方 Codex 0.163 / Claude Code 2.1.302 在用户工作台里真实走一遍「开始」。替身只证明了时序和不自动发送。
- 真实 CLI 的目录信任与登录画面是否都被「带光标编号菜单」规则挡住，按二进制里的字符串推断，未实际截图。
