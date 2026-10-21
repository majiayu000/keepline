# M6 完整验收台账（2026-10-21）

**结论：尚不满足“首版全部通过”的放行条件。Claude 精确关联与前台 CPU 预算不通过，通知/输入法全清单仍有未验证项。** 不推送、不合并、不发布。2026-10-21 10:15 已授权少量真实回合、原生补验和正式签名；通知弹窗由用户确认，公证仅使用现成凭据。历史锁屏记录保留，R1–R7 新证据逐项追加；R6 用户允许通知后再次锁屏，原生点击尚未完成，R7 公证缺可用 profile。任何模拟、历史人工证据都在下表注明来源。

历史 M6 代码：大段输入按 16 KiB 有序分包，保留后台单包 64 KiB 边界，不重试未知结果；Finder 入口在创建线程前启用事件驱动闲置策略；增加本地 ad-hoc 打包脚本。当时未修改或提交并行终端渲染文件；R2 已正式接手并在 a5fd977 / 681dc15 中独立提交。

## 当前补验与历史构建

R1 默认并行 workspace 全套连续三次各 75 项通过；R2 增补后 82 项通过（另 1 个 ignored daemon fixture 由集成测试启动）。R3/R4/R5 为受限真实 CLI/原生验收；R7 Python 语法、真实签名与 codesign 检查通过，spctl 未放行。未把后续文档提交误称为重新跑过 Rust 全套。

### 历史 M6 可复现范围

从 M5 提交 `f31295f` 导出干净源码，加本轮 main.rs、connection.rs、protocol.rs 改动及新增测试；依赖沿用已固定 revision 的 .deps。**没有把并行 screen.rs、Cargo.toml、examples、next/eval 的未提交改动混入最终包。** [环境](environment.json)、[测试](tests.txt)、[包校验](build.txt)、[包摘要](package-receipt.json)。工作树早先全套测试与此干净快照分别记录，最终结论取干净快照。

- 干净快照全套：client 12、core 17、daemon 24、integration 18 通过；1 个 ignored 是被集成测试主动启动的 daemon fixture，不是跳过产品验收。
- 新增 OSC/乱码测试另跑 1 项通过，合计 72 项；[补充日志](osc-test.txt)。
- 大段 UTF-8/多行输入经真实 PTY 写到文件，逐字节一致；主动丢弃 ACK 后重连，没有重放。最初未分包版本确实失败，修复后通过。此项是协议/PTY 自动测试，**不是系统输入法或剪贴板手测**。
- 宿主被杀：真实 cat 退出，重启隔离宿主后旧终端 Interrupted，不伪造 Running；进程 ID 不复用作恢复依据。
- 签名包：自动启动自身后台、第二客户端退出码 0、退出/重开客户端保持原终端 ID/PID，自动可写并接收新输出。[实际进程/RPC 回执](package-check.json)。这不是 Finder 双击或 Dock 点击证据。

## A01–A19 逐项结论

“通过”只覆盖所列证据范围；官方 Codex/Claude 的共同放行门槛仍未满足。R4 已揭示 Claude 托管关联缺陷；R5 已揭示前台 CPU 超预算。历史模拟与本轮真实证据分别标明。

