# Progress Ledger 实现与修复验收

Issue: https://github.com/majiayu000/keepline/issues/138

## 总览时间范围

- 新增“最近 N 小时内有活动”筛选，默认 24 小时，支持输入 7 小时及保留期内任意正整数小时，刷新后保留选择。全部布局和计数使用同一个范围；切换时丢弃旧范围的延迟响应与悬停队列。
- 卡片顶部显示最后活动的相对时间，悬停显示完整日期；详情中的总时间改标“会话跨度”，避免误读成最后活动或连续运行时长。“今天已结束”改为“已结束”。
- 14 项相关单元测试、4 项浏览器回归通过，包含 40 小时前开始但 2 小时前仍活跃、7/24 小时切换、刷新保留、无效输入及窄屏。客户端与测试类型检查、客户端构建及根目录类型检查通过。
- 真实服务只读核对：7 小时 42 个父会话、24 小时 61 个父会话，均无窗口外记录；实际页面切换和刷新保留通过，无浏览器运行错误。记录：`/tmp/keepline-time-range-live-result.json`；截图：`/tmp/keepline-time-range-fixed.png`。数量仅代表验证时的数据快照。

验收日期：2026-10-06。范围包括 T1–T15，保留三个原有 spec 的修改。实现整理至功能分支 `feat/progress-ledger`；未推送或安装用户 hook。

## 2026-10-07：修复“需要你”漏收

- 3377 原本运行 `/tmp/keepline-ui-live.ts`，仅挂载 UI 路由，没有监控或 hook 接收器；会话数据库最后活动停在前一天。已切回本仓库 `service`，完成首次同步并恢复周期扫描。
- 为 Claude Code / Codex 安装 Keepline hook，保留原有处理器；原配置及数据库备份在 `/tmp/keepline-needs-input-backup-20261007-224250`。真实 Claude hook 已收到；Codex hook 信任与实际事件交付仍需区分。
- 参考 Herdr 的 `tab_attention_priority`：阻塞与未查看的回复都应进入待处理。普通已回复会话不再因没有确认清单而直接进入历史；打开查看记录当前轮次已读，下一轮回复重新进入队列。查看不生成验收，也不清除审批等待。沿用 12 小时过期规则。
- 未回答的结构化 `AskUserQuestion` / `request_user_input` 从日志识别，不依赖安装 hook；收到工具结果后清除。前后端统一待处理判定，全部证据齐全的待验收会话也计入“需要你”。
- 最终 153 项相关回归通过，涵盖 hook 问答、缓存已读、审批优先级及架构依赖。两个浏览器用例验证普通新回复入口及已有验收流程，类型检查和完整构建通过。日志：`/tmp/keepline-needs-input-final-tests.log`、`/tmp/keepline-needs-input-build.log`。
- 重启后的真实接口返回 57 个父会话，37 条未读回复进入“需要你”；页面可见 37 个“查看回复”入口。证据：`/tmp/keepline-needs-input-live-result.json`、`/tmp/keepline-needs-input-fixed.png`。

以下第五轮中的“原话模式会话归入已结束”由本轮未读规则替代，候选项不计进度及默认安静提醒保持原契约。

## 第五轮：默认执行记录与安静提醒

- 默认仅审批请求发送系统通知；偏离、缺证据、停止、额度限制保留面板信息，由设置单独开启通知。子会话审批合并到父会话，通知使用父会话标题。
- 未确认的整段原话只作为候选，不参与进度、偏离、待核对及完成说法提醒。验收清单、AI 识别及用户确认项参与判定；新补充的待做要求可由成功检查自动推进，不固定为用户覆盖状态。
- 没有确认要求时，总览和详情展示最近工具动作、最近证据与 agent 最后汇报。完成本轮的原话模式会话归入已结束；已确认要求的待核对满 12 小时也归入已结束，不自动验收。嵌套子会话合并至父会话，并可从详情查看。
- 默认进度账、设置、菜单栏和通知文案改为中文。保留现有视觉主题。修复刷新时详情被卸载、行点击后整页刷新丢失选择；去掉字体加载的内联事件，保持严格 CSP。
- 最新 `bun test` 为 609 pass / 0 fail（85 文件），Rust 为 14 pass / 0 fail。两个新增回归覆盖手动补充要求的证据推进和缺标题时使用原话。类型检查通过。 最终 Tauri `.app` / `.dmg` 构建及严格签名校验通过；包内 sidecar + 浏览器验证行点击、整页刷新保持详情、要求删除/补充持久化、导入及建议待办保存流程，JS / CSP 错误均为 0。实际 3377 服务页面也验证行点击、子任务展开与数据刷新保持详情。

