# 鸟瞰降噪验收（2026-10-21）

决定：默认鸟瞰和“需要你”只显示有交互来源证据的会话。自动化与未知来源保留在可展开隐藏区，显示具体原因；不删除会话，不改变 PTY 控制权。标题只采用确认覆盖的首条真实用户原话；缺失时显示未知，不用 CLI 摘要或尾段追问替代。

## 结果与逐项证据

| 项目 | 结果 | 证据 |
|---|---|---|
| 四个 you-are-here 地图任务默认隐藏 | 通过：真实日志 + 原生展开 | 这四条 entrypoint 继承 claude-desktop，但首任务含明确 unattended 标记；[隐藏列表截图](hidden-expanded.png) |
| 临时目录、SDK / 非交互、Claude sidechain、Codex exec / 子代理 | 通过：真实日志与规则测试 | [分类清单](classification.json)、[最终解析测试](parser-final.log)；sidechain 身份与父会话分离，不能覆盖父卡 |
| Claude -p / print、内部 userType、Hook 标记 | 通过：合成规则测试 | 未调用真实 CLI；不把合成测试冒充 installed Hook 验收。普通用户提问 you-are-here/remem 不按名字隐藏 |
| 缺少来源元数据 | 通过：明确未知后隐藏 | 不当作交互式放行；展开显示“缺少交互入口证据”，不伪称已证实自动化 |
| 注入、指令块、技能正文、命令包装、粘贴正文清理 | 通过：规则测试 + 真实来源抽查 | system-reminder、AGENTS/CLAUDE 块、技能正文、command-*、ambient browser、带属性 pasted_content、X 网页粘贴；表单回复只保留 answer |
| 标题和“你要它”采用真实首条原话 | 通过：20 个有首条来源证据的真实会话 | [逐条核对](audit.md)，17 Claude + 3 Codex；标题 20/20，“你要它”20/20。后续真实要求保留为追加 |
| 只读尾段没有首条原话 | 通过：明确未知 | 不把“继续”当最初要求，标题显示“最初原话未覆盖”；卡内把读到的用户文字标作追加 |
| 重复注入不产生重复标题/原话 | 通过：单测 | 清理后在同回合去重；只设首条标题，后续 summary/title 注入不覆盖 |
| 整张卡可点击 | 通过：Computer Use | 外部卡开 Here，不 Launch；托管卡跳已有画面。操作前后终端数仍为 2，见 [动作读回](native-actions.json) |
| 卡面只有一个主按钮 | 通过：原生截图 | 等你回答/出错/暂停为继续；待核对为你在这里；[最终鸟瞰](overview.png) |
| 其他动作在右上角 ⋯ | 通过：原生菜单 | [菜单截图](card-menu.png)：存为待办、暂存、已验收、放弃；保存已实际操作并从 SQLite 读回 |
| 菜单/详情点击不冒泡导航 | 通过：Computer Use | 菜单展开未启动 shell；保存后留在鸟瞰；关闭 Here 后没有重新打开；待核对主按钮开 Here 而不跳转，见 [详情图](review-card.png) |
| 隐藏计数可展开 | 通过：Computer Use | 选定 fixture 的 10 条隐藏项展开后展示四地图、两 SDK、一临时、一 exec、两家各一子代理 |
| 缓存更新不重扫目录、之后不重复读 | 通过：缓存失效回归与实测 | 规则更新从 SQLite 取近期路径，仅有界重读头/尾并同事务替换派生记录；第二次打开文件/目录/读字节全为 0 |

## 20 个样本的正确率与覆盖率

[审计详情](audit.md)记录每条标题、四字段中的“你要它”、原始 JSONL 路径及字节偏移。[audit.json](audit.json)包含原话与来源，[audit.py](audit.py)复现来源包含检查和预期文本核对。核对由本实现代理阅读来源完成，**不是用户手测或独立人工签收**。没有另调模型。

本轮选取 20 个首条已覆盖的会话：标题与“你要它”的来源/语义正确率分别为 **20/20（100%）**。这不是全部会话的历史恢复率，也不是所有格式的泛化保证。

