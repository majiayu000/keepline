# 键盘与客户端生命周期修复（2026-10-20）

决定：采用物理键映射 + Ghostty 当前终端模式编码；采用单一客户端锁及 macOS 重开回调。普通构建已完成，无提交、无推送。没有重启用户 major 1 后台，也没有迁移或删除其数据库。

本轮评价指标：Ctrl/Alt 必须到达真实 PTY/TUI；退出和重开必须保留宿主及原 PTY；版本不兼容必须有可见说明。若需停止用户终端或迁移旧库才能测试，则放弃该做法，使用隔离 runtime。

## 逐条证据

| 验收项 | 判定与证据类型 | 证据 |
|---|---|---|
| Ctrl-C/D/Z/L/R/反斜线、Ctrl+数字及标点 | 通过，编码单测 | `legacy_control_and_alt_keys`：03/04/1a/0c/12/1c；另测 Ctrl-6、^、_、?、Space。Ctrl-[ 按 Ghostty fixterms 输出 CSI-u，并非自行改写为 ESC。 |
| Alt+字母、数字、标点 | 通过，编码单测 | a/c/z/1/斜线的 ESC 前缀；设置 macOS OptionAsAlt，未覆盖终端自身 DEC 模式。 |
| Kitty 协商后的组合键 | 通过，编码单测 | VT 实际接收 `CSI >1u`，验证 Ctrl 组合、Alt-A 协议序列；pop 后 Ctrl-C 恢复 03。 |
| 真实 cat 的 Ctrl-C | 通过，Computer Use | 新开的 zsh 中启动 cat；[按键前](cat-before.png)，[Ctrl-C 后](cat-after-ctrl-c.png)显示 ^C 及 shell 提示符。不是直接写 0x03 到协议的模拟。 |
| 真实 Codex TUI 连续两次 Ctrl-C | 通过，Computer Use | Codex 0.163.0 `--no-daemon`；[TUI 就绪](codex-ready.png)、[第一次后仍在 TUI](codex-first-ctrl-c.png)、[第二次后回 shell](codex-second-ctrl-c.png)。未输入提示词或请求模型运行。 |
| 可见「退出 Keepline」菜单 | 通过，Computer Use | [菜单 AX 原文](quit-menu-ax.txt)；实际点击该菜单项，测试客户端退出。菜单弹出期间截图 API 不可用，没有伪造菜单截图。 |
| ⌘Q 退出客户端 | 通过，Computer Use + 进程读回 | 工作窗口及最终构建的版本提示窗口均能退出；退出后测试宿主、shell 和 cat 继续运行。 |
| 最后一个窗口关闭后再次显示 | 通过，Computer Use | 点击原生关闭按钮后，CUA 再次获取该应用时出现新窗口；[重开并继续输入](reopened-content.png)。原客户端 PID 23873 未变。未把 CUA 的再次定位操作说成物理点击 Dock。 |
| Dock 重开回调 | 已实现；没有取得直接点击 Dock 的独立证据 | 使用 GPUI `on_reopen`（macOS 应用重开事件）接同一恢复入口。尝试获取 Dock 的 Computer Use 接口超时，因此没有声称 Dock 点击实测通过。 |
| 退出进程再启动，PTY 与内容保留 | 通过，Computer Use + PID 对照 | ⌘Q 后再次启动，仍是 shell 23872 和 cat 24993；[历史内容与继续输入](quit-restart-content.png)。[进程记录](preserved-processes.txt)。 |
| 重复启动只有一个新版本客户端 | 通过，真实进程测试 + 单测 | [重复启动记录](duplicate-launch.json)：第二进程退出码 0，主实例 23873 不变，Workspace 不变。单测覆盖锁占用、唤醒事件、锁释放后重用；不声称能约束此前未实现实例锁的旧二进制。 |
| 新客户端遇到用户真实 major 1 宿主 | 通过，Computer Use | [最终警告窗口](old-backend-final.png)包含指定重启警告及不会保留的数据说明。只交换 Hello，不读取旧 Workspace；宿主 PID 8926 启动时间仍为 20:59:19。 |
| 不迁移、不破坏用户旧库和终端 | 通过，代码路径 + 进程读回 | 协议不匹配进入错误窗口，不进入 daemon spawn。用户原 shell 10304/33651/33764 仍在；只有隔离 fixture 在收尾时停止。 |

## 构建与测试

- `cargo test --manifest-path next/Cargo.toml -p keepline-app`：**10 通过，0 失败**，见 [测试日志](unit-tests.log)。其中原有 6 项回归、新增 2 项键盘编码和 2 项生命周期/旧协议测试。没有把多组断言算成额外测试，也没有复用上一轮的 37 项数字。
- `cargo build --manifest-path next/Cargo.toml -p keepline-app`：成功，[构建日志](build.log)。上游 GPUI 的 695 条 macOS 弃用警告仍在。
- ZIG 使用 terminal-eval 工作树中的固定工具链；`LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast`；`DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`；原生测试使用普通构建，未开启 `visual-test`。
- [环境记录](environment.json)。终端 fixture 日志索引指向临时目录，不扫描用户真实日志目录；Codex TUI 使用本机现有 CLI，未改变其配置、权限或模型。
- 初次 fixture 准备时，短命令退出后其子进程被执行器回收；改用持续运行的测试命令后再执行上述真实验收。旧后台警告与最终验收截图均来自实际应用，不用进程启动尝试冒充成功。

## 本轮代码文件

- `next/crates/keepline-app/src/pane.rs`：修饰键委托给物理键映射和 Ghostty；保留 IME 路径。
- `next/crates/keepline-app/src/keyboard.rs`（新增）：字母、数字、标点/导航键映射、模式编码及测试。
- `next/crates/keepline-app/src/lifecycle.rs`（新增）：单实例锁/唤醒、已有后台版本检测及测试。
- `next/crates/keepline-app/src/main.rs`：可见退出菜单、快捷键、重开入口、可见的启动错误页；仅后台不存在时允许正常启动。
- `architecture.md` 与两份 evidence README：状态、实现决策、用户手测来源及本轮证据。

## 提交整理检查

2026-10-20 提交整理时发现 `gpui-idle.patch` 与已验证依赖源码有一处格式差异，会使 bootstrap 的反向检查失败。补丁已从同一锁定版本的实际 diff 同步；在临时目录对上游原文件应用后，与本机已测文件逐字节一致，随后本地 `next/bootstrap.sh` 通过。没有修改运行逻辑，没有重新下载依赖；干净机器的网络 bootstrap 仍未验证。`.deps`、`target` 排除，正式报告引用的 10 份测试/构建日志纳入证据。