真实记录只读、隔离数据库和配置，首次扫描后监测 300.2 秒；复用派生缓存，热启动枚举 8.748 秒，不作为冷启动结果。43 轮增量扫描平均 1.998 秒（1.522–4.197 秒）；扫描 CPU 累计 72 秒，折合单核 24%。进程树 CPU 采样中位数 0%、均值 18.72%；RSS 中位数 75.2 MiB、末值 75.3 MiB、峰值 461.3 MiB。仍在追加的大日志会重读；平均 CPU 尚未达到接近零，不能以空闲中位数代替平均性能结论。本轮未改扫描调度。

真实总览抽查为 78 个父会话、101 个折叠子会话：5 个运行中、73 个已结束；0 个 Unknown task、0 个误判偏离、0 个原话进度条或待核对。77 个有实际活动、44 个有证据。隔离目录未安装 hook，不能用这个审批数证明真实 hook 已接入；审批合并与提醒由回归覆盖。

证据：`/tmp/keepline-round5-all-tests.log`、`/tmp/keepline-round5-final-typecheck.log`、`/tmp/keepline-round5-native-tests.log`、`/tmp/keepline-round5-results.json`、`/tmp/keepline-round5-samples.json`、`/tmp/keepline-round5-service.log`、`/tmp/keepline-round5-real-ui-results.json`、`/tmp/keepline-round5-browser-final.log`、`/tmp/keepline-round5-live-browser.log`、`/tmp/keepline-round5-final-package-build.log`。原始用户内容没有复制进仓库。未调用真实模型，未安装 hook，未开启登录自启或修改系统通知权限。

## 第四轮：宿主标签与问句收尾

- 开头的宿主标签块按标签结构剥离，不依赖已知标签名单；支持连续、嵌套及自闭合块，只移除开头上下文，保留其后用户文字。Claude 斜杠命令的连续输出块也排除。摘要、事实及已计算账本的派生缓存版本同步更新，避免旧提取结果继续命中。
- “是在网页还是 app 呢”“你看看这是什么呢”“你觉得应该怎么做呢”等保留为问题，不生成默认进度项；要求与问题混合的消息继续保留候选。未标记、没有明确汇报结构的粘贴文字仍按候选保留，没有继续扩大猜测性过滤。
- AI 关闭且未丢弃的 fallback 候选超过 8 项时，详情顶部提示开启识别或手动删减。包内 sidecar + 浏览器验证 8 项不提示、9 项提示，问题不计入这个数量。
- 只读对照前轮真实库的 197 条宿主标签消息：89 条纯上下文排除，108 条尾部原话保留且内容逐条一致。新库开头标签要求为 0；两条此前漏掉的 Claude / Codex 核心原话仍在。保存 1137 条问题，其中 534 条以“呢”结束；问题默认进度项为 0，fallback 自动完成为 0。
- `bun test`：599 pass / 0 fail，84 个文件；类型检查、服务构建、Tauri `.app` / `.dmg` 构建、14 个 Rust 测试及签名完整性检查通过。浏览器验证宿主标签、尾部原话、问句、提示阈值和已有纠正流程；JS / CSP 错误均为 0。测试包含 hook 离线并发与拒收隔离回归。