| ID | 结论 | 证据、已验证范围与缺口 |
|---|---|---|
| A01 冷启动、大量历史 | **部分通过** | R5 Finder 双击隔离 App 自动启动后台并展示 40 会话；M0 112.42 MiB/3.207 秒、暖启动 0 B。未将完整大量历史+可见首屏合为一次测试。见 [R5](../r5-20261021/README.md) |
| A02 两项目双 CLI | **不通过** | R4 Codex 新对话正确挂载；Claude Code + 授权 GLM 真实回合成功但日志短暂打开，未能按 PID 绑定，仍是外部会话。见 [R4](../r4-20261021/README.md) |
| A03 IME、多行、大段粘贴 | **部分通过** | 大段 UTF-8/多行 PTY 字节测试通过；R5 四画面真拼音、选词/回车/退格与聚焦通过。cat 未启用 bracketed paste 时多行粘贴被拒绝，完整原生清单未通过。见 [R5](../r5-20261021/README.md) |
| A04 真实审批/选择 | **部分通过** | R4 Codex request_user_input 原生选择 Blue、状态栏等你回答通过；Claude + GLM Write 原生单次审批及文件写入通过，但工作台托管/等待识别未通过。见 [R4](../r4-20261021/README.md) |
| A05 切换、4 路输出、备用屏 | **通过**（分段原生证据） | R2 Vim/lazygit/htop 鼠标、备用屏滚轮、焦点；R5 四路 10 Hz 真实 PTY 输出，切标签返回、放大/还原及焦点输入。见 [R5](../r5-20261021/README.md) |
| A06 关闭标签/窗口/App 再打开 | **部分通过**（缺 Dock 点击） | [原生历史](../keyboard-lifecycle-20261020/README.md)与本轮包 RPC 回执：原 ID/PID 保持、可继续输入。直接点 Dock 独立路径仍未验证 |
| A07 断线、快照、丢确认 | **通过**（自动 + 历史原生） | reconnect_input_resize、分段 UTF-8/escape 快照、拒绝截断快照、大输入不重放；历史真实客户端重开有截图。曾有一次 CUA 首字符遗漏未定位，见 IME 原记录，不能宣称已修复 |
| A08 待办开始/重复开始 | **通过**（原生 UI + 模拟 CLI） | [M1](../m1-20261021/README.md)：原生表单、无回车预填、重复开始保持终端数、模拟 CLI 按回车后自动归属；官方真实模型回合未验证 |
| A09 系统通知点击 | **未验证**（原生链路） | R6 真实请求返回拒绝，系统设置已登记测试应用；用户后续确认已开启。随后屏幕重新锁定，真实展示/点击仍待补。去重单测不代替原生证据。见 [R6](../r6-20261021/README.md) |
| A10 退出、停止、宿主死亡 | **通过**（真实 PTY 自动测试） | process_exit、stop_agent、host_death；正常 exit_code、结局不变、未识别 shell 拒绝 StopAgent、宿主死亡后 Interrupted |
| A11 单写入者/接管 | **通过**（真实 PTY 自动测试） | 两连接拒绝无权限输入/resize；明确接管；owner 断开自动归还；签名包重开无其他 owner 自动可写 |
| A12 真实 CLI resume | **部分通过** | R4 Codex 继续预填、手动执行、原 ID 续接与重新关联通过；Claude + GLM 外部卡片预填后仅替换 provider 注入 launcher，同 ID 恢复并答 ok，托管关联仍失败。见 [R4](../r4-20261021/README.md) |
| A13 乱码、恶意控制/链接 | **通过**（当前支持边界） | 新增 OSC52 写/读、OSC8 javascript、非法 UTF-8/异常 SGR 后恢复及快照测试；客户端没有自动 URL、剪贴板 OSC、脚本执行回调。R2 新增 OSC 8，仅 http/https/file 且用户 Cmd 点击才调用系统打开；危险协议单测拒绝、真实 Finder 跳转/悬停通过，见 [R2](../r2-20261021/README.md) |
| A14 性能与长期驻留 | **不通过** | R5 可见 1/4 画面累计 CPU 为 Ghostty 2.39×/2.72×，超过 1.2×；footprint 1.03×/0.89×。未计系统 login 包装及共享服务，细节见 [R5](../r5-20261021/README.md)。R1 三轮全套、40 空闲/4 写测试及下述两小时锁屏驻留通过，不能抵消前台超预算 |
| A15 分组、结局、到期 | **通过**（确定时间/日志集成 + 原生 UI） | [M2](../m2-20261021/README.md)：完成/暂存/放弃/撤销、真实截止等待、到期与新回合复开；结局不停止 PTY |
| A16 四字段/机械证据 | **通过**（日志比对 + 原生 UI） | [内容层](../content-layer-20261021/README.md)、[M0](../m0-20261021/README.md)、[M5](../m5-20261021/README.md)：20 个卡片/原话抽查、5 Claude 来源复核；自述不进“做成了”；只读已缓存内容，无输出逐帧解析 |
| A17 离开/40 会话/导航角标 | **通过**（M3 + R5 原生） | M3 离开基线/新标记/搜索；R5 40 合成会话跨 4 空间逐一筛选：各 10、等你 2/错 1、需要你总数 12；空间4继续归属正确且不自动执行。见 [R5](../r5-20261021/README.md) |
| A18 四画面系统 IME 清单 | **部分通过** | R5 系统拼音四画面真实输入/聚焦/退格/回车，逐字节落盘一致、Ctrl-C 四 cat 退出。候选浮窗不在 CUA 截图里；中英切换及每画面完整组合未逐一验证。历史用户豆包候选窗通过不扩大为此次全清单。见 [R5](../r5-20261021/README.md) |
| A19 完整对象关系操作链 | **通过**（模型/SQLite + 原生分段证据） | M1 重复开始/再开/归属、M3 继续、分割树搬移/关标签保留进程与布局测试；普通 shell 无 AgentSession，不自动执行预填。官方 CLI 真实会话门槛另计 |

## 资源与长期驻留

