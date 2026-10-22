# 分割树、输入控制权、符号字体复验

2026-10-20，`feat/agent-workbench`，工作树 `keepline-agent-workbench`。决定：采用分割树与进程内字体回退，保留每项验收证据。未提交、未推送，未重启用户后台 PID 8926 或停止原终端 PID 10304、33651、33764。

## 逐项结果

| 验收项 | 判定 | 证据与范围 |
|---|---|---|
| 任意叶节点向右 / 向下拆分并嵌套 | 通过 | core 嵌套树测试；原生动作测试从 8→9→10 画面，真实创建 shell，见 [native-render.log](native-render.log) |
| 不设数量上限、取消空位 | 通过 | core 测试构造 32 叶；原生实际 10 画面。空标签只有新终端入口，没有固定占位格 |
| 拖动分隔线改变比例并持久化 | 通过 | 原生鼠标事件拖动根分隔线 58%→67%，随后 RPC 读回核对数据库状态 |
| 放大单画面 / 还原 | 通过 | 原生事件点菜单第 3 项，Pane 数 4→1→4；只保留可见 Pane 订阅 |
| 关闭画面回收空间，PTY 不停止 | 通过 | core 关闭节点保留兄弟子树比例；原生动作 10→9→8，核对进程仍 Running 且 PID 存活 |
| 单画面不绘制分屏边框 | 通过 | [单画面原生图](single-native.png)；单叶 render 使用 border=0 |
| 右键与状态栏 ⋯ 使用同一菜单 | 通过 | 两入口共用 `PaneMenu/menu_view`；右键由 GPUI 原生事件分发打开，[菜单截图](menu-native.png) |
| 新分屏为空 shell，继承当前目录 | 通过 | 原生新终端 argv 为 `/bin/sh -l`、无 AgentSession；集成测试先 cd 再读取托管 shell 的 cwd |
| 新画面自动获得键盘焦点 | 通过 | 原生右 / 下拆分后核对新 Pane 的 FocusHandle，不依赖追加鼠标点击 |
| 分割树及比例重开后保持 | 通过 | `nested_split_tree_and_ratios_survive_store_reopen`：真实隔离宿主重开后树完全相等，SQLite 7 个规范化节点；原生客户端加载已存布局 |
| 没有存活 owner 时自动可写 | 通过 | `input_control_follows_live_connections_without_takeover_or_polling`：无 takeover 重连输入成功；原 owner 断开后已有只读连接收到可写推送 |
| 另一个 owner 存活时保持只读 | 通过 | 同一集成测试的第二连接保持只读；现有 takeover 真实 PTY 用例继续通过 |
| Nerd / Powerline 字形与格宽 | 通过 | CoreText 回退实际生成 6 种非缺字 glyph，核对单格 advance；三张图及 [真实窗口截图](eight-cua.png) 均可见图标 |
| 停止 Agent 不误杀 shell | 通过 | `stop_agent_clears_current_without_stopping_shell_or_changing_outcome`；`stop_agent_refuses_an_unidentified_shell`。使用隔离模拟 Agent，不启动官方 CLI |
| 搜索滚动内容 | 通过（核心测试） | `search_finds_scrollback_and_wide_graphemes_without_spacer_gaps` 覆盖已滚出的中文及组合字符；面板由显式搜索触发，无日志目录扫描 |
| 复制 / 粘贴 / 中断 / 待办关联菜单 | 已实现；界面流程未逐项实机验收 | 菜单接入选区/可见文本复制、现有安全粘贴、Ctrl-C、AssignTask；待办标签的“再开会话”选 Agent 后仅预填。没有把源码核对写成完整人工验收 |
| 8 个同时可见画面的 CPU / 内存 | 已测量 | 见下表、原始 JSON；测量前后只读窗口元数据确认没有其他应用窗口遮挡 |
| 相对 Ghostty ≤1.2 倍、长时间功耗 | 未验证 | 本次未运行同机同负载 Ghostty 基线，也未覆盖系统辅助进程的归因；不能声称产品卡整体资源预算已经通过 |
| 系统拼音候选窗、两画面真实拼音 | 用户手测待确认 | 按用户最新决定停止自动复测；本轮没有再次操作输入源，原生事件模拟不算真实 IME 证据 |

## 原生截图及验证方式

- [单画面](single-native.png)
- [嵌套四分屏](nested-native.png)
- [右键菜单展开](menu-native.png)
- [八画面](eight-native.png)

上述四张由真实 GPUI renderer 在 **1440×900 逻辑像素、2× scale（PNG 2880×1800）** 输出，内容来自隔离宿主的真实 PTY。菜单通过原生事件分发模拟鼠标打开；不是 HTML 重画，也不是用户真实输入法验收。
`visual-test` 仅用于截图和事件断言，最终交付二进制已重建为普通构建。

另用 Computer Use 读取临时 `Keepline Split Check.app`，执行该窗口暴露的 Raise 并截图，确认 8 个真实画面：[eight-cua.png](eight-cua.png)。未发送键盘输入、未切输入源、未操作其他应用。这个截图只证明当次窗口内容，不替代 CPU 采样；最终采样前后另有 [窗口元数据](resource-window.json) / [结束元数据](resource-window-after.json)。不同显示器上排序更靠前的窗口不等于遮挡，按矩形交集判断。

