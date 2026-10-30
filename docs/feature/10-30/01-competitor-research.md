# 同类产品与用户真正卡住的地方

记录日期 2026-10-30。目标是为 Keepline 选择值得验证的功能，不是评选产品冠军。

| 内容 | 来源 | 等级 | 本轮验证 |
|---|---|---|---|
| 产品宣称的能力 | 官方页面、README、文档 | 官方说明 | 已读，不代表跑通 |
| 用户描述的问题与使用行为 | 原始 issue、HN 评论 | 原始用户反馈 | 已读原文；版本与身份仍有限制 |
| Keepline 原生基础 | 固定提交的 6 个源码文件 | 固定源码 | 已读，未验安装版 |
| 功能优先级与市场机会 | 根据以上证据判断 | 本轮判断 | 待用户观察验证 |

本轮区分官方宣称、一手用户反馈、源码与产品推断。公开反馈不是代表性抽样，需求强度尚未经过访谈或行为验证。下文的重复是样本内重复，不是市场发生率。

## 1. 产品池与当前形态

截至采集时点；star 是仓库关注量，不等于使用人数，更不能跨列排名。Herdr App 和 TUI 合并为一个产品家族。定价不影响本轮功能选择，因此没有做购买建议或价格普查。

| 产品 | 类别 | 官方定位及本轮关注 | 规模信号与口径 |
|---|---|---|---|
| [Herdr](https://herdr.dev/) | 开源直接竞品 | 原生 App + 独立 core；另有原 TUI，见下一节 | TUI 46,746★；App 469★，GitHub API 2026-10-30 |
| [cmux](https://cmux.com/) | 开源直接竞品 | 原生终端、状态提醒、浏览器和 CLI | 28,612★，同日 GitHub API |
| [Paradev，原 Superset](https://paradev.com/) | 源码可见桌面工作台 | worktree、终端、diff、PR、自动化 | 15,741★，同日 GitHub API |
| [Conductor](https://www.conductor.build/docs) | 商业 Agent App | 从隔离工作区到审查、PR、云端协作 | 官网称 100k+ builders；未独立核实活跃量 |
| [Agent Deck](https://github.com/asheshgoplani/agent-deck) | 开源终端管理器 | 组、搜索、fork、worktree、用量、编排 | 1,184★，同日 GitHub API |
| [Orca](https://github.com/stablyai/orca) | 开源 Agent 工作台 | 多 Agent、桌面/手机/远端、自带订阅 | 110,580★，同日 GitHub API |
| [T3 Code](https://github.com/pingdotgg/t3code) | 开源 Agent App | 统一会话、Queue/Steer、工作树中的恢复 | 32,194★，同日 GitHub API |
| [Emdash](https://emdash.com/docs) | 开源相邻工作台 | 任务、独立工作树、diff、CI、复用资源 | 6,118★，同日 GitHub API |
| [workmux](https://workmux.dev/) | 开源替代方案 | 在既有 tmux 上管理工作树、准备和收尾 | 2,962★，同日 GitHub API |
| [Nimbalyst，原 Crystal](https://github.com/stravu/crystal) | 转型案例 | 旧仓库明确指向 Nimbalyst；不能按旧提交断言产品停止 | 旧仓库 3,130★，不代表新产品采用量 |
| [Warp](https://www.warp.dev/warp-ai) | 相邻终端与 Agent 平台 | AI 与终端工作流结合 | 官方 warp 仓库 65,787★，不等同客户端用户 |
| [Qoder](https://qoder.com/) | 商业 Agent 产品家族 | 桌面、IDE、CLI、手机等不同形态 | 官网家族级 6M+ builders，不是设备连接使用量 |
| [tmux](https://github.com/tmux/tmux) + 原有编辑器 | DIY 替代方案 | 不迁移终端，保留脚本与肌肉记忆 | 本轮 GitHub 规模请求被限流，数值不报 |

发现渠道包含 GitHub 两组按 star 排序的仓库查询、HN 和官网互链。后者补出了 Orca、workmux 等，未把单次关键词返回顺序当产品排名。我方 README 的相关链接也已查阅。维护者全仓库清单请求超时，不能声称已查完全部兄弟项目。

## 2. 七个重点对象，分别值得学什么

### Herdr：终端以外，开始连接 Agent 之间的工作

[当前官网](https://herdr.dev/)介绍原生 App、herdr-core、SSH 多机统一侧栏、`hc run` / `hc send`。启发是允许现有 Agent 派出小任务并收回报告，减少人在窗口间复制粘贴。【官方说明】

必须分清：新 App 与旧 TUI 是两个程序、两套运行实例；旧 TUI 的会话不会自然出现在新 App。官网还写明机器重启或 core 停止后，程序会停止，恢复的是同目录 shell，不自动恢复原 Agent。官网 layouts 是示例，不保证全部随安装包交付。【官方说明】

可借鉴：明确谁需要处理、保留真实终端、给 Agent 小而准确的控制接口。暂不照搬让 Agent 改写整个 UI 的系统；目前公开需求不足以把它排到前三。

### cmux：提醒必须能把人带回正确位置

[官网](https://cmux.com/)确认侧栏、提醒、浏览器、CLI/socket；[恢复文档](https://cmux.com/docs/session-restore)区分布局恢复、原生 session resume 和任意进程内存恢复，不能合并宣传。【官方说明】

[状态 issue](https://github.com/manaflow-ai/cmux/issues/1027)以及[按标签静音请求](https://github.com/manaflow-ai/cmux/issues/1515)说明“有提醒”不等于“提醒可信、值得打断”。【用户原始反馈】

可借鉴：文字状态、跳到原会话、聚焦静音、保留原生 CLI 操作。已有大量能力，提醒、恢复、内置浏览器都已有官方能力说明，不能单独构成切换理由。

### Paradev：让一个任务从开工到审查都在同一个位置

[工作区](https://docs.paradev.com/workspaces)、[准备与收尾脚本](https://docs.paradev.com/setup-teardown-scripts)、[故障排查](https://docs.paradev.com/troubleshooting)共同说明 worktree、准备、运行、归档是一个整体，不能只做“创建工作树”按钮。【官方说明】

[PR 就绪请求](https://github.com/ParadevAI/Paradev/issues/8410)指出用户仍要到 GitHub 查冲突、必要审批和检查；测试全绿并不保证可合并。它支持“减少检查工具切换”的需求，不能据此声称 Paradev 当前缺整个 PR 视图。【用户原始反馈】

可借鉴：任务与分支、原始终端、diff、检查结果和 PR 关联；本轮不建议复制其所有插件和云服务。

### Conductor：隔离只是开头，工作区还要能被检查和收尾

[介绍](https://www.conductor.build/docs)和[worktree 文档](https://www.conductor.build/docs/concepts/git-worktrees)确认每个工作区有文件、分支、命令、会话和审查路径，也说明 setup 与被 Git 忽略文件的处理。【官方说明】

文档明确 worktree 是开发隔离，不是安全沙箱。本轮未运行隔离测试，不承诺防止 Agent 访问用户机器其他文件。【官方说明】

可借鉴：把创建、环境准备、检查和归档做通。商业形态是 App/云服务叠加自带订阅；未核价，不评价性价比。

### Agent Deck：成熟的终端用户并不一定想换一个 App

[官方仓库](https://github.com/asheshgoplani/agent-deck)提供 groups/search/fork、工作树与准备/销毁脚本、用量及 Conductor 功能。这里的 Conductor 是该产品中的编排角色，不是上一款 App。【官方说明】

README 自己把若干成本解析、预算硬停止标为 untested；不能把所有 Agent 的预算控制当作已验能力。其持续编排和自动响应也比“看见问题并跳回去”承担更多控制责任。【官方说明】

可借鉴：重用原有终端、支持 session 精确控制与历史搜索；先减少操作，不默认再加一个常驻模型当总管。

### T3 Code：区分“下一轮再做”和“现在改方向”

[composer 文档](https://github.com/pingdotgg/t3code/blob/32ecd71726fee16f335484c1f95cbce126d461f6/docs/user/composer.md)已写 Queue/Steer 区分、服务器保存队列、编辑或删除排队内容，以及工作树恢复的共享目录限制。【官方说明】

[相关讨论](https://github.com/pingdotgg/t3code/discussions/6684)记录用户因队列语义迁移受阻，但也有维护者说明后续已经实现；这是需求来源，不是“现在仍没实现”的证据。【用户原始反馈】

可借鉴：用户输入的时间与执行时间可预期。Keepline 保留真实终端，先利用各 Agent 已支持的原生语义，不另造一套猜测回合结束的通用状态机。

### Orca：广功能同时暴露了草稿、资源和跨仓库问题

[官方仓库](https://github.com/stablyai/orca)宣称支持自带订阅、多 Agent、桌面、手机和远端。【官方说明】

[多仓库 diff](https://github.com/stablyai/orca/issues/1099)、[评论草稿丢失](https://github.com/stablyai/orca/issues/36286)、[Windows 后台重启](https://github.com/stablyai/orca/issues/36262)、[上下文预算](https://github.com/stablyai/orca/issues/36283)是不同范围的具体报告。【用户原始反馈】

可借鉴：检查结果、保留未发出的反馈、显示实际用量。不能因 Windows 单例就称 Orca 整体不稳定，更不能断言原生 Keepline 一定更快。

## 3. 公开用户反馈支持哪些需求

逐条原文、日期、平台账号哈希、状态与标签存于 [反馈清单](evidence/voc-selected.json)。下面只列与决策有关的代表信号。数字均为样本内条数；同一平台账号只计一次，跨平台是否同一真人未知。所有分类仍是公开个别反馈。

| 用户要完成的事 | 代表原始证据与时间 | 对功能选择的含义 | 边界和反例 |
|---|---|---|---|
| 发现真的在等我的会话 | [状态误报/漏报](https://github.com/manaflow-ai/cmux/issues/1027)，3-06；[等输入颜色难认](https://github.com/ParadevAI/Paradev/issues/8355)，10-09；[后台子 Agent 尚未结束](https://github.com/ParadevAI/Paradev/issues/8120)，10-03 | 一处处理等待事项，文字说明原因、等了多久、回到准确会话 | issue 不是当前版本复现实测；最后一条 API 刷新失败 |
| 核对这轮工作是否可以交付 | [PR 冲突/检查/审批](https://github.com/ParadevAI/Paradev/issues/8410)，10-12；[跨仓库 diff](https://github.com/stablyai/orca/issues/1099)，4-26；[review 评论草稿](https://github.com/stablyai/orca/issues/36286)，10-30 | 任务结果页结合 diff、执行证据、PR/check，反馈能回到原会话 | 不自动把绿 CI 当验收；不默认引入完整编辑器 |
| 并行时不用手工管理一堆环境 | [环境与端口](https://news.ycombinator.com/item?id=48237005)，5-22；[独立基础设施和 URL](https://news.ycombinator.com/item?id=48244816)，5-23；[多套测试栈仍有端口假设](https://news.ycombinator.com/item?id=48253274)，5-24 | 工作树创建、已批准的准备脚本、端口与预览、收尾 | [用户已有端口脚本](https://news.ycombinator.com/item?id=48247259)，5-23，是应接入的替代物 |
| 数日后知道从哪接着做 | [任务归组与标签](https://news.ycombinator.com/item?id=48242262)，5-22；[命名恢复一组项目](https://github.com/manaflow-ai/cmux/issues/2086)，3-25 | 接手摘要、会话历史、按任务找回原始记录 | 不能将“保存布局”包装成“保留运行中的进程” |
| 找回长对话中的一次决定 | [长会话目录与定位](https://github.com/manaflow-ai/cmux/issues/11953)，9-04 | prompt 时间线、跨会话搜索、原文锚点 | API 已核对 closed/completed，不能说竞品至今没做；仅一条导航样本 |
| 任务别因后台问题丢掉 | [Herdr 0.9.5 Linux 重启布局丢失](https://github.com/herdrdev/herdr/issues/5349)，10-17；[Orca Windows 后台退出](https://github.com/stablyai/orca/issues/36262)，10-30 | 分清界面重连、进程死亡与历史 resume；失败原因可见 | Herdr issue 已关闭，且属于 TUI；不得套用新 App |
| 终端首先得能顺畅打字 | [Superset 输入停顿](https://github.com/ParadevAI/Paradev/issues/8108)，10-03；[历史迁移到 Zellij](https://news.ycombinator.com/item?id=48237990)，5-22；[Orca Windows 卡顿自述](https://github.com/stablyai/orca/issues/36284)，10-30 | 输入、复制、搜索、资源占用是基础门槛，别被功能扩张拖垮 | 原帖测量不是本轮测量；没有因果或跨产品性能排名 |
| 知道上下文和额度去了哪里 | [派单 worker 上下文膨胀](https://github.com/stablyai/orca/issues/36283)，10-30 | 每会话用量来源、最后更新、未知明确显示 | 单例；请求输入包含缓存，不等于实际账单；不能承诺精确剩余额度 |
| 手机上短暂查看和回复 | [手机输入效率](https://news.ycombinator.com/item?id=50299174)，10-30 | 若做远程，先解决短回复、待处理事项、重连 | 仅一个明确移动痛点样本，不能推断人人要手机版 |
| 不为新工具放弃成熟习惯 | [tmux/编辑器已经足够](https://news.ycombinator.com/item?id=48246650)，5-23；[试用后回到 tmux](https://news.ycombinator.com/item?id=50301261)，10-30 | 一个真实项目即可试用；可接既有命令与脚本 | 这是反对新 App 的证据，不能把反馈全部算潜在客户 |

另读了 [审查压力帖子](https://www.reddit.com/r/ClaudeCode/comments/1ue76os/how_are_you_all_reviewing_code_when_youve_got/)和[并行会话数讨论](https://www.reddit.com/r/ClaudeCode/comments/1xdjb2g/how_many_claude_code_sessions_can_you_run_before/)。前者主帖链接个人文章，部分评论者自荐工具；后者页面相对时间与搜索索引时间不一致。两者只作补充，不加入 31 份样本、独立账号或发生频率统计，也不从评论里的多 Agent 自述推算生产率。

## 4. Keepline 已有基础，哪些才算新增

本轮固定源码提交 `5b25397ca9ea18e0d21ee790fe977db8cd91e39f`，不是工作区所有改动，也不是已安装版；远端分支读取为另一提交，main 读取超时，因此不作发布差距判断。以下是接口/实现层证据，不称完整用户验收。

| 能力 | 当前可确认到的层级 | 本轮建议 |
|---|---|---|
| 任务与验收标准、机械证据字段 | [tasks.rs](https://github.com/majiayu000/keepline/blob/5b25397ca9ea18e0d21ee790fe977db8cd91e39f/next/crates/keepline-app/src/workbench/tasks.rs#L73) 已有渲染入口 | 加变更与检查的具体定位；不要重新发明验收对象 |
| 历史 session 精确继续 | [sessions.rs](https://github.com/majiayu000/keepline/blob/5b25397ca9ea18e0d21ee790fe977db8cd91e39f/next/crates/keepline-app/src/workbench/sessions.rs#L24) 按会话生成 resume | 加退出原因与恢复后核对；不是从零新增“恢复” |
| 鸟瞰和按任务整理 | [overview.rs](https://github.com/majiayu000/keepline/blob/5b25397ca9ea18e0d21ee790fe977db8cd91e39f/next/crates/keepline-app/src/workbench/overview.rs#L1) 有界面实现 | 整理真正要处理的事项；不是再加一个 dashboard |
| 启动、接回与终端输入协议 | [protocol.rs](https://github.com/majiayu000/keepline/blob/5b25397ca9ea18e0d21ee790fe977db8cd91e39f/next/crates/keepline-core/src/protocol.rs#L26) 有请求 | 可作为精确会话 CLI 的基础；网络接入仍需另外实现 |
| 原生 Agent 选择与恢复命令 | [model.rs](https://github.com/majiayu000/keepline/blob/5b25397ca9ea18e0d21ee790fe977db8cd91e39f/next/crates/keepline-core/src/model.rs#L70) 的 Agent 枚举 | 不把其他分支的支持或通用终端可运行，等同原生全量识别 |
| worktree 全生命周期、diff/PR 核对工作流 | 本轮 6 个文件与原生目录关键词搜索未验证这两条完整用户流程；既有 queued 是内部请求排队 | 可作候选新增；未提交修改与旧 Bun/Web 不计入该结论 |

当前既有产品文档将 SSH、模型路由、编辑器、插件系统等列为首版范围外。本轮可提出下一阶段候选，但不把这些直接改成必须补齐的缺陷。

## 5. 会改变结论的事实，及反向核验

| 事实或假设 | 回源与日期 | 核验结果 |
|---|---|---|
| Herdr 只是 TUI，桌面形态可形成差异 | [Herdr 官网及 FAQ](https://herdr.dev/)，10-30 | 不成立。已有独立原生 App；旧 TUI 和新 App 不能混用能力与故障 |
| Superset 品牌/仓库仍原样 | 官网跳转 [Paradev](https://paradev.com/) 与 GitHub API，10-30 | 更正。已更名，旧链接会跳转 |
| 原生 App 可以保证任意重启后活进程仍在 | [Herdr FAQ](https://herdr.dev/)与 [cmux 恢复文档](https://cmux.com/docs/session-restore)，10-30 | 不成立。界面退出、后台停止、机器重启需分开 |
| Queue/Steer 是还没人满足的新需求 | [T3 当前文档](https://github.com/pingdotgg/t3code/blob/32ecd71726fee16f335484c1f95cbce126d461f6/docs/user/composer.md)及[讨论](https://github.com/pingdotgg/t3code/discussions/6684)，10-30 | 不成立。文档已有；老抱怨不能代表当前缺失 |
| worktree 就是安全隔离 | [Conductor 官方 worktree 文档](https://www.conductor.build/docs/concepts/git-worktrees)，10-30 | 不成立。明确不是安全边界；本轮未测试沙箱，也不提出保护性卖点 |
| 工作树只需创建，不用准备环境 | [Paradev 故障排查](https://docs.paradev.com/troubleshooting)与[生命周期脚本](https://docs.paradev.com/setup-teardown-scripts)，10-30 | 不成立。依赖、被忽略文件、启动与清理仍需处理 |
| cmux 至今缺长会话导航 | issue 11953 的 API，10-30 | 无法支持。closed/completed；具体安装版行为未验 |
| Crystal 旧仓库不活跃就代表产品失败 | [仓库说明](https://github.com/stravu/crystal)，10-30 | 不成立。它指向 Nimbalyst；本轮不编造失败原因 |
| Keepline 的任务结果字段是建议新增 | 固定源码 tasks.rs，10-30 | 更正。已有字段，应补可核对的具体工作流 |
| “结果核对”可由唯一卖点宣传 | [Paradev](https://paradev.com/)、[Conductor](https://www.conductor.build/docs)、[Emdash](https://emdash.com/docs)，10-30 | 不成立。别人已有 diff/检查/PR；区别只能通过实际任务观察证明 |

## 6. 数据与覆盖边界

[本轮统计](evidence/voc-summary.json)由保存的数据计算。GitHub 热度/最新两路的 8 次计划请求中成功 5 次，100 行去重为 76 个 URL；最新结果会偏向活跃的大仓库，热度结果也会混入系统支持等宽泛诉求，不将排名用作需求排行。进一步回源旧轮及定向新 issue，筛选 15 条 GitHub 加 16 条 HN，共 31 个平台账号。HN 3 个讨论树是发现池，并非其中每条都计为有效用户。没有跨平台真人去重。

OpenCLI doctor 显示扩展连接正常，两次 Reddit search 却返回 HTML/JSON 解析错误；公开网页仍可阅读。GitHub CLI 直连超时后，匿名 urllib API 可用，后段匿名额度耗尽；3 路检索失败，若干当前状态和固定竞品 commit 未补齐。本轮未做竞品源码实现判断，因此也不冒充固定源码深测。

以上资料支持“问题确实有人遇到”与“功能值得验证”，不支持“多数开发者需要”“用户一定迁移”“可以收多少钱”。未找到有充分一手材料证明死亡原因的失败产品；以真实迁移回 tmux/Zellij、旧产品转型和功能被补齐作为反例，避免杜撰失败案例。没有运行体验、性能、成本、兼容或稳健性对照实验。
