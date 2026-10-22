# 终端渲染实测差距清单（对照 Ghostty 1.3.1）

日期：2026-10-21。对照方式：同一段 VT 字节流、同一字体（Menlo）、同一字号（12.5pt）、同一行列（100×30），分别在 Ghostty 和 Keepline 的真实绘制代码（`screen.rs` 的 `capture` + `paint_pane`）里显示，截图后逐像素比较。

- 要支撑的决定：终端渲染层下一轮先修哪些问题，以及以后改 `screen.rs` / `theme.rs` 时用什么挡住回归。
- 放弃条件：如果 Keepline 侧离屏渲染和真实 Workbench 窗口显示不一致，这套对照就只能作参考，需要改成对真实窗口截图。本轮还没在真实窗口上复核（见文末“局限”）。
- 结论：**采用**。这套对照能稳定重跑，并且实测确认了 15 项差距；制表/方块/盲文/Powerline 这一组和 SGR 样式这一组差距最大。字格比例问题已经由新单测拦住。

证据：[evidence/terminal-reference-20261021/](evidence/terminal-reference-20261021/README.md)（每张图依次为 Ghostty｜Keepline｜差异热图）。重跑方法：`next/eval/terminal-reference/run.sh`（见该目录 README）。

## 严重程度口径

- **高**：常用工具日常可见，或者功能错误（信息丢失、交互失效）。
- **中**：明显的视觉差异，但信息还在。
- **低**：少见、细微，或者在容差内。

“证据”一列：**截图** = 两边实测图里能直接看到；**代码** = 静态截图无法区分（交互类），由读代码确认，未做真实事件测试。

## 差距清单

