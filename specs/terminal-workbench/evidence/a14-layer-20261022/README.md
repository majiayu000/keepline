# A14 前台 CPU：终端网格单层绘制（2026-10-22）

**决定：采用单层绘制（commit c68e2da）；A14 新的前台倍率待验证，A14 维持不通过。** 本轮没有取得任何有效的前台资源样本：02:40 起本机一直锁屏，到 09:30 自动截止时仍锁定（用户已约 6.7 小时无输入）。最后一次有效倍率仍是 R8 的 1.60×（1 画面）/ 1.24×（4 画面），那是旧构建的数据，不代表本次改动后的结果。下面的绘制路径 A/B 只说明热点已经消除，**不是** A14 倍率，也不能据此推算 Ghostty 比值。

## 支撑的决定与放弃条件

要回答的问题：GPUI 路线能否在不改 GPUI 补丁的前提下，把前台 CPU 降到 Ghostty 的 1.2× 以内（累计 CPU，含后台和生产者进程）。如果在有效前台样本中，1/4 画面仍明显高于 1.2×，且剩余热点无法在客户端侧继续消除，就按产品卡 3.1 的条件评估是否切换到 Swift + GhosttyKit。本轮证据还不足以触发这个决定。

## 归因（先验证，后修改）

R11 修复后火焰图（[flamegraph-after.json](../r11-20261021/flamegraph-after.json)，4 画面，5563 ms 样本）显示，`gpui::bounds_tree::BoundsTree::insert` 的**自身时间占全部采样的 50.5%**，来源有两处：
- `Scene::insert_primitive`（实时绘制，经 `paint_glyph`/`paint_quad` 调用，inclusive 17.2%）：每个 glyph、每个单元格背景都要插入一次 R-tree，用来算叠放顺序；
- `Scene::replay`（inclusive 32.9%）：未变化的 Pane 走 GPUI 缓存子视图，但 replay 会把上一帧的每个图元重新插入 BoundsTree，所以“缓存”的画面每帧仍要付出几乎同样的开销。

GPUI 的 `Scene::insert_primitive` 在 layer 栈非空时，直接使用 layer 的 order，跳过 BoundsTree。热点在客户端的场景构建层，不在后台解析、IPC 或提交频率（R8 已测得后台约 2%、生产者约 3%）。

## 改动

`screen::paint_cached` 把网格（终端背景、单元格背景、装饰线、字形、sprite）放进一次 `window.paint_layer(bounds, …)`。光标仍在层外绘制；Pane 的悬停链接、选区和 IME 预编辑浮层本来就在 `paint_cached` 之后绘制，所以仍在字形之上。层内的叠放顺序按 GPUI 图元类型排列：quad 在 sprite 之下，与 Ghostty 先画背景、再画文字的做法一致。

`paint_layer` 是 GPUI 的公开 API。**`next/patches` 没有改动，没有新增维护代价。**

## 前台无关的 A/B（不是 A14）

[`paint_bench`](../../../../next/crates/keepline-app/examples/paint_bench.rs)（`--features visual-test`）使用真实的 GPUI 窗口、真实的 `screen.rs` 和 libghostty-vt。它每帧把 R8 单画面或 R5 四画面负载的下一帧字节喂给一个 Pane，然后调用 `Window::draw`（布局、绘制、生成 scene）；4 画面时其余三个 Pane 和工作台一样走 `.cached()` replay。计时是主线程 CPU（`CLOCK_THREAD_CPUTIME_ID`），先预热 50 帧，再计 2000 帧。它**不包含** Metal 提交与呈现、显示链接、后台、IPC 和工作台其余 UI。基线与优化版只差 `screen.rs`，交替运行三轮。原始数据见 [paint-bench.jsonl](paint-bench.jsonl)，二进制哈希见 [build-sha256.txt](build-sha256.txt)。

| 画面 | 基线 µs/帧（3 轮均值） | 单层 µs/帧 | 变化 |
|---|---:|---:|---:|
| 1 | 1013 | 516 | −49% |
| 4 | 3236 | 604 | −81% |

