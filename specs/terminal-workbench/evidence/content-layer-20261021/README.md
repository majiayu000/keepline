# 会话内容层验收（2026-10-21）

决定：采用 Rust 确定性增量解析、有来源的卡片和默认 7 天外部会话。实现与下述本地验收完成；真实 CLI Hook 全链路、独立人工签收与长期功耗仍未验证。未调用模型、未修改 Codex/Claude 配置、未重启用户后台、未推送。

## 逐项验收

| 项目 | 结果 | 证据与边界 |
|---|---|---|
| Codex / Claude 用户原话、追加、助手回复、工具调用/结果/退出码 | 通过：规则单测、真实日志抽查 | [后台测试](backend-tests.log)、[解析复测](parser-retest.log)、[20 个卡片](cards-reviewed.md)；未知结构保留计数，不推测成功 |
| apply_patch / Edit / Write、git commit、测试摘要 | 通过：规则单测 | 成功回执关联调用；失败、未知退出、echo、读取旧历史不算成功。真实样本有 Write 回执，未覆盖所有真实 CLI 工具版本 |
| 回合、错误、PermissionRequest / Stop | 通过：合成单测/后台集成；部分真实日志 | 真实日志覆盖审批拒绝/认证错误；安装版 Hook 端到端未测 |
| 游标、半行、重启、轮转、事务回滚与幂等 | 通过：后台测试 | 内容与 offset 同一事务；重复事件不复制记录；尾段/单行有界 |
| 标题、原话、证据、最后回合、活动时间落库 | 通过 | SQLite session_content + session_records；CLI 标题优先，原话回退；派生投影有界 |
| 分组与四字段 | 通过：规则单测 + 本实现代理逐字段核对 | 用户结局优先；无证据不猜；助手自述明确标注，不进“做成了”；不是用户人工签收 |
| 近期外部会话只读可见 | 通过：真实日志离线导入 + 原生窗口 | 49 个近期身份，原生 fixture 取 40 个真实投影；不把日志存在当成已绑定 PTY |
| 继续外部会话 | 通过：真实原生操作 + 单测 | [截图](external-continue.png)显示新 shell 只预填 Claude --resume；未回车。Codex --no-daemon resume 与引用转义经 core 单测，本轮未实际启动两家 CLI |
| 状态栏摘要/分组、锚定卡片、鸟瞰、需要你 | 通过：Computer Use 原生窗口 | [鸟瞰](overview.png)、[工作视图卡片](workspace-card.png)；不是浏览器仿制图 |
| 两处存为待办并关联原会话 | 通过：原生操作 + SQL 读回 + 集成测试 | 鸟瞰保存真实外部 Claude 原话、弹窗保存模拟托管原话；[两条任务及关系](native-actions.json)。后台从记录取完整原话，不仅用 UI 截断片段；缺失历史有声明 |
| 40 外部 + 8 托管空闲资源 | 通过：30 秒原生测量 | [资源](idle-48.json)、[8 画面](eight-visible.png)；见下文，不是 8 个真实 Agent 同时工作 |
| 已关联日志追加不再启动 ps/lsof | 通过：4 个模拟 Agent 30 秒负载 | CPU 0.2723 秒、ps/lsof 新增均 0、增量读 87,780 B |
| FSEvents 缺口恢复不退出 | 通过：既有回归复跑 | MustScanSubDirs 有界补扫和根目录重建状态推送通过 |
| 长期功耗、同机 Ghostty 相对预算 | 未验证 | 不用短时空闲替代长时间测量 |

## 20 个真实会话抽查

只读本机日志，选择 3 个 Codex 和 17 个 Claude 会话，涵盖审批等待、认证错误、回合结束、运行/未知、转述注入及工具回执。逐条卡片、JSONL 路径、字节偏移、工具 call ID 与核对说明在 [cards-reviewed.md](cards-reviewed.md)；机器可读版本见 [cards-reviewed.json](cards-reviewed.json)。33 次辅助来源/调用关联检查。

**核对由本轮实现代理阅读原始记录完成，不是用户手测、独立人工审计或另外调用模型。** 自动脚本只检查来源定位，不能自动判定语义准确率。用户要求的独立人工签收没有发生，不能据此宣称已获得人工认可。

| 字段 | 有来源 / 明确未知 / 保守推断符合率 | 信息覆盖率 |
|---|---|---|
| 你要它 | 20/20（100%）；17 有原话、3 明确未知 | 17/20（85%） |
| 做成了 | 20/20（100%）；5 有成功回执、15 明确无已确认成果 | 5/20（25%）有机械成功证据，非真实成果召回率 |
| 停在 | 20/20（100%）；自述与回合事件分开标注 | 16/20（80%）末轮状态已知，4 未知 |
| 下一步·推断 | 20/20（100%）为保守建议、均明确推断 | 不评为已发生事实 |

没有发现助手自述混进“做成了”（0/20）。这些是所选样本的有源/保守表达符合率，**不是完整历史恢复率，也不是对所有日志格式的准确率保证**。首次抽查发现 app-open 注入、system-reminder 包裹的跨会话转述、side-session 通知和嵌套代码围栏中的完成语句误判；修复后重新生成并逐项核对了当前报告。复合 shell / JS 编排工具的成果可能漏记，保留未知，不为增加覆盖率猜测成果。

