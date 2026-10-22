# R10 高负载 RPC 与资源测试稳定性

决定：修复 socket 就绪发布竞态，保留单次 RPC、不重试。用同时运行的空闲 daemon 作 CPU 对照，采样期不包含 attach/断开，负载越高只延长墙钟窗口，2% 单核上限不变。

## 根因与修复

在修改前、八个 `/usr/bin/yes` 独立进程持续运行时，全量 workspace 在另一个调用同一 workspace() 辅助函数的测试复现 **ConnectionRefused / OS error 61**，[原始失败](baseline-failure.txt)。这与用户只保留 panic 行的错误位置一致，但不声称已拿到用户那次错误全文。该基线中四 Agent CPU 测试通过，用户的 CPU 断言失败本次基线未复现。

`UnixListener::bind` 内部先 bind 再 listen；bind 已创建可见路径，线程此时被调度走，测试看到文件就连接会被拒绝。此前还在 socket 发布后才做 Store/Index 初始化，连接可能在后台未就绪时排队。现在先初始化，在私有 `.sock-start` 上完成 bind/listen 和权限，再原子 rename 为 `workbench.sock`。全过程持有原有 host.lock，未新增 retry、RPC 超时放宽或重放路径。RPC 错误补上 connect / hello / request / reply 阶段，保留 io::ErrorKind，便于区分将来的真实错误。

回归测试用 SQLite 独占事务阻挡启动，确认初始化期间正式 socket 不存在；放开屏障后正式路径出现，首次 RPC 必须成功，无连接重试。

## CPU 门槛

- 原测量把四次 attach/快照、断开及其后台清理算入 30 秒日志追加成本；现在预先建立连接并保持到采样结束。
- 同期启动空闲隔离 daemon，仅比较两个明确 PID 的 proc_pid_rusage，按本机 Mach timebase 换算。扣除空闲对照，且对照自身必须 ≤0.2% 单核，否则不准扣掉污染值。
- 窗口为 30 × ceil(load average / logical CPU)，限制 30–90 秒；四 writers 每个仍 10 Hz，持续写满整个窗口，不减少数据速率。增量 CPU ≤2% 单核；最小读量随窗口同比增长，并逐一检查持久游标等于文件大小、ps/lsof 增量严格为 0。
- 窗口后的完成等待额外最多 30 秒，不把额外等待时间加入预算分母。功能测试仍并行，资源测试继续独占已有夹具锁；没有产品轮询。

## 验证

三轮默认并行 `cargo test --workspace -- --nocapture` 在八个 yes 全程存活下执行。每轮负载、耗时、退出码见 [runs.json](runs.json)，执行输出见 run-*.txt。与 load≈80 的用户环境不同，本轮记录实际 load，不伪称等于 80。未读取或重启旧 keepline-service，也未操作用户工作台后台。

结果：**连续三轮各 90 passed，0 failed**，另 1 ignored 为 daemon fixture 子进程入口。八个 yes 已逐 PID 核实后停止。

```text
4 agents / 90s (load [32.7568359375], 16 cores): daemon CPU 0.7772s, idle control 0.0001s, incremental 0.863%, ps spawns 0, lsof spawns 0, incremental bytes 263416
4 agents / 60s (load [22.08935546875], 16 cores): daemon CPU 0.4406s, idle control 0.0001s, incremental 0.734%, ps spawns 0, lsof spawns 0, incremental bytes 175560
4 agents / 60s (load [19.52783203125], 16 cores): daemon CPU 0.4875s, idle control 0.0001s, incremental 0.812%, ps spawns 0, lsof spawns 0, incremental bytes 175636
```
