# R9 Claude 确定性关联与真实 GLM 复验（2026-10-21）

结论：采用 `claude --session-id <UUID>` 新建、`claude --resume <ID>` 续接。Claude A02 / A12 真实托管链路通过；A04 原生单次审批作答通过，但“等你回答”识别仍不通过，不把无 Hook 的日志缺口伪装成已解决。共两条真实用户提示，未追加第三轮。

## 实现与边界

- 所有 Keepline 新建 Claude 预填命令使用 UUID v4；续接保持原 ID。后台只枚举托管 PTY 子树，用 macOS KERN_PROCARGS2 保留 argv 参数边界，仅提取明确的 `--session-id` / `--resume` / `-r` UUID。内核返回缓冲区含环境尾部，但解析止于 argc；不读取、保存或输出环境值。
- 同 runtime 的日志 session ID 必须精确相等且唯一；不同 ID、同目录或歧义都不绑定。显式 ID 不依赖日志 FD 保持打开，仍由进程退出监听清除 current。
- 无显式 ID 保留托管 PID 的 lsof 证据路径。新日志身份/托管根清单变化触发一次探测，未关联日志反复追加不会反复运行 ps/lsof；确认不到保持外部。没有按目录猜测、全局 lsof 或轮询。
- 额外修复原生验收发现的多行预填竞态：`?2004h` 可能到达于合并唤醒和绘制之间。绘制清除 dirty 后检查待预填命令，新到的输出会再次唤醒；排队时也立即检查已就绪的静默 shell。仍等待 bracketed paste，绝不自动回车。快照模式单测证伪了“快照丢失 2004”的初始假设。

## 隔离与凭据

测试 runtime 位于 `/Users/lifcc/Library/Caches/KeeplineAcceptance/r9-s9ff7qt3/runtime`，独立 HOME `/tmp/kl9-home-vmxybhu2` / CLAUDE_CONFIG_DIR。本轮没有读取、复制 Claude 官方凭据文件或钥匙串，没有官方 OAuth 刷新尝试，没有修改用户 `~/.claude`、cc-switch 当前 provider 或数据库。

[fixture.py](fixture.py) 启动器仅在 exec 前以 SQLite `mode=ro` 读取用户指定 provider 行的 env，注入私有 Claude 进程。密钥不打印、不落配置、不进证据。证据只包含用户测试原话、机械结果、会话标识及计数；没有复制原始日志或环境。两轮使用 Claude Code 2.1.302、GLM-5.3-Flash，MCP 空配置、Hook 关闭。第一轮 CLI 新默认 auto 模式，因此不把它算作审批验收；第二轮明确 `--permission-mode default`。

第二轮 CLI 显示自身 updater 提示；用户 CLI symlink 检查仍指向 2.1.302。复现脚本已显式禁用 updater，避免后续测试更新软件；不声称本次自动更新路径经过完整文件审计。R4 的旧 OAuth 操作事实仍见原审计，本轮没有重试官方登录。

## 真实 Computer Use 验收

以下按键、点击、选项、截图均来自真实隔离 Keepline 窗口；AppKit 仅用于已授权的窗口置前。不是模拟 CLI 或单测替代。

| 项目 | 结果 | 证据 |
|---|---|---|
| 新建 UUID 命令预填，回车后关联待办 / PTY / JSONL | 通过 | [预填](new-session-prefill.png)、[真实首轮](associated-first-turn.png)、[读回](first-turn-readback.json)。CLI ID `f5eb4f20-8648-49bf-9576-c494f327c1b3`，AgentSession `62d38c9d-8605-445c-a76f-f578fd7da145`；lsof 0 |
| 首轮四字段与用户原话 | 通过 | [卡片](first-turn-card.png)：你要它为原话+验收要求，做成了为 Write 工具成功文件修改，停在为最后回复，下一步标明推断；文件字节为 `ok` |
| Ctrl-C 两次退出、继续预填同 ID、手动执行续接 | 通过 | [resume 预填](resume-prefill.png)、[恢复原历史](resumed-same-history.png)。恢复到同待办标签的新画面，不自动执行 |
| 第二轮记得先前字节并写新文件 | 通过 | [真实结果](resumed-write-result.png)、[读回](resumed-readback.json)。不调用 Read，模型从历史回答 `ok`；两个文件均为 `[111,107]`。AgentSession 总数保持 1，关联移到新终端 |
| 真实 Write 单次审批、画面内选择 Yes | 通过（CLI 作答） | [审批窗口](real-approval.png)、[审批时读回](approval-readback.json)：作答前文件不存在，选择后写入成功 |
| 审批时工作台显示“等你回答” | **不通过** | 同一审批读回仍为 `turn=Running`，waiting 为空。JSONL 当时仅提供工具调用，不提供 PermissionRequest；不装 Hook 的当前链路不能区分审批等待与执行中 |
| 第二轮四字段 / 退出 current | 通过 | [卡片](resumed-card.png)、[两条原话/两个机械证据](resumed-readback.json)、[退出读回](exit-readback.json)：current=false，结局不变 |
| 多行预填竞态修复 | 通过（原生） | [新画面立即预填](multiline-prefill-fixed.png)，没有按空回车、没有执行第三轮。原始首轮截图来自修复前，曾需要一次空回车唤醒 |

额外保留的问题：第二轮回复以“完成。”开头，现有完成自述匹配未命中；虽然 turn=Completed 且机械证据齐全，分组仍显示“做一半停了”。四字段内容准确不代表分组语义全部正确。本轮不扩展完成判断规则，A15 增记此真实缺口。

## 自动验证

最终默认并行 `cargo test --manifest-path next/Cargo.toml --workspace`：**89 passed，0 failed**（app 23、core 20、daemon 26、integration 20）；另 1 ignored 为集成测试主动启动的 daemon_fixture。[完整测试执行记录（省略依赖告警详情）](tests.txt)。[release 客户端/后台构建](build.txt)通过。

新增覆盖：UUID 唯一性；argv 边界/无效与冲突 ID；同 cwd 双明确 ID 精确绑定、无 ID 不猜；日志已关闭仍绑定、零 lsof、退出 current 复位；手敲无 ID 且 FD 已关闭保持外部，五次追加不再探测；bracketed paste 快照恢复。后两项真实 FD/原生事件自动测试不是模型调用。首次手敲追加测试失败，因为夹具持有 FD 导致事件未及时派发；改为每次追加后关闭，按真实 CLI 短暂打开情形验证，没有提高预算或删断言。

R6 继续等待用户解锁通知，不启动通知复验；公证由用户处理。不推送，不触碰用户运行的 Keepline 后台与终端。

收尾已停止本轮 R8/R9 私有客户端、后台和测试 PTY，以及专用 Ghostty 对照副本；用户 Keepline PID 80936 保持运行。[清理回执](cleanup.json)。
