# R1：并行测试与首次订阅空窗

## 决定与根因

采用资源窗口隔离，不改产品 CPU 预算。`Host` 持有测试进程内读写锁：常规真实 PTY 测试共享锁继续并行，40 空闲与 4 路写入测量独占。CPU 原本已只取被测后台 PID（按 Mach timebase 换算），没有把测试驱动 CPU 算进去；原样全量在本轮第一次通过，未独立复现用户报告的 CPU 超预算，不能断言具体是哪一次系统调度导致。代码确认原来所有后台启动/首次索引/40 PTY 创建与测量会重叠；现在隔离这些干扰，保留 ≤0.6 CPU 秒/30 秒、零重复 ps/lsof、>80 KiB 实际读取、游标追平与空闲业务 I/O 零增量断言。

**Claude 超时已复现且定位为产品竞态，不能只归咎 CPU 忙。** 仅加隔离和 30 秒 deadline 后仍失败：[原始诊断](failure.txt)。状态 Healthy，已关联，log_opens=2、log_bytes=788（两份首条原话），而文件已经追加到 734 字节；追加从未消费。首次 `bootstrap → association.apply` 后没有安装 vnode listener，只有下一批 FSEvents 才会安装。长持有写 FD 可推迟 FSEvents，形成等待空窗。修正为首次关联后、第一次阻塞接收事件前立即 follow，沿用“先注册再推进游标”封闭竞争，不加轮询、不加重复进程探测。

Claude 等待上限为 30 秒，超时打印识别状态、内容状态与文件大小；文件始终保持打开以真实覆盖 vnode 路径，不用 close 或重写触发器掩盖缺陷。常规功能测试仍并行，未将整个 cargo suite 改为单线程。

## 验证

使用 `ZIG=<既有 zig>`、`LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast`、完整 Xcode DEVELOPER_DIR，连续执行三次 `cargo test --workspace -- --nocapture`，没有设置 RUST_TEST_THREADS / --test-threads。逐轮输出见 run-1/2/3.txt。每轮 75 项通过（app 15、core 18、daemon 24、integration 18），仅子进程入口 daemon_fixture 按设计 ignored。

真实 CLI、通知及可见资源对照不由此自动通过；后续 R4–R6 补验。用户后台、终端、配置未操作。

实测三轮资源：

- 第 1 轮：40 idle PTYs: CPU 0.0000%, disk read 0 B, write 0 B, Unix syscalls 0; 4 agents / 30s: daemon CPU 0.4857s (1.566%), ps spawns 0, lsof spawns 0, incremental bytes 87932
- 第 2 轮：4 agents / 30s: daemon CPU 0.4603s (1.484%), ps spawns 0, lsof spawns 0, incremental bytes 87856; 40 idle PTYs: CPU 0.0000%, disk read 0 B, write 0 B, Unix syscalls 0
- 第 3 轮：40 idle PTYs: CPU 0.0013%, disk read 0 B, write 0 B, Unix syscalls 2; 4 agents / 30s: daemon CPU 0.4575s (1.476%), ps spawns 0, lsof spawns 0, incremental bytes 87856
