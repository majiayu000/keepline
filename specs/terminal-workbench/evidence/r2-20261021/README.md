# R2 终端渲染与交互验收

日期：2026-10-21。目标：补齐终端协议/字格表现的明确差距；拒绝用截图差异低替代交互正确性。只操作隔离 runtime，用户 Keepline PID 80936 与后台未重启。

第一批：独立提交 a5fd977，审查见 [batch1-review.md](batch1-review.md)。第二批包含 mode 2027、鼠标/备用屏滚轮、符号自绘、OSC 8、ic_width、焦点事件。

## 真实 Computer Use

| 项目 | 结论 | 证据 |
|---|---|---|
| Vim 鼠标与备用屏滚轮 | 通过 | [vim-native-scroll.png](vim-native-scroll.png)：从第 1 行滚到第 16 行，光标随实际点击定位 |
| lazygit 鼠标选择 | 通过 | [lazygit-native-click.png](lazygit-native-click.png)：点击 Local branches 后右侧切为该分支提交日志 |
| htop 鼠标选择 | 通过 | [htop-native-click.png](htop-native-click.png)：仅显示测试 PID 10469/80898，点第二行选中测试后台；未执行 kill/renice |
| OSC 8 悬停 | 通过 | [osc8-hover.png](osc8-hover.png)：移动到 OPEN TEST FOLDER 后下划线覆盖链接单元格 |
| OSC 8 Cmd 点击 | 通过 | [osc8-finder-open.png](osc8-finder-open.png)：系统 Finder 打开 file:///tmp/kl-r2-ggjte6r1/project；只关闭本次测试 Finder 窗口 |
| ?1004 | 通过 | 真实 Vim 的 FocusLost/FocusGained autocmd 写入 [focus-events.txt](focus-events.txt)，包括测试客户端置前/失焦和标签切换 |
| 客户端退出/重开 | 通过（附带） | Cmd-Q 只退出测试客户端；重开后原 Vim、lazygit、shell 保持且自动可写，测试后台 PID 80898 不变 |

AppKit 仅用于用户已授权的测试窗口置前，按键、鼠标、截图均用 Computer Use。最初前台焦点不稳定，vim-before.png / vim-scroll.png 是排障过程截图，不是成功证据。修复画面外 release 吞掉标签点击后重开测试客户端复测。native-fixture.py 只搭建隔离服务/进程，未通过 RPC 注入交互操作。

## 自动测试与像素对照

单测覆盖 emoji 家庭/肤色/旗帜/keycap、snapshot 重连宽度、鼠标 SGR/传统协议/修饰键/移动去重、备用屏 application cursor 模式、?1004 开关、链接双宽单元格及危险协议拒绝、所有 box/Powerline 字符及盲文几何。原生字体几何测试属于模拟 marked text，不能冒充系统拼音验收。

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

完整报告：[after-report.md](after-report.md)、[after-report.json](after-report.json)。主要改善图：box-blocks-after.png、codex-logo-after.png、text-unicode-after.png、lazygit-status-after.png。

保留差距：Menlo 字格 h/w +3.0%；Ghostty 本轮失焦光标不能作为聚焦光标对照；双线/圆角 sprite 形状与抗锯齿仍有差异；没有声称逐像素一致。R5 的四画面 IME 与性能预算另行验收。

最终默认并行 `cargo test --workspace`：82 项通过（app 21 / core 19 / daemon 24 / integration 18），一个 ignored 为被测试调用的 fixture；见 [workspace-tests.txt](workspace-tests.txt)。原生字体/marked-range 几何通过，见 [native-geometry.txt](native-geometry.txt)。本机原无 htop，使用 Homebrew 官方 formula 安装；未使用 sudo 或修改用户 TUI 配置。
