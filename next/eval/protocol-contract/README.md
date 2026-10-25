# CLI 状态接口复验

Keepline 的回合状态规则（[tech.md 回合状态判定](../../../specs/terminal-workbench/tech.md)）读取四个不由我们维护的接口。Codex 的 app-server 协议标为实验性，`claude agents --json` 还是研究预览。升级 Codex 或 Claude Code 后先跑一次：

```bash
python3 -I check.py --live
```

不带 `--live` 只检查 schema 和命令输出的结构；带上后，会对正在运行的 Codex 后台做一次只读的 `thread/read`。两种方式都不订阅线程，也不修改任何配置。

| 检查 | 保护的规则 | 失败时 Keepline 的表现 |
|---|---|---|
| `ThreadStatus` 的 `idle` / `active`，以及 `activeFlags` 里的 `waitingOnApproval` / `waitingOnUserInput` | S06 只读对账 | 对账结果被跳过，界面上提示「Codex 官方状态查询不可用」 |
| `thread/read` 的参数，未加载的线程返回错误 | S06 | 同上 |
| 子 Agent 的会话记录头带 `parent_thread_id` | Codex 子 Agent 日志跟踪 | 父会话不再统计子 Agent，回合结束后可能过早进入「待核对」 |
| `claude agents --json` 的 `sessionId`、`status`、`state`、`waitingFor` | S03 | 对账结果被跳过，界面上提示「Claude 官方状态查询不可用」 |

FAIL 表示 Keepline 依赖的字段变了，需要先改代码再升级；WARN 表示出现了新取值，Keepline 会忽略它而不是去猜，需要判断要不要支持。

## Hook 载荷样本

`crates/keeplined/tests/fixtures/hook_shapes.jsonl` 是从旧版 Keepline 记下的真实 Hook 事件中提取的结构样本，只保留事件名、字段名、后台任务的类型和状态、定时唤醒的数量、通知类型，以及是否带 `agent_id`，不含 ID、路径或正文。单元测试 `recorded_hook_payload_shapes_parse_as_their_task_lists_say` 会逐条解析。重新生成：

```bash
python3 -I hook_shapes.py ~/.keepline/spool/events > ../../crates/keeplined/tests/fixtures/hook_shapes.jsonl
```

2026-10-23 生成：74515 条事件，共 94 种结构。后台任务的 `status` 只出现过 `running`；Codex 0.163 的 Stop 不带 `background_tasks` 和 `session_crons`。

## 已验证版本

| 日期 | Codex | Claude Code | 结果 |
|---|---|---|---|
| 2026-10-23 | 0.163.0 | 2.1.302 | `check.py --live` 0 failed, 0 warnings |
