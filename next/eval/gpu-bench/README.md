# GPU 占用基准

隔离的 fixture daemon 与 `/bin/sh` 负载（不跑 Agent、不读真实数据），测 Keepline 客户端在固定终端负载下的 GPU / CPU，并可与 Ghostty 同负载对照。

```sh
cargo build --release -p keepline-app
cargo test -p keeplined --test managed_session --no-run   # fixture daemon
python3 bench.py --ghostty                 # 全部场景，各 15 s
python3 bench.py --scenes mixed --interact # 鼠标扫过窗口并打字
GPUI_DAMAGE_VERIFY=1 python3 bench.py      # 每个局部重画帧与完整渲染逐像素比对
```

- GPU：`ioreg` 中 AGX user client 的 `accumulatedGPUTime`，按进程差分，单位 ms/s（每秒 GPU 忙的毫秒数）。
- `frames`：`KEEPLINE_FRAME_STATS=1` 时 GPUI 渲染器每 2 s 打印的帧率、每帧命令缓冲 GPU 时间、图元数、跳过的相同帧和平均重画面积。
- 运行时被测窗口会被提到最前并抢走键盘焦点。

场景：`idle` 空闲；`hidden4` 后台标签 4 个 spinner；`vis1` 前台 1 个 spinner；`vis4` 前台四分屏 spinner；`flood` 连续滚动输出；`mixed` 一个空闲 shell 加三个 spinner。spinner 模拟 Claude Code 的三行状态区，每 80 ms 原地重画。
