# CPU 计量纠正

这台 Apple M3 Max（macOS 26.5.1）的 `proc_pid_rusage` CPU 字段是 Mach ticks。
不能直接把 `ri_user_time + ri_system_time` 当纳秒；`mach_timebase_info` 为 125/3。
实际对照见 [cpu-units-check.json](cpu-units-check.json)：Python 进程 CPU 时钟增加 0.591653 秒，rusage 原始差值 14,201,260，换算后 0.591719 秒。

本轮最初给出的 8 画面空闲约 0.003%、输出约 0.49% 撤回，均低估了 CPU。
此前 resource-recheck 中使用相同换算的 CPU 数字也不能继续作为预算证据；日志 I/O / 探测次数结论独立于 CPU 单位。

修正现有测试后，首次完整运行的 4 Agent / 30 秒测得 0.722479875 秒（2.330%），超过原预算 0.6 秒，明确失败。
没有调高预算。随后复用 SQLite 热点预编译语句、跳过未变化的检查点写入、合并已经排队的文件事件（不等待、不引入定时器），同一测试测得 0.5358 秒（1.728%），ps/lsof 启动增量均为 0，读取增量 87,932 字节。
最新完整运行见 [tests.log](tests.log)。这只代表本机本次短测；不是长时间功耗或相对 Ghostty 预算验收。