同一有界索引快照有 **470 个近期身份**：204 个有交互入口证据；266 个进入隐藏区（96 Codex 子代理、138 Claude sidechain、24 Codex exec、4 无人值守地图任务、2 SDK、1 临时目录、1 来源未知）。在 204 个交互会话中，**只有 25 个（12.25%）首条原话被已有头/尾窗口覆盖**，其余明确未知。这项覆盖缺口没有用猜测填补，也没有为了补标题扫描全文。新会话从 offset 0 开始增量读取，能记录后续真实首条输入。

真实抽查中修复了浏览器 ambient 包装、带属性的 pasted_content 闭合标签和表单 question/answer 包装污染。无引用边界的任意粘贴无法可靠判断作者；只处理明确的围栏、引用、网页边界或可识别的助手解释加用户祈使尾句，不保证对所有无标记混合文本准确。技能的原始调用正文不拿来生成用户要求；用户自己在普通文字里提到技能名仍保留。

## 资源与测试

这次复用了上一阶段的私有缓存数据库，不写真实日志。规则失效后从缓存取 470 个近期路径，**目录遍历 0、读取 68,827,656 B（约 65.64 MiB）、2.401 秒**；每个文件仍是最多 16 KiB 头和 128 KiB 尾。元数据大行只从已读前缀提取完整字段，不额外读正文。随后同库启动 **0.025 秒、打开文件 0、读取 0 B**，见 [warm-index.json](warm-index.json)。实验中重新复制私有缓存后重跑用于验证修改，产品不会每次重做。

- [backend-tests.log](backend-tests.log)：core 13、daemon 单元当时 16、集成 15 通过；1 个 ignored 为集成测试子进程入口。40 空闲 PTY 5 秒：CPU 0%、磁盘 0、syscalls 0；4 模拟 Agent 30 秒写入：CPU 0.2908 秒，ps/lsof 增量 0，读 88,084 B。
- [parser-final.log](parser-final.log)：最后增加“首条未覆盖”修复后，daemon **17 项通过**，包括已有索引/FSEvents 测试。这与上面的 daemon suite 重叠，不累加。
- [client-tests.log](client-tests.log)：最终客户端 binary **13 项通过**；包含当前共享工作树里的并行 screen 测试，但那部分修改不属于本轮提交。
- [build-tail.log](build-tail.log)：最终原生 app 构建成功，上游 GPUI 弃用 warning 仍存在。

复现命令（在工作树根执行；构建环境沿用上一阶段 ZIG、LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast、DEVELOPER_DIR）：

```sh
cargo test --manifest-path next/Cargo.toml -p keepline-core -p keeplined -- --test-threads=1 --nocapture
cargo test --manifest-path next/Cargo.toml -p keepline-app --bin keepline-app
cargo build --manifest-path next/Cargo.toml -p keepline-app --bin keepline-app
cargo build --manifest-path next/Cargo.toml -p keeplined --example content_audit --example content_fixture
next/target/debug/examples/content_audit /tmp/kl-noise-audit-20261021 > /tmp/kl-noise-audit.json
python3 specs/terminal-workbench/evidence/overview-noise-20261021/audit.py
```

审计脚本选定此次会话 ID，未来数据变化应重新选样和人工核对，不能复用旧判断作为新结论。原生驱动 `native-check.py` 依赖上述私有审计库，取 20 个审计样本 + 10 个已知隐藏样本，并创建 2 个明确标记的模拟 Agent PTY；窗口内模拟画面的内容不是模型输出。来源只读，fixture 只监听私有 logs，独立 HOME/runtime，不接管用户实例。截图由 Computer Use 操作真实 GPUI 窗口取得，逻辑 1440×888；不是设计稿拼贴。隐藏列表图取自同轮功能构建，最终仅压平了卡面摘要换行；所有实例均由驱动结束并读回确认退出。

没有重启用户后台、关闭用户终端、修改 CLI 配置或启动真实 Codex/Claude。真实安装版 -p/Hook/SDK 各种启动器的全链路、无标记粘贴的任意作者归属、长期功耗与独立用户签收未验证。

用户指出的 `screen.rs`、应用 `Cargo.toml`、`next/eval/`、examples 与 terminal-reference 文件未修改、未暂存、未提交。
