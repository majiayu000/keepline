# M5 运行时关联

决定：继续使用托管 PTY 子进程 + 打开日志的唯一身份证据。Claude 不安装 Hook；Codex 保留 `--no-daemon`。没有可靠证据时保持未关联，不按目录或最近活动猜测。

## Claude

- 通过：两个同 cwd 的模拟 Claude 进程，经真实 PTY、进程树和 `lsof -p` 各自绑定独立 JSONL，无 Hook；追加 Edit 结果和 AskUserQuestion 后只推进游标，ps/lsof 计数不增加；原话、机械证据、问题选项不串线。[集成测试](test.txt)
- 通过：M0 的真实 Claude 投影选 5 个会话，按 inode/offset 重新只读原始日志；首条原话来自 user 记录、机械证据来自 tool_result，抽取位置一致。未重读全文。[原始记录校验](claude-source-audit.json)
- 既有单测覆盖 Edit、审批等待、Stop、新回合；同终端 Codex→Claude、退出与持久化由 managed_session 集成测试覆盖。
- **未验证，等待用户授权**：官方 Claude 2.1.302 真实对话、权限请求和完整工具回合。模拟进程持有日志 FD，不能证明真实 CLI 短暂打开文件的所有时序都能捕获；没捕获仍不猜归属。真实日志是内容解析证据，不是托管关联证据。

## Codex 默认 daemon 模式研究

本机 `codex-cli 0.163.0`。执行只读 `--help` 和本机 `codex app-server generate-ts --experimental --out /tmp/kl-codex-protocol-0163`，未连接或重启用户 daemon，未发起模型请求。保留本机生成的相关协议类型供复核：

- [Thread](codex-Thread.ts.txt)：有 thread ID、path、cwd、originator、source/threadSource，没有 TUI PID 或本地客户端连接 ID。originator 明确表示创建时来源，独立于当前 client/executor。
- [ClientInfo](codex-ClientInfo.ts.txt)：只有 name/title/version，不携带 OS PID。
- [ThreadListParams](codex-ThreadListParams.ts.txt)：可按 cwd/source 筛选，不能唯一对应多个相同项目的 TUI；查询出来的 thread 不等于当前画面。
- [ThreadBackgroundTerminal](codex-ThreadBackgroundTerminal.ts.txt) 的 osPid 属于线程启动的后台工具终端，不是 TUI 客户端；不能反向冒充关联证据。

因此本轮**否定用这些协议字段替换现有关联**。这不是声称 Codex 内部绝不存在映射，而是已安装版本公开生成接口不足以证明唯一映射。共享 app-server 的日志/Hook 祖先不在 PTY 下，已有真实观察见 [前次验收](../real-codex-acceptance-20261020.md)。未引入代理、拦截或修改用户配置；可靠上游 PID↔thread 证据出现后再替换。