| # | 问题 | 影响的工具 | 严重 | 证据 | 代码位置 |
|---|---|---|---|---|---|
| 1 | SGR 2（变暗）没有渲染，暗色文字按正常亮度显示 | Codex（提示语、路径、命令说明、启动 logo 都是 dim）、Claude Code、shell 自动建议 | 高 | 截图 `codex-start` `codex-logo` `sgr-colors` 第 1 行 | `next/crates/keepline-app/src/screen.rs:123-131` 只取 bold/italic/inverse；`Run`（`screen.rs:2`）没有 faint 字段 |
| 2 | Emoji 字素簇被拆开：ZWJ 组合（👨‍👩‍👧‍👦、🧑‍💻）、肤色（👍🏽）、国旗（🇨🇳）、keycap（1️⃣）都显示成多个零件，占用的格数也比 Ghostty 多（同一行多约 11 格），后面的内容整体右移 | Codex / Claude Code 输出里的 emoji、git log 提交信息 | 高 | 截图 `text-unicode` 第 7 行 | `next/crates/keepline-core/src/terminal.rs:72` `Vt::new` 没开 mode 2027（`Mode::GRAPHEME_CLUSTER`），Ghostty 默认开（`grapheme-width-method = unicode`）。这是 VT 层的设置，不在渲染代码里；改它会影响后台进程的屏幕状态和 IME 宽度预测 |
| 3 | 光标形状（DECSCUSR 0–6）被忽略，一律画成 60% 透明的块 | vim/neovim 插入模式（应为竖线）、zsh vi-mode、所有切换光标的 TUI | 高 | 截图 `cursor-bar` `cursor-underline` `vim-insert`（Ghostty 是竖线，Keepline 是块） | `screen.rs:365-379` 固定画块；`capture` 不读光标样式 |
| 4 | 下划线全部缺失：SGR 4、4:2 双线、21、4:3 波浪、4:4 点线、4:5 虚线，以及 SGR 58 下划线颜色 | lazygit（底栏 Donate / Ask Question 链接）、vim 拼写检查、编辑器诊断、带链接的 CLI 输出 | 高 | 截图 `sgr-colors` 第 1–3 行、`lazygit-status` 最后一行 | `screen.rs:275-276` 的 `TextRun` 写死 `underline: None` / `strikethrough: None`；`capture` 不读 `st.underline` |
| 5 | 鼠标上报缺失：程序打开 ?1000/1002/1003/1006 后，点击、拖动、滚轮都不转发；备用屏下滚轮只滚 Keepline 自己的回滚区，不会转成方向键（?1007） | vim / lazygit / htop 的鼠标操作，以及全屏 TUI 里的滚轮 | 高 | 代码 | `next/crates/keepline-app/src/pane.rs:445-454`：鼠标只做本地选区，滚轮只调 `vt.scroll` |
| 6 | 方块元素 U+2580–259F 在行与行之间有缝；阴影字符 ░▒▓ 用字体自带的点阵画，而 Ghostty 画成实心半透明色块 | 进度条（Codex / Claude Code）、btop、lazygit 滚动条 ▐、TUI 图表 | 中 | 截图 `box-blocks` 第 11–17 行（连续块） | 没有自绘路径：`paint_pane`（`screen.rs:319`）把所有字形交给字体排版。行高 15px 大于 Menlo 字形高度（ascent+descent ≈ 14.55px） |
| 7 | 制表线竖线 │ 在行与行之间断开（横线和圆角基本正常） | lazygit 面板边框、Claude Code / Codex 方框、tmux 分隔线 | 中 | 截图 `lazygit-status` 左侧面板 | 原因同 #6；Ghostty 用 sprite 自绘 U+2500–257F |
| 8 | 盲文 U+2800–28FF 的点挤在字形框里，格与格之间有缝，Codex logo 看起来一块一块的；Ghostty 把 2×4 点均匀铺满整格 | Codex 启动 logo（用户最初报的问题，1.2 修正后仍然存在）、终端图表 / 绘图工具 | 中 | 截图 `codex-logo`、`box-blocks` 第 19–22 行 | 原因同 #6 |
| 9 | Powerline 分隔符 E0B0–E0BF 没拉伸到满格高，色块上下露出背景 | starship、p10k 等提示符和状态栏 | 中 | 截图 `box-blocks` 第 24–25 行 | `screen.rs:285-300` 只对 PUA / 双宽字形按宽度缩放，没按格高对齐 |
| 10 | 删除线 SGR 9、上划线 SGR 53 没有渲染 | git diff 工具、TODO 列表、部分 TUI | 中 | 截图 `sgr-colors` 第 2 行 | 同 #4 |
| 11 | SGR 8（隐藏）的文字仍然显示 | 少数程序用它隐藏输入或占位文字；可能泄露应当隐藏的内容 | 中 | 截图 `sgr-colors` 第 2 行（“hidden” 在 Keepline 可见） | `screen.rs:123-131` 不读 invisible |
| 12 | OSC 8 超链接打不开，悬停也没有下划线；裸 URL 不能 cmd+点击 | Codex / Claude Code 的文件链接与网址、`gh`、`ls --hyperlink` | 中 | 代码（静态截图两边都不显示下划线，看不出区别） | `capture` 不读超链接；`pane.rs` 没有链接点击处理 |
| 13 | CJK 字形比 Ghostty 大、更粗：Keepline 把字形缩放到正好占满两格，Ghostty 按表意字宽度（ic_width）缩放并留出边距 | 所有中文 / 日文 / 韩文输出 | 低 | 截图 `text-unicode` 第 2–4 行、`vim-*` 第 1 行 | `screen.rs:285-300` 的 `scale = target / line.width()` |
| 14 | 字格高宽比：Menlo 下 Keepline 2.0×（15.06×30 px），Ghostty 1.93×（15×29 px），高了 **+3.0%**，在 5% 容差内。换成 JetBrains Mono（Theme::mono 的首选字体）后，按公开字体度量推算约为 **−9%**（Keepline 会更扁），本机没装这个字体，未实测 | 所有字形（字符画最明显） | 低（Menlo）/ 待验证（JetBrains Mono） | 截图 `report.md`；单测 | `next/crates/keepline-app/src/theme.rs` 的 `TERMINAL_LINE_HEIGHT`（固定倍数）与 `screen.rs` `CellMetrics::measure`；Ghostty 用字体自己的 ascent−descent+line_gap |
| 15 | 焦点事件 ?1004 不发送 | vim `autoread`、部分 TUI 的焦点刷新 | 低 | 代码 | app 代码里没有 `FOCUS_EVENT` 的使用 |

## 修复记录

- **第一批已修（R2 接手审查后独立提交）：#1 变暗、#3 光标形状、#4 下划线（单线、双线、波浪、点线、虚线和下划线颜色）、#10 删除线和上划线、#11 隐藏文字。** 改动在 `next/crates/keepline-app/src/screen.rs`：`Run` 改成带 `Attrs`，装饰线按字格自绘，光标按 `CursorVisualStyle` 画。新增单测 `sgr_decorations_and_faint_survive_capture`、`decscusr_shape_reaches_the_painter`。重跑 `run.sh` 后，Codex 启动画面的差异像素从 3.4% 降到 1.2%，logo 从 4.7% 降到 3.5%；`sgr-colors` 逐项目视与 Ghostty 一致，但数字没有变化（2.2%），因为它主要由字形抗锯齿和背景色造成。光标对比这次运行里 Ghostty 失焦画成空心块，只能确认 Keepline 侧已经画成竖线和下划线，两边对比要重跑。

