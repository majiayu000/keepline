# 等你回答屏幕兜底与一行新建待办（2026-10-21）

依据 [调研结论](../../research/waiting-and-capture-20261021.md) 与 product.md 6.3 / 6.4「新建待办」。

- 要支撑的决定：无 Hook 时，Claude / Codex 的审批能否进入「等你回答」；新建待办能否一行完成。
- 放弃条件：屏幕兜底一旦误标（普通 shell、已答过的提示、用户自己打开的菜单被标成等你回答），收紧规则或关闭兜底。
- 结论：**采用**。自动测试与隔离原生渲染通过；真实 Claude 审批的原生复验未做（见「未验证」）。

## 实现

后台 `keeplined`：
- `recognition::screen_wait`：只认两种严格形状。审批是 `Do you want to …?`（Claude Code）或 `Would you like to …?`（Codex），后面紧跟以 `1. Yes` 开头的编号选项，且选项仍在屏幕底部；选择题是带光标的编号选项，下方页脚同时含 `Enter to select` 和 `Esc to cancel`。字符串取自本机 Claude Code 2.1.302 与 Codex 0.163 的二进制。
- `pump`：每段输出后静止 1 秒，检查一次屏幕底部；没有输出就没有定时器。只检查已关联 Agent 的画面。用户 3 秒内刚输入过时推迟到 3 秒后再判断，避免把用户自己打开的菜单当成 Agent 提问。
- 用户在该画面输入后立即撤销标记；之后静止时重新判断。
- 结果只存在后台内存（`screen_waits`），不写数据库。快照里把它叠加到该会话的内容上：分组为「等你回答」，等待文字带「（画面上的提示）」标明来源。日志已是等待时不覆盖。

客户端：
- 通知：正在看的画面（窗口在前台且画面聚焦）不发；标题前加空间名。
- 「＋ 新待办」改为左栏待办区一行输入：回车保存、Esc 取消，按钮「保存并开始 / 保存 / 展开 / 取消」。空间取当前空间，Agent 取默认设置。「展开」把已写的标题带入完整表单。
- 待办没有验收标准时，预填说明行显示「还没写验收标准」和「补写」。

## 证据

自动测试：
- `screen_wait_reads_only_strict_approval_and_choice_shapes`：Claude 审批框、Codex 审批、Claude 选择题识别正确。以下 7 种都不识别：只是提到这些字样的正文、Agent 回答里的编号列表、没有光标的菜单、离底部太远的页脚、已作答并被后续输出顶上去的审批、普通 shell 输出等。
- `quiet_agent_screen_with_an_approval_waits_on_the_user_until_they_answer`：真实 PTY 加真实后台。Claude 显式 ID 会话的日志里没有审批记录：
  - 用户输入后出现的提示在 1.5 秒内不标记，3 秒后标为「等你回答」；
  - 用户作答后立即撤销；
  - 屏幕清掉后静止重查，仍保持清除；
  - 全程 lsof 0 次。
- `notifications_only_follow_new_attention_facts_and_opt_in`：正在看的画面不发通知，其他画面照发。
- 全量：core 20、daemon 31 + 集成 27、client 24，共 102 项通过；workspace fmt 与 clippy `-D warnings` 通过。

隔离原生渲染（[fixture.py](fixture.py)，基于 R14 的 40 会话夹具）：
- 第二个画面的日志以出错结束，绑定后才在屏幕上出现 Codex 审批。
- 真实后台在约 1 秒内把它改判为「等你回答 · Would you like to run the following command?（画面上的提示）」，fixture 输出的就是这条判断。
- GPUI `render_to_image` 截图，1440×900：
  - [native/screenwait-native.png](native/screenwait-native.png)：状态栏、标签角标、空间角标、「你在这里」卡；
  - [native/quicktask-native.png](native/quicktask-native.png)：左栏一行新建待办；
  - [native/overview-native.png](native/overview-native.png)、[native/menubar-native.png](native/menubar-native.png)。

## 未验证

- 真实 Claude Code / Codex 审批出现在用户自己的工作台里的原生复验。本机辅助功能未授权给当前会话，没有做点击操作。
- 已知盲区：
  - 提示在会话绑定之前就已出现并且之后再无输出的画面，不会补查。现实中审批出现在绑定之后，暂不处理。
  - Codex 的选择题页脚样式未取到，目前只识别 Codex 的审批，不识别 Codex 的选择题。
