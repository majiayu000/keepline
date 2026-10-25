# 真实待办启动与状态复验 · 2026-10-22

决定：修复已复现的启动交接错误，保留真实整条链路待验。通知不要求用户点击；A14 依用户决定后置。**本轮没有发送模型请求，也没有发生付费回合。**

## 指标与边界

官方 Codex 0.163.0 / Claude Code 2.1.302：「开始」只启动 CLI；登录、主题、说明、目录信任、审批页面不接收待办内容；输入框就绪后只填一次且不发送；真实回合正确关联待办。Claude 审批等你回答与简短完成回复分组分别验证。机械证据只认工具结果。

独立 HOME / CODEX_HOME / CLAUDE_CONFIG_DIR / runtime。`fixture.py` 仅在隔离 Claude exec 时只读 cc-switch 指定 GLM provider 并注入环境；不读官方 Claude 凭据/钥匙串，不改用户配置或 provider，不把密钥落盘。Codex 只复制已授权 auth.json 到隔离 HOME，隔离配置选低推理成本；测试清理时移除副本，不改源文件。原生动作全部 Computer Use，AppKit 只激活自己的客户端。

## 真实发现与修复

原逻辑把 bracketed-paste 已启用、画面静止且没有「选中第 1 个编号选项」当就绪。但 Claude 首次主题页、安全说明页和新版无编号目录信任页均会启用 bracketed-paste；任务被送到菜单后消失，状态条却称已放入。

`pane.rs` 现在要求实际输入提示符；取最后一个提示符，避免旧菜单残留阻塞当前输入；菜单下方 Enter to confirm/select/continue 提示、任意编号选项及主题选择均阻止放入。不新增配置或后台轮询，沿用原 60 秒启动超时及手动复制兜底。

| 项目 | 结论及证据类型 | 截图 / 证据 |
|---|---|---|
| Codex 未登录不填任务 | **通过，真实官方 CLI + CUA**；初始构建已经正确拦住选项 1，本轮增加选项 2 的回归 | [真实登录页面](codex-before.png) |
| Claude 主题页误填 | **真实复现，修复后原生通过** | [修前](claude-onboarding-misplaced-before.png)、[修后](claude-theme-blocked.png) |
| Claude 安全说明页误填 | **真实复现，修复后原生通过**；等待静止后仍显示启动中，无任务文本 | [修前](claude-security-notes-before.png)、[修后](claude-security-notes-blocked.png) |
| Claude 无编号目录信任页误填 | **真实复现；最终修复仅单测通过，原生待复验** | [复现](claude-unnumbered-trust-before.png)、`bracketed_paste_pages_without_an_input_prompt_are_not_ready` |
| 任意编号菜单、长主题页、旧菜单后新输入框、启动 splash / shell | **仅单测通过**；Codex logo 的 >_ 不被当输入提示符 | `handoff_tests` 四项 |
| 真正输入框只填一次且不发送、首轮正确关联待办 | **未验证最终构建**：16:50 左右系统锁屏；CGSSessionScreenIsLocked=Yes，前台 loginwindow，CUA cgWindowNotFound | 隔离实例见 [runtimes](runtimes.json) |
| 真实 Claude 审批与“完成。”归待核对 | **本轮未验证**：未发送付费请求；现有完成自述单测与模拟审批集成测试单列，不冒充真实回合 | 见测试记录 |

本轮 Claude 截图在主题页之后都是单色：复用的 r9 启动器丢掉了 `TERM` / `COLORTERM`，不是 Keepline 渲染问题，也不能当颜色证据。启动器已经修复；夹具现在启动前先跑 [r9 color_check](../r9-20261021/color_check.py)，复验时再用 `--live <root>` 确认运行中的进程。

主题、安全说明截图来自修复过程中的构建；无编号信任菜单是在其后发现并修复。最终构建没有完整原生通过证据，不把前两项扩展成整条通过。

## 自动检查

Rust 1.95.0 / Zig 0.16.0 / `LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast`，与 CI 固定工具链一致。

- `cargo fmt --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过（修正本次 nonminimal_bool，无 allow）。
- `cargo test -p keepline-app`：29 项通过，含 4 项交接回归。
- `cargo test -p keeplined claude_short_completion_is_a_review_claim_not_evidence`：1 项通过。
- `cargo test -p keeplined --test managed_session quiet_agent_screen_with_an_approval_waits_on_the_user_until_they_answer`：1 项通过，属于模拟屏幕集成验证。
- `cargo build --release -p keepline-app`：通过。未替换用户正在运行的 bundle。

[输出摘录](checks.txt)，原主题页回归的 [RED](regression-red.txt) 与早期 [GREEN](handoff-green.txt) 均保留；以最终 checks 为准。未重跑整个 workspace 测试，后台未改动，完整门禁交由 CI。

只停止本轮隔离进程、删除本轮隔离 Codex auth 副本，未操作用户后台/终端。见 [清理记录](cleanup.json)。

## 剩余工作

解锁后，使用新隔离实例复走无编号信任页→输入框，确认填入一次且尚未发送，再各用 Codex/Claude 一次最小文件写入回合核对关联、审批等待、机械证据及完成分组。真实通知、Dock/输入法剩余清单仍按 acceptance-final 原结论，不扩大本轮证据。