## 资源测量

硬件：Apple M3 Max / Mac15,8，128 GiB，macOS 26.5.1。普通 debug 客户端，Ghostty VT 为 ReleaseFast，`GPUI_PAUSE_IDLE_DISPLAY_LINK=1`（现有 run.sh 同设置）。实际原生窗口外框 **1440×888**，受系统工作区限制；不将它冒充 1440×900 内容区。

空闲 30 秒；随后 8 个 PTY 各约每秒 8 行，持续 30 秒。覆盖客户端、隔离后台及 8 个 cat 进程，读取累计 CPU 和 `phys_footprint`；内存为各阶段结束时读数，不是峰值。CPU 百分比按单核 100% 计算。最终 8 画面空闲采样中，覆盖进程的磁盘读写增量均为 0 B。

| 场景 | 覆盖进程累计 CPU | 平均 CPU（单核） | phys_footprint 合计 |
|---|---:|---:|---:|
| 8 画面空闲 | 0.093 秒 | 0.311% | 92.9 MiB |
| 8 画面持续输出 | 5.732 秒 | 19.060% | 326.5 MiB |

CPU 单位已纠正：`mach_timebase_info=125/3`，先将 Mach ticks 换成纳秒。初次结果低估约 41.7 倍，现已撤回。校准、首次负载失败与修正过程见 [CPU 勘误](cpu-units-correction.md)，最终原始数据见 [空闲](eight-pane-resources.json) / [持续输出](eight-pane-active-resources.json)。

## 测试与复现

- `cargo test --manifest-path next/Cargo.toml --workspace -- --nocapture`：**37 通过、0 失败**；仅 `daemon_fixture` 是刻意 ignored 的辅助进程入口。[最终日志](tests.log)
- `cargo clippy --manifest-path next/Cargo.toml -p keepline-core -p keeplined --all-targets -- -D warnings`：通过。
- `cargo build --manifest-path next/Cargo.toml --workspace`：通过，最终是无 visual-test 的普通构建。GPUI 上游仍有 695 条 cocoa 弃用等警告，不把上游警告写成零警告。
- 原生事件/几何断言：[native-render.log](native-render.log)，仅属于模拟/自动验证。
- 40 空闲 PTY：本轮 CPU 0.0000%、磁盘读写 0 B、Unix syscalls 0，业务后台计数无增长。
- 4 模拟 Agent / 30 秒：CPU 0.5358 秒、ps/lsof 增量 0、增量读取 87,932 B，预算仍为累计 CPU <0.6 秒。

构建环境：`DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`、`LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast`；`ZIG` 指向测评目录 vendor/zig/zig。

```sh
# 编译一次截图版本，再运行隔离原生测试。
cargo build --manifest-path next/Cargo.toml -p keepline-app --features visual-test
KL_NATIVE_ONLY=1 python3 specs/terminal-workbench/evidence/split-tree-20261020/native-check.py
# 恢复普通构建；单独测资源，期间不要运行编译或遮挡测试窗口。
cargo build --manifest-path next/Cargo.toml --workspace
KL_RESOURCE_ONLY=1 python3 specs/terminal-workbench/evidence/split-tree-20261020/native-check.py
```

脚本只创建并清理自己的临时宿主与 PTY，不使用真实 ~/.codex 或 ~/.claude，不启动官方 Agent。重跑截图前需先 `cargo test --no-run` 编译 fixture。

## 文件与交付边界

- `keepline-core/src/{layout,model,protocol,terminal}.rs`：树、对象归属、major 2 协议、显式滚动搜索。
- `keeplined/src/{store,host,recognition,log_index}.rs`、`tests/managed_session.rs`：规范化布局表、控制权交接、当前 cwd、Agent 停止、资源计量与热点 SQL。
- `keepline-app/src/{workbench,pane,connection,screen,theme,main}.rs`：嵌套绘制/拖动/菜单/放大、可见 Pane、焦点、字体回退与测试截图入口。
- `keepline-app/assets/`：Symbols Nerd Font Mono v3.4.0 + MIT 许可证。字体 SHA-256：`f0f624d9b474bea1662cf7e862d44aebe1ae1f6c7f9cb7a0ca5d0e5ac9561c60`。
- `architecture.md`：状态归属、协议/库边界、阶段表、资源单位修正；原 IME 记录注明后续人工确认。

字体取自 [Nerd Fonts v3.4.0 SymbolsOnly](https://github.com/ryanoasis/nerd-fonts/tree/v3.4.0/patched-fonts/NerdFontsSymbolsOnly)，参考 [Ghostty 内置 Nerd Fonts 与格宽适配](https://ghostty.org/docs/install/release-notes/1-2-0)。只在当前应用进程注册，不安装系统字体。

**部署边界：wire major 已为 2，旧固定网格库明确拒绝打开，不自动迁移、回填或覆盖。** 按仓库“不做未授权旧数据迁移”的约定，本轮在新隔离 runtime 验证，没有把新协议宿主替换到用户仍运行的 major 1 后台。旧后台及原终端继续运行；用户现有库升级不是本轮已完成事项。