按 app 的五秒间隔在隔离数据目录、真实记录只读运行，首轮完成后持续监测 300.6 秒。新派生缓存 0 命中 / 1369 miss，首轮扫描 25.591 秒（系统文件缓存未清空；不能作为前轮冷启动的同条件加速对比）。43 轮增量扫描平均 1.921 秒，范围 1.513–2.623 秒。服务进程树 CPU 采样中位数 0.1%、均值 19.41%；扫描子进程累计 CPU 75.35 秒，折合单核 25.12%。RSS 中位数 88.4 MiB、末值 87.3 MiB、含扫描子进程峰值 491.3 MiB。扫描间隙基本空闲，但本轮平均开销高于第三轮，运行中的日志持续追加时仍会重新解析；不把中位数当平均值，也未证明所有工作负载下增量小于一秒。监测结束后仅关闭本轮测试服务。

监测证据：`/tmp/keepline-round4-monitor.py`、`/tmp/keepline-round4-home`、`/tmp/keepline-round4-results.json`、`/tmp/keepline-round4-samples.json`、`/tmp/keepline-round4-service.log`。

证据：`/tmp/keepline-round4-all-tests.log`、`/tmp/keepline-round4-typecheck.log`、`/tmp/keepline-round4-native-build.log`、`/tmp/keepline-round4-native-tests.log`、`/tmp/keepline-round4-browser.log`、`/tmp/keepline-round4-real-assertions.json`。真实内容未复制进仓库，未调用真实模型。通知点击、登录自启及“停止监控”原生按钮仍未人工验收。

## 第三轮：候选要求优先

产品决定按本轮审计建议执行：不再用行动词白名单决定一条用户消息有没有资格成为要求。默认保留候选，排除明确的系统注入、引用/粘贴块、格式明确的长完成汇报、纯命令/代码、短状态追问与批准。混合消息保留整段原文用于展示，引用/代码之外的 `authoredText` 用于匹配和模型输入。无明确边界的内容仍可能是候选，需要用户纠正或开启识别；没有声称规则能判断任意自然语言的真实意图。

- “我想做一个 agent 任务板”“我要求功能和性能都比他们好”等不含原白名单词的意图保留。实际 Claude / Codex 会话中此前漏掉的两条核心原话已从原始事实对应到新库的要求记录。
- 问题保留在原话列表，标记“问题 · 不计进度”，默认不生成要求项。混合了要求和问题的多段消息仍保留为候选，避免整段丢失。真实库保存 744 条问题，0 条问题生成默认进度项；纯 `bun test` / `git status` / `bun run typecheck` 要求项为 0，系统注入要求为 0，fallback 自动完成为 0。
- AI 默认关闭，开启后只识别/拆分最新候选用户消息。先按 `(agent_session_id, ask_id)` 原子落库领取，再调用提供者，每条消息最多一次；结果跨进程、扫描与重启复用。工具、agent 汇报和完成声明不触发调用，也不发送给模型。先前未处理的候选保留 fallback，不追补历史模型调用。失败保留候选且不自动重试，用户可显式重新识别。
- 模型返回的状态、完成声明和证据字段不进入状态判定；完成仍由实际命令/测试证据匹配。界面明确说明 AI 用途，去掉了旧间隔/积压步骤设置。
- 手动删除或补充后，重扫、重启、AI 开关和显式重新拆分不恢复被删除项。复用原有候选记录保存删除意图；未改变用户原话。浏览器实际验证补充、删除、重新拆分及刷新后保持修改。
- spool 的 30 个并发 hook 总计时从 3 秒放宽到 15 秒，该测试超时设为 20 秒；30 个落盘、永久拒收隔离、29 个重放与重复不投递的断言保留。没有放宽生产 hook 或提醒时限。

新一轮在新的隔离数据库验证，复用前轮只读扫描生成的摘要/事实缓存。尚未发布的 migration 015 同步更新问题类型、原文/匹配文本及识别结果结构；用户数据库只读检查尚无账本表，未修改用户库。真实模型语义准确率未做付费提供者验收；自动化使用受控返回验证调用时机、缓存、约束和失败行为。

