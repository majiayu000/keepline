# 订阅资源复验与修复记录

范围：本地 `feat/agent-workbench` 隔离工作树，未提交、未推送。测试日志全部在临时目录，
不读取用户真实 CLI 日志，不修改 CLI Hook 配置。主线工作区的原有改动保留。

## 复现与诊断

1. 单独运行原空闲测试得到 Unix syscall 0；与其他后台集成测试并行时复现 **+2**，
   原 `assert_eq!(0)` 失败。CPU 为 0.0000%，磁盘读写为 0 B。
2. `pti_syscalls_unix` 是整个进程的 Unix 系统调用总数，不能据此认定发生了日志轮询。
   无密码的系统级 DTrace 不可用（sudo 要求密码）；没有更改权限或系统设置。
3. 使用 LLDB 在隔离宿主上跟踪 `libsystem_kernel.dylib`。向监听祖先目录中的**无关路径**产生
   一个文件事件，捕获到 FSEvents `process_dir_events → kevent/fcntl/lstat`，
   `FSEventsD2F_server` 的 Mach 消息处理，以及两次 `__workq_kernreturn`。
   路径在应用回调中过滤后，仍可能有这些系统框架调用；它们不经过 Keepline 日志/目录扫描代码。
   另看到 libtest 超过 60 秒时输出耗时提示，测试宿主也不是完全无系统调用的进程。
4. **边界**：这次控制实验解释了为何不能要求总调用数恒为零，并未逐条追溯用户历史运行里的那两次调用。
   不把线程回收或某个具体 syscall 写成那次 +2 的已确定原因。
5. 改为总调用增量 0–4（最多两对小量原生唤醒），并独立断言日志打开/读取、目录遍历、
   索引写入、ps/lsof 启动计数全部不变。CPU <0.5%、磁盘读写为零的门槛保留。
   容差只用于全局观测噪声；业务 I/O 不放宽。

## 修复决策

- 已关联日志只推进游标、更新已存在会话时间；不在每个追加事件上读取整个身份索引或启动 ps/lsof。
- 新身份/未解决的托管关联才进入识别路径；外部日志尝试后不随每行追加反复探测。
- FSEvents 用于目录发现和历史重放；已关联文件额外使用阻塞式 kqueue vnode 通知。
  这是负载测试发现保持写描述符打开时 FSEvents 追加通知延后的修复，避免仅靠改测试写法通过。
- 队列溢出记录丢失路径的父目录；缺口按事件范围补扫。范围持久化，按目录项分步推进；
  正常启动不全扫，异常恢复也不扫描未受影响的旁支。
- 根移动/重建后重建监听，补扫结束后继续处理；缺口内丢失的缓存路径清理，不改用户业务结局。
- 识别状态单独通过 Unix socket 推送；客户端显示中断/补扫状态。没有客户端状态轮询。

## 预算与结果

本轮最终结果：**21 项测试通过**（core 7、后台单测 6、后台集成 8；另有 1 个仅用作子进程入口的 ignored fixture）。
Clippy `-D warnings` 和 GPUI 客户端编译通过；客户端依赖仍有既存的 GPUI 弃用警告。
[architecture.md](../architecture.md) 保留逐项验收映射，不把单次通过描述成长期功耗验收。

| 场景 | 本轮测量 | 判定 |
|---|---|---|
| 40 会话空闲 5 秒 | CPU 0.0000%；磁盘读/写 0 B；Unix syscalls +2；业务计数未增长 | 通过修订后的门槛 |
| 4 Agent 混合追加 30 秒 | 后台累计 CPU 0.0148 秒（0.048%）；ps 0 次，lsof 0 次；增量读取 87,856 B；四个游标均到 EOF | 通过 |
| 缺口及根重建 | 溢出、指定子树、状态流、原生根重建与旧索引清理测试通过 | 通过 |

本机原始输出：`/tmp/kl-review-tests.log`、`/tmp/kl-review-clippy.log`、`/tmp/kl-review-app.log`。
LLDB 控制实验原始调用栈：`/tmp/kl-syscall-trace-results.json`（未包含用户真实日志内容）。

- 空闲：40 会话，5 秒，CPU <0.5%，磁盘增量 0，Unix syscall 0–4，业务计数不增长。
- 负载：4 Agent × 约 10 行/秒 × 30 秒；两条保持写句柄打开，两条逐行开关。
  累计后台 CPU <0.6 秒；暖机后 ps/lsof 启动增量 0；增量读取 >80 KB；四个 offset 均追到 EOF。
- 缺口：溢出后继续接收、只重扫目标子树、状态推送顺序、原生根目录重建后继续增量读取。
- 真实官方 CLI/Hook、原生提示人工验收及长期整机功耗：未完成。

复验命令（在 `next/`，先设置与 bootstrap 锁定版本匹配的 ZIG）：

```sh
LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast cargo test -p keepline-core -p keeplined -- --nocapture
LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast cargo clippy -p keepline-core -p keeplined --all-targets -- -D warnings
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast cargo check -p keepline-app
```

编译检查不能替代原生状态提示人工验收；短时负载不能替代长期整机功耗。

## 2026-10-20 CPU 单位勘误

本记录原 CPU 数值按纳秒处理了 Mach ticks，在本机 Apple Silicon 上低估约 41.7 倍，撤回这些 CPU 预算结论。
I/O 计数、ps/lsof 次数和事件恢复证据不受此计量错误影响。最新正确单位下的测试、失败记录与修正见 [分屏复验的 CPU 勘误](split-tree-20261020/cpu-units-correction.md)。
