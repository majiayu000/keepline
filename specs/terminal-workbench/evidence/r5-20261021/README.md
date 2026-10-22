# R5 解锁后的原生验收（2026-10-21）

决定：区分实际 UI 输入、合成会话导航与资源夹具。全部窗口操作/按键/截图通过 Computer Use；AppKit 仅激活自己的客户端、Ghostty 副本及 Finder。用户 Keepline PID 80936、用户后台和终端未操作。未调用模型、未改输入源设置。

## 原生结果

| 项目 | 结论 | 证据与边界 |
|---|---|---|
| A01 Finder 双击 | 通过本次冷启动路径 | [Finder](finder-bundle.png) 双击私有 .app，自动启动客户端/后台并显示 40 个合成外部会话，[首屏](finder-started.png)。最初夹具 HOME 路径过长导致单实例 UNIX socket 创建失败，改用短 /tmp HOME 后重测；不是首次尝试即成功。大量真实历史的索引预算仍见 M0，未在本次复制用户完整日志库 |
| A06 最后窗口重开 | 部分通过 | 点击关闭窗口后，再观察应用时窗口出现；但 CUA 观察可能触发 reopen，不能当作 Dock 点击证据。Dock 原生接口 timeout，未取得真实 Dock 点击回执；此前 Cmd-Q/客户端重连保留原 PTY 的 R2/M6 证据仍有效 |
| A17 40 会话跨空间 | 通过本次合成数据原生导航 | 4 空间各 10，会话各含 2 等待 + 1 错误，左栏需要你总计 12；鸟瞰全部 40，逐空间筛选均 10，标题归属正确。[计数](navigation-counts.json)、[全部](overview-40.png)、[空间1](space-1.png)、[空间2](space-2.png)、[空间3](space-3.png)、[空间4](space-4.png)。空间4卡片继续进入空间4项目目录，预填 r5-4-01，不自动执行：[截图](navigation-resume-space4.png)。离开/新标记另见 M3 |
| A05 四路输出/切换 | 通过切换与输出子项 | 4 个真实 PTY 各 10 Hz 输出，启用 1049 备用屏；切到输入标签再切回，4 个计数继续推进：[前](alternate-four-before.png)、[后](alternate-four-after.png)。原 80 列夹具超过分屏实际列数，因此发生正常折行；没有据此宣称完整 TUI 布局通过。鼠标/备用屏滚轮/焦点另见 R2；本次另经画面菜单放大/还原，见 [放大](alternate-zoom.png)、[还原](alternate-restored.png) |
| A18 四画面真实拼音 | 部分通过 | macOS 当前输入源始终 `com.apple.inputmethod.SCIM.ITABC`。四个 cat 中逐键 pressKey 真组字，非 typeText 模拟。pane4 nihao 选词一次得到你好；pane2 组字退格只删拼音，补 o 后选词；pane1 不选词直接 Return 得 nihao；pane3 zhongwen 选词得到中文。每个画面均只收到聚焦时内容，Ctrl-C 四个 cat 均退出 |

真实 IME 截图：[pane1组字](ime-pane1-marked.png)、[pane2退格](ime-pane2-backspace.png)、[pane3组字](ime-pane3-marked.png)、[pane4组字](ime-pane4-marked.png)、[结果](ime-four-result.png)、[四个cat退出](ctrl-c-four-cats.png)。独立 tee 文件逐字节读回：[ime-bytes.json](ime-bytes.json)。候选浮窗不在 CUA 应用截图中，不能声称本次验证其位置；中英切换及每个画面完整组合未逐一完成。用户此前豆包两画面候选位置通过仍是历史用户手测。

A03 附带发现：cat 没开启 bracketed paste 时，多行中文/emoji 粘贴被现有安全粘贴规则拒绝；不记为多行原生通过。后续 Return 在 pane3 产生一个空行，读回保留该事实。单行启动命令使用原生 paste；真实拼音按键另测，不能混用。

## 同机资源对照

采用 release Keepline + 自带后台，与官方 Ghostty 1.3.1 的专用副本对照（只改副本 bundle ID 和 ad-hoc 签名，原安装/配置不变）。另开没有历史内容的 Keepline runtime，不将 40 会话导航夹具计入基线。每组可见窗口、3 秒预热 + 30 秒采样、10 Hz 同一 Python 输出脚本；当前非被测夹具停止输出。单画面输出上限 80×20；四分屏改为 60×20，避免换行，两应用在各比较组内字节一致。字体 Menlo 12.5，Ghostty 160×48；Keepline 1440×889 点窗口含产品侧栏，终端区域约 1190×710 点，Ghostty 1200×728 点窗口。不是逐像素等尺寸对照。

`measure.py` 只测明确测试 PID 的 proc_pid_rusage，Mach timebase 换算累计 CPU，使用 phys_footprint 而非 RSS；记录原始样本。Keepline 计客户端+后台+输出进程，Ghostty 计客户端+输出进程。Ghostty 系统 login 包装进程无读取权限，单列排除；WindowServer/GPU/系统服务归因未测。资源比值用于发现明显差距，不冒充完整系统功耗。

首次 Ghostty 尝试错误地把 SIGUSR1 发给 login 包装层，导致夹具退出，样本作废；修正为递归确认 Python PID 后重测。第二次因 login 资源读取被拒绝而中止，也不计入结果。最终有效样本见 resource-*.json。

| 可见画面 | Keepline CPU 秒 | Ghostty CPU 秒 | CPU 比 | Keepline peak MiB | Ghostty peak MiB | 内存比 |
|---|---:|---:|---:|---:|---:|---:|
| 1 | 1.911 | 0.800 | 2.39× | 341.49 | 332.67 | 1.03× |
| 4 | 6.518 | 2.400 | 2.72× | 371.57 | 416.80 | 0.89× |

**A14 不通过：CPU 两组均超过 1.2×，内存两组在预算内。** 不提高预算、不用锁屏空闲数据替代。下一步需要针对绘制热路径做 profiling，本轮没有把发现缺陷写成通过。四分屏负载结束后的原生画面：[Keepline](resource-keepline-four-result.png)、[Ghostty](resource-ghostty-four-result.png)。