最终扫描逻辑按 `service --scan-interval 5` 在真实记录上持续 300.3 秒；只采样服务自己的进程树，没有账本 HTTP 详情请求。热摘要/事实缓存下首次枚举并重算约 8.4 秒（不是空缓存首次解析结果）。之后 51 轮增量扫描平均 0.844 秒、范围 0.524–1.452 秒；进程树 CPU 采样均值 5.95%、中位数 0%，扫描子进程累计 CPU 28.42 秒，折合单核约 9.47%。RSS 中位数与最后值均为 95.1 MiB，含子进程的峰值 365.1 MiB。运行中的日志仍在变化，有短暂 CPU / 内存尖峰；没有声称平均 CPU 为零，也未将服务监测等同于实际 WebView 长期运行验收。

监测入口：`/tmp/keepline-round3-monitor.py`；隔离目录指针：`/tmp/keepline-round3-home`；结果及完整样本：`/tmp/keepline-round3-results.json`、`/tmp/keepline-round3-samples.json`；扫描日志：`/tmp/keepline-round3-service.log`。本轮测试服务均已关闭。

证据：`/tmp/keepline-round3-all-tests.log`、`/tmp/keepline-round3-typecheck.log`、`/tmp/keepline-round3-native-build.log`、`/tmp/keepline-round3-browser.log`；真实原话及问题检查为 `/tmp/keepline-round3-real-assert.ts`、`/tmp/keepline-round3-real-assertions.json`。系统通知点击及登录自启边界保持本文件末尾记录。

## 第二轮真实数据审计修复（历史，要求提取已由第三轮替代）

- 系统注入的 `<codex_internal_context>`、`<send_user_message_question_reply>`、`<task-notification>` 不作为用户要求。Claude 通知也不创建新的用户轮次。批准、状态追问、普通提问和没有明确行动指令的闲聊不生成要求项；含“修复”的追问也有回归覆盖。规则识别保持保守，含糊输入可通过用户编辑或明确 checklist 确认。
- `fallback` 原话不凭关键词自动完成。规则只给明确匹配检查命令、调用成功且同次调用没有失败测试的确认要求项完成证据。`14 passed; 0 failed` 可作为成功结果，`3 passed; 1 failed` 阻止完成；读取日志的 Bash/Codex 命令不产生执行证据。
- 未变化会话跨扫描子进程复用已计算进度账，不加载事实或重写要求/证据；文件指纹、用户修正、checklist、归属、验收及规则配置变化使缓存失效。运行状态和 goal 状态单独刷新，不触发内容重算。变化文件在摘要扫描的同一趟解码中生成事实磁盘缓存，数组写入后立即释放；账本不再重复解析原始 JSONL，Codex / Claude 的实际文件追加回归验证二次事实读取次数不增长。缓存沿用摘要派生数据数据库，按保留窗口清理，原始数据与用户确认仍由原有库保存。移除逐文件执行的全缓存过期清理，消除多处逐步遍历全量事实/证据的开销。
- 导入候选在打开详情时刷新，避免新会话先被扫描时永久缓存空候选；回归按反向扫描顺序复现，修复前失败，修复后无需重读事实即可找到先前会话。
- Claude 桌面 `Application Support` 路径正确解析，保留 `--resume=<id>` / `--session-id <id>`；先按实际会话 ID 绑定，带明确 ID 但不在扫描时间窗内的进程不再误配历史会话。Codex 桌面自定义标题读取 `threads.name`，过滤内部上下文标题；真实 rekey 会话标题已复验为“发版并冻结格式”。
- `find -delete` 等删除命令在工具名为 `find` 时也计为修改步骤；hook 通过 `env python3` 解析解释器。离线 30 个并发 hook 均成功落盘，一条永久拒收事件进入隔离目录，其余 29 条正常重放，重复重放不再次投递。

## 上一轮核心通路修复