`soak.py` 使用最终干净 release，40 外部 + 8 托管模拟会话；产品空闲不轮询。测试驱动每分钟用 `proc_pid_rusage` 记录客户端、后台和全部 8 个 PTY；CPU 按本机 Mach timebase 换算，footprint 非 RSS。在采样首尾各读一次 RecognitionStatus，不持续 RPC 刷新。所有日志、HOME、runtime 隔离，不启动模型。macOS 已锁屏，因此这属于**后台驻留趋势**，不等于 8 个可见画面资源验收，也不包括 WindowServer 等共享系统进程归因。

两小时驻留已完成：**7200.009 秒、121 个样本**，最大采样间隔 60.009 秒。完整测试进程树 footprint 从 **74.83 MiB 降至 73.57 MiB**，峰值 74.83 MiB；累计 CPU **0.121999 秒**（后台与 8 个 PTY 增量均为 0）。全部覆盖进程磁盘读写增量 0 B；ps/lsof、目录枚举、日志打开/读量、索引写入计数首尾完全一致。未观察到增长趋势。

[汇总](soak-summary.json)、[完整结果](soak-result.json)、[每分钟原始样本](soak-trend.jsonl)。只记录锁屏后台驻留，不扩大为前台/GPU/功耗或 Ghostty 相对预算通过。测试结束只清理了本轮隔离客户端、PTY 与后台。

历史 M6 时 Ghostty 可见对照未执行；R5 已在解锁状态补测，CPU 2.39×/2.72× 不通过，内存 1.03×/0.89×，详见上表与 R5 原始值。已有八画面短测仅作历史参考：[分屏资源](../split-tree-20261020/README.md)。

## 历史 M6 本地 App

产物：`next/target/bundle/Keepline.app`（target 已忽略，不将二进制提交）。构建脚本 `next/package-macos.py --release-dir next/target/acceptance-clean/release`；包内客户端与 keeplined 相邻，Symbols 字体嵌入二进制并携带许可；codesign --verify --deep --strict 通过，链接依赖均为系统 framework/library。

该历史产物没有安装到 /Applications，当时没有正式签名、公证、发布或推送。R7 另行签名的独立产物见下节。默认启动仍会连接用户原有后台；如果版本过旧，应显示明确警告而不重启它。真实 major 1 提示已有 [原生证据](../keyboard-lifecycle-20261020/old-backend-final.png)。用户自己处理旧后台切换后再进行实际使用。

## 当前剩余缺口

- Claude + GLM 真实回合可运行，但精确托管关联失败；A02 不通过，A04/A12 部分通过。官方 Claude OAuth 失败停止，凭据审计在 R4。
- 可见 1/4 画面 CPU 超过 Ghostty 1.2× 预算；A14 不通过。
- A18 仍缺候选浮窗与每画面完整组合，中英切换未逐一验证；Dock 点击未验证，大量历史+Finder 首屏未合为一次测试。
- 系统通知用户已确认开启，后续系统再次锁屏；A09 点击/展示仍待解锁补测。
- R7 Developer ID + hardened runtime 签名验证通过；spctl 拒绝未公证包。没有发现可用既有 notary profile，未提交公证或发布。见 [R7](../r7-20261021/README.md)。

此前真实截图：[鸟瞰](../m3-20261021/return-overview.png)、[工作视图预填](../m3-20261021/resume-prefill.png)、[卡片弹窗](../m4-20261021/menubar-route.png)、[嵌套四分屏](../split-tree-20261020/nested-native.png)。这些是原记录，**不是锁屏后重新截图**。

## R3 关联复查更新

[三条路径实测](../r3-20261021/README.md)：两条最小用户提示 + 两次 CLI 自动标题完成。默认 daemon 与 embedded 均返回 ok，但此测试是独立 PTY，不替代 A02/R4 的工作台托管验收。结论继续保留 --no-daemon；远端覆盖回调/首轮审批前关联未证明。

## R4 真实回合更新

[真实 CLI 验收](../r4-20261021/README.md) 已完成 Codex 与授权 GLM 上的 Claude Code 最小回合，不能再用“未获额度授权”解释缺口。官方 Claude 认证失败后已停；[凭据操作事实](../r4-20261021/claude-credential-audit.md) 明确无法确认内部刷新请求。Claude 精确关联仍不通过。

## R7 当前签名产物

`next/target/signed-acceptance/Keepline.app` 使用 Ma JiaYu (C5UWZ934C2)，不覆盖用户运行的 `target/bundle`。codesign --verify --deep --strict 通过，hardened runtime/时间戳存在，无运行时例外 entitlement；spctl 为 Unnotarized Developer ID。签名、公证、分发分别记录，见 [R7](../r7-20261021/README.md)。
