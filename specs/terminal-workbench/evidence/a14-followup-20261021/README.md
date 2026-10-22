# A14 连续前台复测 · 2026-10-21

**决定：新的前台性能倍率未验证，A14 保持不通过。** 本轮未改产品源码或提高预算，不把后台样本、未启动负载或构建通过写成性能达标。最后有效 CPU 倍率仍是 R8 的单画面 1.60×、四画面 1.24×；这不是当前构建的新结果。

## 指标与放弃条件

沿用未修改的 [R5 measure.py](../r5-20261021/measure.py)、[R8 单画面负载](../r8-20261021/load-one.py)和 [R5 四画面负载](../r5-20261021/load.py)：10 Hz、3 秒预热、30 秒采样，累计客户端 + 后台 + Python 生产者 CPU，phys_footprint，预算各 ≤ Ghostty 1.2×。Ghostty 的 login 包装层仍按既有口径排除。Menlo 12.5、原有窗口/终端区域差异不变。

每轮只在开始时用已获授权的 AppKit 激活一次。NSWorkspace 激活事件 + 100 ms 前台 PID 记录贯穿采样；任何失焦都作废，最多三次，**有效但超预算的结果不重采**。采样内不调用 UI 工具、不强抢焦点、不关闭其他应用。找不到空闲窗口时有界结束，不要求用户守候。

## 实际结果

| 阶段 | 结果 | 证据 |
|---|---|---|
| 首次单画面采样 | 无效；约 0.478 秒即切到 Chrome，随后微信、Chrome、Telegram、微信 | [原始前台记录](keepline-one-a-focus-1.json)、[未计入验收的资源值](keepline-one-a.json) |
| 自动等待单画面空档 | 180.13 秒内没有连续 20 秒无键鼠活动；未启动生产者，不产生 CPU 样本 | [等待记录](keepline-one-batch-1-idle-gate.json) |
| 自动等待 Ghostty 空档 | 同样 180.13 秒到上限；未采样 | [等待记录](ghostty-one-batch-1-idle-gate.json) |
| 四画面对照 | 两套原生窗口均已准备，因共同的空闲前提持续不满足，取消剩余测量 | [Keepline](keepline-four-ready.png)、[Ghostty](ghostty-four-ready.png)、[停止原因](progress.json) |

首次无效资源值为总 CPU 1.409 秒，但既失焦，后续检查还发现测试 workspace 多出一条待办和终端，因此不能用于比较，也不能拿它宣称改善。前台记录只证明哪些应用进入前台；不能据此认定 RustDesk、SkyComputerUseService、用户或某个其他进程是激活请求的发起者。

输入空闲只是减少打扰的调度条件，不是新增产品验收条件。记录仅含距最近事件的秒数，不读取键入内容、剪贴板或其他应用内容。当前 runner 在首次等待超时即停止整批，避免对同一阻塞重复等待；本轮先遇到两次超时才收敛到这个行为。更早的一次等待在准备四画面前主动取消，见 [记录](setup-pause.json)。

## 隔离、正确性与清理

- 当前 release 构建成功。结束时源码 HEAD 为 `2f836d3`；[二进制 SHA-256](build.json)用于识别实际测试文件。本轮无产品 Rust 变更，未重复运行全量测试，也不把历史 91 项写成本轮结果。
- Swift 审计程序编译、Python 控制器语法与四份显式 PID 清单检查通过。Ghostty 生产者与 login PID 在任何信号发送前逐个复核；不会向 login 发 SIGUSR1。
- 单画面原生就绪图：[Keepline](keepline-one-ready.png)、[Ghostty](ghostty-one-ready.png)。四分屏 Ghostty 用真实菜单完成；这些截图是准备完成的证据，不是 30 秒有效采样证据。
- 原测试 runtime `/tmp/kl8-res-cu14f42c/runtime` 出现新增待办/终端后，另建 `/tmp/kl8-res-e_xxqjcj/runtime` 做干净单画面实例，四画面用 `/tmp/kl8-res-q1j2_abp/runtime`。未修改或删除新增待办内容，不把其画面截图提交。
- 两套纯测试客户端/后台/生产者、两套专用 Ghostty 均已结束；用户 Keepline PID 57531 与用户 Ghostty PID 637 仍运行，未操作。原测试实例因有新增内容保留客户端、后台和新终端，只停止最初的负载程序。任务数据库另作私有本地备份，路径见 [保留记录](retained-runtime.json)，数据库不提交。备份不等于恢复运行中的进程。清理记录见 [cleanup.json](cleanup.json)。
- 工作树并行修改的 `product.md` 不属于本轮，未改动、未纳入提交。未调用模型，未读凭据，未改系统通知/输入源/用户配置，未推送。

## 后续决定

采用有界自动前台审计；否定在用户操作期间持续抢焦点、把后台低值当作达标、或凭无效数字继续做猜测式优化。当前环境未提供有效连续前台窗口，CPU 新倍率和新增优化收益均待验证。A09 用户可见通知与点击跳转也保持未验证；不点击验收通知不影响 Keepline 正常使用。

复跑须先重建自己的隔离夹具并刷新 cases.json 的 PID 清单，保留既有 receipts，使用新输出前缀。编译 frontmost.swift 后运行 run-pair.py；不要直接重用已结束进程的 PID。