- 摘要结果不再持有 transcript facts。启用账本时，全量同步按账本保留期扫描（默认 30 天），旧会话数据库记录保留；排除项不补读进度事实。事实按记录时间过滤，复用派生数据磁盘缓存；内存事实缓存按序列化大小限制为 16 MiB，扫描器不持有所有会话的事实数组。大文件解析结束后回收 Bun 的临时分配，输出摘录独立复制，避免引用完整大输出。
- Claude 的正常 Bash 结果可用 `is_error` 识别成功与失败；后台启动不算完成。读取已有测试日志、TodoWrite 等折叠调用不产生执行证据。
- Codex 解析 `item_completed` / `CommandExecution` 的结构化退出码。结构化码优先；仅有包装器文本时，任一非零码阻止整个调用被判为成功。
- Hook 每个事件独立 fsync 并原子落盘，不抢共享锁、不因锁超时退出而丢事件。重放继续处理后续文件；临时失败保留重试，格式错误或永久拒收记录保存在 `spool/rejected/`。真实 HTTP 413 后的有效权限事件已通过回归。
- Goal 元数据只读查询真实 `thread_goals` / `updated_at_ms`；受阻和额度用尽显示停止原因及提醒。
- 常见 Codex 元记录不再算未知格式。模型只能用成功且属于同一轮的证据支持完成说法。sleep、TodoWrite 折叠；`find -delete/-exec/-fprint` 等计入有效步骤。
- 原话可提取公共 API、禁改路径、保留标签约束；agent 最终报告的明确后续建议可预填 todo，保存前不创建；含糊要求可手动从同项目、同 runtime 的先前会话导入要求，并清空旧完成证据。
- 原生命令加入 Tauri app manifest；远程权限只给配置的服务端口与五个账本命令，不授予 process/opener 插件权限。HTML 和应用设置 CSP；保留现有 Google 字体的两个明确域名。

## 完成的检查

| 检查 | 结果 |
| --- | --- |
| `bun test` | 599 pass，0 fail；84 个文件 |
| `cargo test --manifest-path menubar-tauri/src-tauri/Cargo.toml --lib` | 14 pass，0 fail |
| `bun run typecheck` | CLI / 服务与 React 客户端通过 |
| `bun run build:server` | 最终 CLI / 服务构建通过；客户端在下项 Tauri 构建中重建 |
| `cd menubar-tauri && bun run tauri build` | React production build、`.app` 与 `.dmg` 构建通过 |
| `codesign --verify --deep --strict …/Keepline.app` | 通过 |
| 签名包内 sidecar 实际启动 | 隔离 HOME 启动、首次同步、认证后账本 API、CSP 响应均通过 |
| 浏览器实际 UI | 核心意图保留、问题不计进度、复制汇报排除、删除/补充及重新拆分后刷新保持修改；确认 checklist 后 Claude 隐式成功、导入要求、建议待办保存前不创建及保存到同一 goal 通过；0 个 JS / CSP 错误 |
| 模型关闭 | 同时拦截网络、`Bun.spawn`、`Bun.spawnSync`；提供者与子进程调用均为 0 |
| 提醒 | hook 到队列的计时及五秒轮询预算、重复偏离只提醒一次、撤回、focus、native 队列通过 |
| 原生进程及权限 | 实际子进程验证保留 / 停止自有服务，外部进程不受影响；真实 Tauri ACL 拒绝其他 loopback 端口及未授权命令 |
| 通知目标 | URL 同源、session 编码、section/default anchor 测试通过 |

包内 sidecar 的网页探针使用 Tauri 实际传入的 `KEEPLINE_WEB_DIST=…/Keepline.app/Contents/Resources/web`；省略它时只能验证接口，不能验证包内网页。

端口占用测试此前依赖真实 LiteLLM 价目表下载，远端等待超过其 5 秒上限；该测试现固定无关价目表响应，继续验证占用端口不会修改会话状态。未改变生产价目表逻辑。

