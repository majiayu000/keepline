# R4 真实 CLI 验收（2026-10-21）

决定：以真实 CLI 画面、日志、后台对象三方核对，CLI 成功不等于工作台关联成功。不伪造 Hook 或按目录猜身份；不能精确关联即记录不通过。所有输入、审批、继续入口、截图通过 Computer Use，AppKit 只激活测试客户端。

隔离范围见 [runtime](native-runtime.json)，用户 PID 80936 与后台未操作。客户端/后台为 R2 release；Codex 0.163.0，Claude Code 2.1.302。测试目录在 /private/tmp，按既定去噪规则进入“已隐藏”列表，未放宽产品规则。

| 项目 | Codex | Claude Code（用户指定 Zhipu GLM provider） |
|---|---|---|
| A02 新对话托管关联 | 通过：原终端 codex → CLI ID 01a15c6f-7868-7d53-91d4-4f896eb039a0 | **不通过**：内容进入外部会话，terminal=null，原画面仍是 Shell |
| A04 真选择/审批并作答 | 通过：Plan 模式 request_user_input，选 Blue，CLI 回复 Blue，状态栏先等你回答再回合结束 | CLI 原生审批通过：Write acceptance.txt 单次 Yes，文件为 ok；**工作台关联/等待投影未通过**，不能把整项标绿 |
| A12 继续与真实续接 | 通过：Ctrl-C 两次退出→卡片继续→新画面预填同 ID，手动回车；问此前颜色，回复 Blue；后台原 AgentSession 改挂新终端，没有重复会话 | 真实续接通过（限定 provider 注入）：外部卡片继续预填原 ID；为遵守隔离，把可执行名换成只注入授权 provider env 的 launcher，未改 ID；续接问之前写的内容，回复 ok。托管重关联仍不通过 |
| A16 四字段/原话 | 原话与追加原话正确；无工具证据，“做成了”保持未知；停在标明 Agent 自述 Blue；下一步标明推断 | 原话正确；“做成了”由 Write 成功 tool_result 支持且文件实际为 ok；停在保留 Agent 自述，下一步标为推断 |

截图：[Codex 问题](codex-question.png)、[作答](codex-answer.png)、[四字段](codex-card.png)、[退出](codex-exit.png)、[预填](codex-resume-prefill.png)、[续接答案](codex-resumed-answer.png)；[Claude 审批](claude-glm-approval.png)、[Write 成功](claude-glm-write.png)、[四字段](claude-glm-card.png)、[预填](claude-resume-prefill.png)、[续接答案](claude-resumed-answer.png)。后台读回：[Codex](codex-readback.json)、[Claude](claude-glm-readback.json)。两种 CLI 均原生 Ctrl-C 返回 shell。

## Claude 身份缺口

真实 Claude 进程是托管 shell 35702 的子进程 61978，ps 识别命令为 claude。审批暂停时仅对该 PID 的 lsof -p 检查没有 JSONL 打开项；后台外部索引已有正确 ID，但无可证明的 PID↔日志关系。因此真实环境无法复现模拟夹具“持续打开日志”的关联条件。没有增加周期探测、放宽目录匹配或把外部会话强行挂回画面。下一步需另行验证稳定身份信号；当前 A02 不放行。

另观察到 Codex 已带 --no-daemon、首条日志尚未出现时仍短暂提示“改用 codex --no-daemon 才能跟踪”；关联后恢复正常，提示文案待修。

## 认证及额度边界

Claude 官方凭据尝试两次均返回 OAuth token revoked；按用户要求停止，并记录完整 [凭据操作与刷新不确定性](claude-credential-audit.md)。随后用户另行授权 cc-switch 指定 provider：仅只读 providers 表 id fe4d110b-81b5-43d1-80db-b42ccaeafde5、app_type=claude 的 settings_config.env，运行时注入隔离进程；不切当前 provider、不写数据库、不写 ~/.claude，不记录 token。CLI 显示 GLM-5.3-Flash（模型名由该 provider 提供）。未再读取官方凭据或钥匙串。

最小用户回合：Codex 两条（选择题与续接）；GLM 两条（Write 与续接）；另两次官方 Claude 请求认证失败。CLI 自动标题/内部 continuation 不被算作人工提示数量，也不声称零额外 token。此证据不等于 Anthropic 官方模型服务验收通过。
