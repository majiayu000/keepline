# R3 默认守护模式关联复查

日期：2026-10-21。本机官方 codex-cli 0.163.0。决定：**保留 --no-daemon 兜底，不替换默认关联**。目标是完整、唯一、可在审批前使用的 PTY↔thread 身份，而不是按 cwd/标题/最近活动猜测。放弃条件：标识截断、回调跨线程、依赖未证明的远端覆盖或只能在回合完成后关联。

只创建 `/tmp/kl-r3-26rginoh` 私有 CODEX_HOME，复制本机现有授权到该 0700 目录（auth 0600），最小配置沿用 gpt-6.1-sol/high；未改用户配置、未连接用户 app-server。自建 daemon PID 28055，官方 TUI 通过独立 PTY 驱动；这不是 Computer Use 验收。两条提示各返回 ok，CLI 另自动生成两次标题；**保守计为 4 次模型完成，未追加第 3 条用户提示**。

## 三条路径

| 路径 | 实测 | 决定 |
|---|---|---|
| (a) `-c notify=[python, script, --keepline-pane, token]` | CLI 明确警告 `Running without the shared background server`，原因 command-line overrides requires embedded mode；真实完成回调同时含 token + 完整 thread-id | 不能把隐式 embedded 冒充解决共享 daemon 关联。且同 token 被自动标题线程继承，见 probe.json 的两个不同 thread_id；必须额外验证持久化会话，不可收到 token 就绑定 |
| (a) 显式 `--remote unix://` 加相同覆盖 | 能连隔离共享 daemon，无 embedded 警告，TUI 产生新 thread 的截断标题；为守预算未发第三条模型提示 | 该远端路径的 notify 执行/跨线程隔离/恢复未验证，不能升级默认；即便 notify 生效，完成通知仍晚于首轮审批 |
| (b) rollout session_meta / turn_context | 默认 daemon 与 embedded 各有真实 JSONL；检查字段与字节，均无 notify token、无通用 CLI override/env 容器；完整键名见 probe.json | 不能从这些记录取回启动 token。没有声称对任意未来字段/任意环境变量做过穷举 |
| (c) OSC 标题/其他输出 | 默认标题只有项目名与活动转圈，OSC 7501 只有 app/state；`tui.terminal_title=["thread-id"]` 确有 UUID 前缀，但实际输出为 `01a15c6a-59fa-76a0-b0f5-f1607...`，不是完整 UUID | 不按 UUID 前缀猜测身份；不能用于可靠默认关联 |

第二条提示的 PTY 驱动在退出警告面板时把 bracketed-paste 的 `[200~` 前缀带入了提示；实际原话与该限制保留在日志中，仍仅请求回复 ok，没有工具动作。不能将此作为输入法通过证据。

## 证据与源码边界

[probe.json](probe.json) 只保存线程 ID、字段名、来源和最小回复，不含授权/用户历史。三个 raw*.vt.txt 是本轮输出去除控制序列后的文本；没有收录整个用户 rollout。

本机源码 `/Users/lifcc/Desktop/code/AI/tools/codex` HEAD `8a94430bb2`：tui/src/lib.rs 的 can_reuse_implicit_local_daemon 在存在 cli_kv_overrides 时返回 false；bottom_pane/title_setup.rs 有 SessionId 项；rollout/src/recorder.rs 明列 SessionMeta。该 checkout 不是已安装 0.163.0 的可证明构建源，**源码只用于提出假设，上表结论以安装版实测为准**。未改 Codex 源码或用户 CLI 配置。

后续候选是上游提供未截断的专用 OSC 身份事件，或 SessionStart 等能保留每个 TUI 启动 token 的精确线程事件；当前不引入代理或轮询式目录匹配。