## 索引与资源

[冷启动记录](audit-metrics.json)：遍历 1,605 目录，打开 5,578 文件，读 153,445,621 B（146.34 MiB），耗时 **8.171 秒**，发现 49 个近期身份。每文件身份头最多 16 KiB，近期尾段最多 128 KiB（另读 1 字节判定行边界），没有读取 17 GB 全文。旧会话也需要首次身份头，因此总读量高于 49×128 KiB。

[同库再次启动](warm-index.json)：**0.047 秒，目录遍历/文件打开/读取字节全部 0**。只访问缓存 SQLite。本次性能试验用新的私有数据库做冷启动，不代表产品每次重扫。

40 个真实日志离线卡片 + 8 个模拟 Agent 的真实 PTY，8 画面同时可见，稳定后测 30.003 秒：

- 客户端 CPU **0.0781%**，物理 footprint **77.923 MiB**；后台 CPU **0%**，**7.110 MiB**。
- 客户端 + 后台 **85.033 MiB**；加 8 个 shell 合计 **95.363 MiB**。
- 所测进程磁盘读写全部 **0 B**；后台 ps/lsof、目录遍历、打开日志、读日志字节、索引写入计数增量均 **0**。
- CPU 按 mach_timebase_info 换算 proc_pid_rusage ticks，百分比以一个逻辑核为 100%；内存是 physical footprint，不用 RSS 冒充。
- 原生窗口逻辑尺寸 **1440×888**，截图 2×；系统可用高度导致不是严格 1440×900。测量起止窗口可见记录在 visibility-*.json；交叠的 126×126 区域来自 ChatGPT Computer Use 指针浮层（约 1.24%），未遮住整块画面，不假称全无遮挡。

负载测试第一次超原有 CPU 预算；定位到每条 native 通知各自提交 SQLite。现在仅在收到事件后固定收集 200ms 突发事件，再同事务批量处理（最多 64 批，截止时间不延长）。空闲仍无限阻塞等待，没有周期超时、轮询或降低 SQLite FULL 持久性。修复后原预算复测 CPU 0.2723 秒通过。

## 测试与复现

- [backend-tests.log](backend-tests.log)：core 13、daemon 单元 13、集成 15 通过；1 个 ignored 是测试子进程入口，不是跳过验收。含 40 PTY 5 秒空闲 CPU 0%、磁盘 0 B、Unix syscalls 0，和 4 Agent 30 秒负载。
- [parser-retest.log](parser-retest.log)：真实抽查修正后的 6 项解析测试通过；与上面的单元测试有重复，不相加。
- [client-tests.log](client-tests.log)：最终客户端 binary 13 项通过；其中当前工作树并行进行的终端渲染测试也被运行，不能把这些改动归给本阶段。
- [build-tail.log](build-tail.log)：原生 app 最后构建成功；GPUI 上游弃用 warning 仍存在。
- 真实安装 CLI/Hook、默认 Codex 守护模式关联、Claude 交互续接、Dock 重开、长时间功耗本轮未验证。

```sh
export ZIG=/Users/lifcc/Desktop/code/AI/tools/keepline/.claude/worktrees/terminal-eval-04a203/eval/terminal/vendor/zig/zig
export LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cargo test --manifest-path next/Cargo.toml -p keepline-core -p keeplined -- --test-threads=1 --nocapture
cargo test --manifest-path next/Cargo.toml -p keepline-app --bin keepline-app
cargo build --manifest-path next/Cargo.toml -p keepline-app
# 此 example 写私有目标库，只读用户日志。相同目标再次运行读取缓存。
cargo run --manifest-path next/Cargo.toml -p keeplined --example content_audit -- /tmp/kl-content-audit-reviewed-20261021 > /tmp/kl-content-audit-reviewed.json
python3 specs/terminal-workbench/evidence/content-layer-20261021/audit-cards.py /tmp/kl-content-audit-reviewed.json /tmp/kl-content-audit-reviewed-20261021
```

审计脚本中的样本 ID 是此次快照，随未来日志变化需重新选样；不能把内置核对说明用于不同样本。`native-check.py` 是本机测试驱动，依赖上述私有审计库和 `content_fixture` example；它复制缓存到独立 runtime、使用独立 HOME，生成 8 个明确标记的模拟 Agent。通过 Computer Use 操作、截图，measure-ready / finish 文件仅为测试编排信号，不进入产品。完成后只结束其自身客户端、后台和 PTY。本轮没有运行真实 Codex/Claude 命令，也没有改用户输入源或其他窗口。

原生继续入口曾在切离鸟瞰后、异步创建 Pane 之前访问缺失实体而崩溃；已修为等待 Launch/PutTab 成功并同步 Pane 后再切视图，最终构建经原生继续操作复验。保存待办的两个入口也已 SQL 读回（各一条，分别关联外部/托管会话）。

## 切换与未验收边界

本阶段 wire **major 3**。旧 major 1/2 宿主只接受一次探测并显示明确版本提示，不会自动重启。major 1 旧对象库不做兼容迁移；major 2 同结构对象表可以保留空间、待办和结局，新内容表另建。无论哪版，重启宿主都会中断现有 PTY，须由用户另行决定。用户当前后台保持原状，因此这里的完成指新构建与隔离验收，**不代表在用后台已经接入内容层**。