## 第二轮只读常驻验证

使用 `service --scan-interval 5`，原始 Codex / Claude 日志及 Codex 元数据库只读，数据库、配置和派生缓存均在隔离临时目录。禁用模型与系统通知。首次同步后持续采样五分钟：每秒用 `ps` 采集该服务及其所有子进程的 CPU / RSS，每约十秒输出一次；结束后自动停止本轮服务。扫描耗时来自每轮完成日志，另记录扫描子进程的累计 CPU 时间，避免只凭稀疏采样漏掉短尖峰。

本轮在热缓存上验证增量扫描；启动日志的 `full:true` 表示全量枚举，不表示空缓存。最终五分钟验证不作为首次空缓存扫描的性能结论。真实会话仍在追加记录，因此“摘要缓存命中大部分”与“所有会话完全静止”不同。

最终代码测量结果（首轮后持续 300.3 秒，包含下述一次账本 HTTP 读取）：

| 指标 | 结果 |
| --- | --- |
| 热缓存启动全量枚举 | 2.416 秒；摘要缓存 1,357 hit / 6 miss |
| 52 轮增量扫描 | 平均 0.767 秒；最小 0.531 秒，最大 1.346 秒 |
| 扫描子进程累计 CPU | 25.19 秒，折合单核约 8.4% |
| 整个服务进程树 `ps` CPU 采样 | 平均 5.77%，中位数 0.1%；短尖峰仍存在 |
| 进程树 RSS | 中位数 325.0 MiB，峰值 607.9 MiB（包含扫描子进程）；最后 328.7 MiB |
| 账本 HTTP 读取后的主进程 | 约 316–329 MiB；五分钟内小幅增长，未观察到前轮 600 MiB–1.5 GiB 的剧烈波动 |

短扫描容易被 `ps` 采样漏掉，因此不能把采样中位数接近 0 当成总 CPU 为 0；上述扫描累计 CPU 单列给出。本轮测试服务已自动停止，5572 / 5574 / 5576 / 5578 的监听已清除，原有 3377 服务未操作。

真实数据语义复查：系统注入要求项为 0，“现在呢 / 这个库还有什么没做的吗”独立要求项为 0，fallback 自动完成项为 0。16 个实际 Claude 桌面会话按命令行 ID 绑定存活进程；历史会话仍可能是 lost。rekey 会话标题为“发版并冻结格式”。这些检查只读查询本轮隔离库，不修改用户数据库。

另做同一分支、同一缓存的短时 ledger 开 / 关对照：完全未变化的一轮分别消耗约 0.185 / 0.130 秒扫描 CPU，差约 0.055 秒。这只是两轮静止观察，不当作整个真实工作负载的平均 CPU。菜单栏使用的 `/api/ledger?hours=24` 另做认证请求验证：195 个会话、响应约 16.7 MB，耗时 0.355 秒；该请求包含完整账本详情，测量后 HTTP 进程的 RSS 高于只扫描的空闲进程。未声称已测实际 WebView 常驻 CPU。

复现入口与证据：

- `/tmp/keepline-round2-finalcheck-monitor.py`、`/tmp/keepline-round2-finalcheck-home`（隔离目录指针）
- `/tmp/keepline-round2-finalcheck-monitor.log`、`/tmp/keepline-round2-finalcheck-service.log`
- `/tmp/keepline-round2-finalcheck-results.json`、`/tmp/keepline-round2-finalcheck-samples.json`
- `/tmp/keepline-round2-real-assertions.log`、`/tmp/keepline-round2-overview-probe.json`、`/tmp/keepline-round2-baseline.log`
- `/tmp/keepline-round2-all-tests-final.log`、`/tmp/keepline-round2-typecheck-final.log`、`/tmp/keepline-round2-native-tests-final.log`
- `/tmp/keepline-round2-native-build-final.log`、`/tmp/keepline-round2-browser-final.log`

