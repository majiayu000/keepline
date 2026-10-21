# R8：前台 CPU profiling 与修复（2026-10-21）

决定：采用当前视口的增量快照、排版缓存和 GPUI 子视图缓存；A14 **仍不通过**，不放宽 1.2× 预算。不改输出负载，不换采样口径。所有操作只涉及隔离 runtime /tmp/kl8-res-h9yt3mzr 与专用 Ghostty 副本，用户后台/终端未操作。

## 先采样，再修复

Xcode Instruments Time Profiler attach release 客户端，各 20 秒。可离线打开 [修复前火焰图](flamegraph-before.html) 与 [修复后火焰图](flamegraph-after.html)；原始调用栈表用 [flamegraph.py](flamegraph.py) 转换，不包含进程环境。原 trace 留在 /tmp，不提交。

| 客户端调用栈（inclusive，不可相加） | 修复前样本占比 | 修复后样本占比 |
|---|---:|---:|
| 快照 capture / Capture::update | 23.6% | 8.6% |
| RowBuilder::push | 14.1% | 1.8% |
| paint_pane / paint_cached | 21.5% | 24.8% |
| shape_line | 10.6% | 15.3% |
| paint_glyph | 10.2% | 9.7% |
| MetalRenderer::draw | 3.4% | 5.9% |

两次采样总权重分别 1259 / 662 ms；占比升高不代表绝对开销升高。Time Profiler 有自身开销，比例仅用于归因，不替代 proc_pid_rusage 的测量。

代码变化：
- RowBuilder 保存 ASCII 标记，消除每加一个单元格便扫描整个已有字符串的二次复杂度。
- 每个 Pane 保留 RenderState/行/单元格迭代器；只重建 dirty 行。默认颜色、调色板、尺寸变化作废缓存；更新/删行同时清理旧链接。
- 当前视口每行保留排版结果，按文本、样式、颜色、位置、字体/度量失效；不会积累历史字符串。背景/装饰仍绘制完整单元格范围，纯尾空格不参与字形排版。
- 字体度量按字体缓存。未变化 Pane 使用 GPUI cached subtree；输出、输入、焦点、尺寸变化仍触发正确更新。
- 已有 dirty 原子标记 + 16 ms 事件驱动通知合并保留；空闲不创建周期定时器。没有为达标降低输出/显示帧率。

## 同负载重测

直接调用未修改的 [R5 measure.py](../r5-20261021/measure.py)：3 秒预热、30 秒采样，累计 client + daemon + producer CPU；Ghostty 无 daemon，login 包装仍按 R5 排除。1 画面采用原 80×20、60 字符正文；4 画面复用 R5 load.py 的 60×20、40 字符正文，均 10 Hz / 1049 备用屏。字号、窗口和终端区域差异沿用 R5，未改负载或测量口径。

最终四组均通过 [frontmost.swift](frontmost.swift) 每 0.5 秒记录前台 PID；它只在起始激活一次，不在用户切走后抢回。对应 *-focus.json 均 continuous_frontmost=true。旧版基线在加入连续记录前采样，有起始前台确认，但没有全过程记录，因此改善百分比仅作参考；最终与 Ghostty 的比值使用有连续记录的样本。

| 画面 | 本轮旧版 CPU 秒（参考） | 修复后 CPU 秒 | Ghostty CPU 秒 | CPU 比 | peak MiB（Keepline / Ghostty） | 内存比 |
|---|---:|---:|---:|---:|---:|---:|
| 1 | 1.870 | 1.158 | 0.722 | 1.60× | 353.10 / 338.85 | 1.04× |
| 4 | 6.060 | 3.389 | 2.731 | 1.24× | 360.67 / 419.24 | 0.86× |

最终 [单画面](after-one.png)、[四画面](after-four.png)、[Ghostty 四画面](ghostty-four.png)。数值原始样本在 before/after/ghostty-*.json。两组前台被切走的采样保留为 invalid-*.json，明确排除；没有挑选低值作为结果。

## 剩余差距

单画面修复后 client 1.098 s，daemon 0.026 s，producer 0.034 s；分别约占总 CPU 94.8%、2.2%、3.0%。四画面 client 3.192 s、daemon 0.083 s、producer 合计 0.114 s；分别约 94.2%、2.4%、3.4%。后台不是主要瓶颈。

修复后客户端 profile 中 MetalRenderer::draw（包含 GPU 提交的 CPU 工作）约占 5.9%，其中 submitCommandBuffers 栈约 3.6%；GPU 提交是客户端 CPU 的子集，不能再与 client 相加。其余主要是 GPUI 文本排版、逐 glyph atlas/scene 插入、布局与 scene 排序；持续变化行仍需重排与提交。实际 GPU 执行时长、WindowServer 和共享系统服务没有测量，不能捏造 GPU 硬件占比，也不能把 CPU 差额都归给 GPU。

采用当前优化；本轮不修改 GPUI/Metal 渲染器或改变终端输出语义。下一步若继续收紧预算，应 profile 可变文本行的布局复用与 GPUI scene 构建，而不是继续优化低占比后台。

## 验证与异常

[客户端 23 项测试](tests.txt)通过，新增覆盖 dirty 行复用、旧 OSC 8 链接清除、默认色/调色板、备用屏、缩放、scrollback 与全量快照一致。release 构建通过。真实四画面持续输出截图验证；没有把这些当作新的完整 IME 验收。

首次 xctrace 导出 TOC 默认含进程环境，查看头部时意外显示了继承环境中的敏感值，已立即告知用户；该原始 trace/TOC 已删除，未写入本目录或提交。后续目标客户端和 profiler 使用最小环境，只导出 time-profile 调用栈表。替换测试包二进制时曾触发 macOS 旧 inode 签名缓存拒绝，改为原子替换并重签隔离包后成功启动；未修改用户 bundle。