## 已排除 / 未判定

- **256 色和真彩色：已排除。** 早先截图颜色偏差很大，原因是 `screencapture` 带上了显示器色彩配置（M27P20）。转换到 sRGB 之后，调色板和真彩色与 Keepline 一致（误差 ±10 以内，来自 ICC 换算）。脚本现在会自动转换。
- **ASCII、粗体、斜体、反色、16 色、常见组合字符（é ä ñ）：基本一致。** Menlo 没有连字，两边都不合并。
- **SGR 5 闪烁：未判定。** 静态截图看不出是否闪烁。
- **Keepline 真实窗口的色彩管理：未知。** `render_to_image` 输出的是原始 sRGB 值，GPUI 窗口在屏幕上是否做了色彩管理，本轮没有测。

## 局限

- Keepline 一侧用的是 `examples/terminal_reference`：调用同一份 `screen.rs` 的 `capture` + `paint_pane`，离屏 `render_to_image`，不经过 Workbench 的窗口框架、后台进程和 PTY。这样做是为了不启动、不影响用户正在使用的 Workbench 实例（完整 App 启动时会把请求转发给已在运行的实例）。
- Ghostty 一侧截的是真实窗口，需要“屏幕录制”权限；每张截图会短暂把 Ghostty 窗口切到前台（窗口失焦时 Ghostty 把所有光标画成空心块，会让光标对比失效；`compare.py` 会检查这一点并报警）。
- 交互类差距（#5 #12 #15）只读了代码，没做原生事件测试。

## 以后怎么提前发现

1. `screen.rs` 新增单测 `cell_aspect_ratio_stays_within_five_percent_of_ghostty_for_the_resolved_font`：从字体文件读度量，按 Ghostty 公式算出格子尺寸，Menlo 的结果与实测截图（15×29）锚定。把行高改回 1.6 时测试失败（+37.5%）；如果装了 JetBrains Mono，测试会改测这个字体，按推算会失败，这正好暴露 #14。
2. 改 `screen.rs` / `theme.rs` / 字体相关代码后，跑一遍 `next/eval/terminal-reference/run.sh`，看 `out/report.md` 的 changed px 有没有上升，以及 `out/compare/` 的差异图。
3. 修完上表任一项后，把对应 fixture 的 changed px 记进这份清单，作为下一轮的基线。

2026-10-21 R2 接手：第一批审查与全量三轮测试通过，重新执行 13 个 fixture，见 [接手记录](evidence/r2-20261021/batch1-review.md)。Ghostty 光标失焦警告仍保留，不能以差异百分比替代焦点验收。

### R2 第二批（2026-10-21）

采用：#2 默认开启 mode 2027，后台/客户端快照共用 VT；#5 libghostty 鼠标编码及备用屏方向键滚轮；#6–9 按格自绘块、制表线、盲文和 Powerline；#12 OSC 8 白名单链接；#13 ic_width 的 CJK 字体缩放；#15 按窗口和画面焦点发送 ?1004。

| fixture | 接手基线 | 本轮修复后 |
|---|---:|---:|
| box-blocks | 6.8% | 3.2% |
| codex-logo | 3.5% | 1.5% |
| codex-start | 1.2% | 1.2% |
| cursor-bar | 0.2% | 0.2% |
| cursor-block | 0.2% | 0.2% |
| cursor-underline | 0.2% | 0.2% |
| hyperlinks | 0.6% | 0.6% |
| lazygit-status | 5.1% | 3.8% |
| sgr-colors | 2.2% | 1.7% |
| text-unicode | 4.8% | 4.0% |
| vim-back-normal | 0.5% | 0.5% |
| vim-insert | 0.6% | 0.6% |
| vim-normal | 1.0% | 0.9% |

13 个 fixture 已重新执行 run.sh。Menlo 行高差仍为 +3.0%；背景色、抗锯齿和部分制表线接缝形状仍有差异。Ghostty 失焦光标警告保留，不宣称光标像素完全一致。裸 URL 自动识别不在本轮 OSC 8 实现范围。

原生鼠标验证发现画面外松开事件会吞掉标签点击；修为仅释放从本画面开始的手势。修后真实 Vim 滚轮、lazygit 分支面板点击、htop 进程行选择、OSC 8 Cmd 点击打开 Finder 测试目录通过，见 [R2 证据](evidence/r2-20261021/README.md)。