仍持续追加的大日志在变化时会重新解析，因此会有活动尖峰；未实现按文件追加字节偏移读取，也未把五分钟观察等同于任意时长的内存保证。

## 上一轮只读同步验证（历史记录）

原始日志及 Codex 元数据库只读访问，所有 Keepline 数据和缓存写入独立临时目录。没有将测试数据写入用户数据库。

盘点：17,375 个 Codex 日志约 31 GB；最近 30 天约 1,012 个、5.8 GB（运行期间新会话使扫描结果为 1,013 个）。最大 Codex 会话文件约 327 MB。Claude 最近 30 天目录约 0.61 GB，过滤后的有效会话为 341 个。

**以下是前轮仅同步的历史结果，不能作为常驻性能验收；29 秒的热扫描已被第二轮修复取代。历史的自动完成项统计也被当前保守判定取代，原话 fallback 不再自动完成。**

从空摘要 / 事实缓存连续跑五轮，每轮主动清空 JS 缓存并关闭摘要缓存连接后再同步，以验证磁盘缓存。识别 1,354 个会话；首次 58.9 秒，后续约 29 秒。各轮结束 RSS 为 1,352 / 1,458 / 1,483 / 1,506 / 1,515 MiB，峰值约 1.48 GiB；最后两轮增加 9 MiB。JS 堆保持几十 MiB，事实缓存序列化大小不超过 16 MiB。运行中的原始会话会继续追加，因此缓存未命中的少数日志正常重读。

进一步收紧读取工具的证据规则并更新事实缓存版本后，再验证两轮：48.7 / 30.0 秒，RSS 峰值 1,458 MiB。最终数据库显示：

| Runtime | 会话数 | 有完成证据的会话 | 已完成要求项 |
| --- | --- | --- | --- |
| Claude Code | 341 | 40 | 165 |
| Codex | 1,013 | 223 | 432 |

这些是匹配规则确认的要求项证据，不代表用户已验收整个任务。另核对全部 81 条真实 goal：13 blocked、2 budget_limited、8 usage_limited 均一致；0 个状态不一致。最近 12 个 Codex 会话均显示账本可用。

日志与复现入口：

- `/tmp/keepline-real-data-repair.ts`、`/tmp/keepline-real-data-repair-final.log`、`/tmp/keepline-real-data-repair-current.log`
- `/tmp/keepline-real-data-repair-results.json`、`/tmp/keepline-real-repair-path`（隔离目录指针）
- `/tmp/keepline-repair-all-tests-final.log`、`/tmp/keepline-repair-native-tests-final.log`、`/tmp/keepline-repair-typecheck-final.log`
- `/tmp/keepline-repair-native-build-final.log`、`/tmp/keepline-repair-browser-final.log`、`/tmp/keepline-repair-package-probe.log`

这是当前数据集的有限重复扫描验证，未声称覆盖任意更大日志或长时间运行。

## 人工验收边界

- 用户选择跳过系统通知点击；本轮未重新启用系统通知。通知 URL / anchor 自动化通过，真实系统点击仍未验收。
- 未注销并重新登录。实际登录自启仍待人工验收；所有原生运行测试保持 autostart 关闭。
- “停止监控后退出”的实际 UI 点击此前受 `cgWindowNotFound` 阻断。本轮已自动验证实际子进程的保留 / 停止及外部服务保护，未把 UI 点击写成通过。
- 通知权限请求到实际 macOS 展示的端到端十秒时限尚未人工测量；本轮计时覆盖 hook、服务队列与轮询预算。
- 默认进度账、菜单栏与通知已改为中文；现有视觉主题保留，旧会话工具页未全面本地化。

## 本地构建产物

- `menubar-tauri/src-tauri/target/release/bundle/macos/Keepline.app`
- `menubar-tauri/src-tauri/target/release/bundle/dmg/Keepline_1.1.0_aarch64.dmg`

本地 ad-hoc 签名，未推送或发布。重打包前退出该构建目录的应用及其自有 sidecar，避免对运行中的可执行文件重新签名。
