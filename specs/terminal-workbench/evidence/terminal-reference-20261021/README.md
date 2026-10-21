# 终端渲染对照证据（Ghostty 1.3.1 vs Keepline，2026-10-21）

由 `next/eval/terminal-reference/run.sh` 生成后复制到这里。每张 PNG 从左到右依次是：Ghostty 真实窗口截图（已转为 sRGB）｜Keepline `screen.rs` 离屏渲染｜差异热图（Keepline 重采样到 Ghostty 的网格后逐像素比较；字格比例的差别单独记在 report 里）。

- 条件：Menlo 12.5pt，100×30，Retina 2×，前景 / 背景色取 Keepline 默认（`#d7dae0` / `#101217`）。
- 数字：[report.md](report.md)、[report.json](report.json)（每行的差异比例在 `row_changed` 里）。
- 结论与代码位置：[../../terminal-reference-gaps.md](../../terminal-reference-gaps.md)。

| 图 | 内容 |
|---|---|
| codex-logo / codex-start | 真实 Codex v0.163.0 启动画面（loading 时的盲文 logo、就绪后的界面） |
| vim-normal / vim-insert / vim-back-normal | vim 普通模式 → 插入模式 → 回到普通模式（光标形状随模式切换） |
| lazygit-status | lazygit 0.66.2 主界面（文件面板、diff、提交列表） |
| box-blocks | 制表线 U+2500–257F、方块 U+2580–259F、盲文 U+2800–28FF、Powerline、连续性测试 |
| text-unicode | 中日韩文字、emoji（VS16 / ZWJ / 国旗 / 肤色 / keycap）、组合字符 |
| sgr-colors | SGR 全部样式、16 色、256 色、真彩色 |
| cursor-block / cursor-underline / cursor-bar | DECSCUSR 2 / 4 / 6 |
| hyperlinks | OSC 8 链接（静态截图两边都看不出区别；判定见差距清单 #12） |