三轮之间的波动不到 2%。当时有其他会话在本机编译，基线和优化版承受同样的负载。真实应用每秒画多少帧、4 个 Pane 的帧在多大程度上被合并，本基准都没有测，所以不能把这里的节省换算成 A14 的 CPU 秒数或倍率。

在优化版上用 `sample` 采样，剩余开销分散在 VT 单元格读取（`ghostty_render_state_row_cells_get` 等）、scene 插入与排序、`paint_glyph` 和光栅边界查询上，没有单一主导的热点。

## 正确性

- `cargo fmt --check` 通过；`cargo clippy --workspace --all-targets -D warnings` 通过（Rust 1.95.0）。
- `cargo test --workspace`：app 25、core 20、daemon 32、集成 27 项通过，另 1 个 ignored 是由集成测试启动的 daemon fixture，合计 104 项。见 [tests.txt](tests.txt)。分支新增测试后，总数已不是 R13 的 91 项。
- 像素回归：用 `terminal_reference` 在 100×30、Menlo 12.5、2× 下渲染 13 个 fixture，对比改动前后的输出，见 [render-comparison.json](render-comparison.json)。11 个完全一致；lazygit-status 有 2 个像素差，且都 ≤32；box-blocks 有 1061 个像素差超过 32（约 0.08%），只出现在相互交叉的斜线和弧线 path（╳、╱╲、圆角）的抗锯齿上，放大后对比形状不变。原因是层内这些 path 共用同一个 draw order，由 GPUI 合并为一批合成。本轮没有重新和 Ghostty 截图比较。
- 已知既有问题，非本次引入：`cargo clippy -p keepline-app --features visual-test` 报 `screen::capture` 未使用和 workbench.rs 两处可折叠 `if`；这个特性组合不在 CI 检查范围内。

## 前台复测尝试

为本轮准备了可重复的夹具，均未取得样本：
- [fixture.py](fixture.py)：使用隔离的 runtime 和 HOME、专用测试 App、R8 单画面负载 / R5 四画面 2×2 负载，输出只含显式 PID 的 spec；
- [ghostty-fixture.py](ghostty-fixture.py)：从 /Applications 复制 Ghostty 1.3.1，只改副本的 bundle ID，并做 ad-hoc 签名。四画面由第一个 surface 通过 AppleScript 让**副本自身**分屏。这条路径因为锁屏没有跑通，属于未验证；
- [run.py](run.py)：等待“未锁屏且 20 秒没有键鼠输入”，再依次跑 opt-1、Ghostty-1、opt-4、Ghostty-4、base-1、base-4。每个用例用未修改的 R5 measure.py 和 [frontmost.swift](frontmost.swift) 审计前台，只在开始时激活一次，失焦即作废，最多 3 次。

运行记录见 [run-log.json](run-log.json)：03:27 启动，09:30 截止，第一个用例都没有开始。run.py 每 15 秒检查一次，但只记录了截止事件；锁屏状态是在 02:40、03:26、09:30 手动抽查得到的（`CGSSessionScreenIsLocked=1`，见 [lock.swift](lock.swift)），09:30 时用户已约 24300 秒无输入。没有强抢焦点，没有拿锁屏或后台下的低值计算倍率。凌晨的一次 Ghostty 探针曾创建出无 surface 的窗口，已立即结束，不产生数据。用户的 Keepline（target/bundle）、Ghostty、后台和 /Applications 都没有动过。

## 复跑方法

`ZIG` 必须指向 Zig 0.16（本机 Homebrew 是 0.15.2，会让 libghostty-vt-sys 构建失败；可用的 0.16 在 `.claude/worktrees/terminal-eval-04a203/eval/terminal/vendor/zig/zig`）。构建 `LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast cargo build --release -p keepline-app -p keeplined`，再把基线（父提交 cc66156）和本提交的二进制分别放进两个目录。然后分别编译 `frontmost.swift`、`lock.swift`、`window-id.swift` 到 `/tmp/kl14-idle-guard`、`/tmp/kl14-lock`、`/tmp/kl14-wins`，在一段不用电脑的时间运行 `run.py <opt> <base> <截止 epoch>`。拿到 opt 与 Ghostty 的有效配对后，再更新 A14 的结论。
