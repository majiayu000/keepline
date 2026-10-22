# 终端渲染对照（Ghostty 作参照）

把同一段 VT 字节流分别放进 Ghostty 和 Keepline 的真实绘制代码，用同一字体、字号和行列数截图，生成差异图和数字报告。实测结论见 [specs/terminal-workbench/terminal-reference-gaps.md](../../../specs/terminal-workbench/terminal-reference-gaps.md)。

```bash
./run.sh            # 重新生成合成 fixture，两边渲染，比较
./run.sh --record   # 先重新录制 codex / vim / lazygit，再跑上面这些
```

需要：macOS、`/Applications/Ghostty.app`、调用方有“屏幕录制”权限、`uv`、Xcode，以及 zig 0.16（环境变量 `ZIG`，`run.sh` 里有默认路径）。录制还需要 `codex`、`vim`、`lazygit`。可调参数：`REF_COLS` `REF_ROWS` `REF_FONT` `REF_SIZE` `GHOSTTY`，Keepline 一侧用 `KL_REF_FONT` 覆盖字体。

| 文件 | 作用 |
|---|---|
| `gen_fixtures.py` | 生成合成 fixture：制表线 / 方块 / 盲文 / Powerline、emoji / 中文 / 组合字符、SGR 全部样式和颜色、DECSCUSR、OSC 8 |
| `record.py` | 在固定 100×30 的 PTY 里按时间线给真实 TUI 发按键，在检查点保存原始输出（相当于给 `script` 加了固定窗口大小和脚本化按键）。只录脚本自己发出的按键 |
| `fixtures/*.vt` | 原始 VT 字节流（录制的和生成的） |
| `../../crates/keepline-app/examples/terminal_reference.rs` | 用 `screen.rs` 的 `capture` + `paint_pane` 离屏渲染出 PNG。进程独立，不加单实例锁，不碰正在运行的 Workbench |
| `ghostty_shot.sh` | 用隔离配置（`--config-default-files=false`）启动一个新的 Ghostty 实例，显示 fixture，截取窗口并转为 sRGB |
| `compare.py` | 计算字格尺寸，生成左中右对比图和差异热图，输出 `out/report.{md,json}` |

注意事项：

- Ghostty 窗口必须是前台焦点，否则所有光标都画成空心块；脚本通过 `open -na` 启动并激活它，`compare.py` 会检查并报警。跑的时候 Ghostty 窗口会短暂跳到前台。
- `screencapture` 带的是显示器色彩配置，脚本用 `sips` 转回 sRGB 后才和 Keepline 比较。
- `run.sh` 只编译 example，不会重写 `target/debug/keepline-app`（那个文件可能正在运行）。
- `out/` 是生成物；需要留证据时，把 `out/compare/*.png` 和 `out/report.*` 复制到 `specs/terminal-workbench/evidence/` 下。
