# R11：可变 ASCII 文本排版缓存与前台审计

决定：采用有界 ASCII 原生字形缓存；A14 **仍不通过，新的倍率未验证**。不修改负载、采样脚本或 1.2× 门槛。

## 实现与正确性

R8 修复后火焰图中 shape_line 占客户端采样 15.3%，paint_glyph 9.7%。本轮按字体/样式缓存 95 个可打印 ASCII glyph ID 和 baseline，只有原生排版证明每字一格且无上下偏移才使用。关闭 liga/calt，保持终端 ASCII 单元格独立；CJK、emoji、组合字符及非等宽字体继续完整排版。缓存仅四种样式，字体/单元格度量变化即失效，不保存历史输出。没有降低帧率或改变 PTY 消费。

- app 24 项测试通过，见 tests.txt；release 构建通过。
- 原生 visual-test 检查四种字重/斜体的 glyph ID、字体、位置、baseline 与完整 shaper 一致；IME UTF-16、宽字符与符号字体检查通过。
- 13 个 fixture 用同一 Menlo 12.5、100×30 网格重画；12 个与旧版在既有 >32 像素阈值下完全一致，box-blocks 仅 15/1355400 像素变化。和已有 Ghostty 截图比较均不增大差距，见 render-comparison.json。Ghostty 图来自已有对照，未声称本轮重拍。

## 前台复测

原封不动调用 R5 measure.py，1 画面 R8 load-one.py，4 画面 R5 load.py；3 秒预热、30 秒采样，计入客户端/后台/生产者。隔离 runtime `/tmp/kl8-res-jn0qbbqp`，用户后台未操作。

frontmost.swift 监听 NSWorkspace 激活事件并每 100 ms 记录 PID、bundle、应用名、UTC 与相对时间。每次仅开始时激活测试窗口；失焦自动作废、重采，最多三次，全部结果保留。

1/4 画面各三次均失焦，所有 `keepline-{1,4}-*.json` 性能值均无效，不生成新倍率；没有拿后台低值替代前台结果。因此不再启动无可配对的 Ghostty 新采样。A14 仍引用 R8 的最后有效 1.60×/1.24×。

观察到前台切入 Claude、Lark、Lark Helper、Chrome、QuotaBar、Rekey 等，详见 focus JSON。RustDesk 与 SkyComputerUseService 虽在运行，本轮未作为前台切入者出现，不能据此认定它们抢焦点。系统 runningboardd 记录只显示 launchservicesd/WindowServer 中转，不能确定原始激活请求者；发起进程归因未验证，未停止任何相关应用。

## 修复后调用栈

[Time Profiler 火焰图](flamegraph-after.html)记录四画面 20 秒输出。5563 个有效 1 ms CPU 样本，另 2 条缺栈采样未计入图。shape_line 未出现在采样栈；paint_glyph 约 20.49%、paint_cached 20.24%、MetalRenderer::draw 2.35%（inclusive，不可相加）。当前热路径转为 glyph atlas/scene 提交。该 profile 没有连续前台保障，只用于找热点，不与 R8 样本权重作性能倍率比较，也不推算 GPU 实际执行时间。

## 剩余限制

新实现有原生排版/像素回归证据，缺连续前台的有效资源样本；不声称 CPU 已改善或达标。后续采样仍使用同一自动审计脚本，不能放宽连续前台条件。
