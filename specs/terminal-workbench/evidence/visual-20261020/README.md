# GPUI 视觉重做与生命周期修正

2026-10-20。决定：采用 `design/README.md` 的集中 Theme，恢复现有界面重做；首版 Codex 预填暂用 `--no-daemon`。未增加搜索、智能摘要、审批判断等新业务能力。找到可靠的守护模式关联方式后移除该临时限制。

本轮指标：工作视图与鸟瞰使用同一套颜色、字体、尺寸；在 1440×900 下对照原稿；退出 Agent 不再留下 current；已关联日志追加仍不触发 ps/lsof。若视觉检查出现裁切或资源回归，先修正后交付，不把编译通过当作体验验收。

## 图像证据

- [工作视图并排](main-side-by-side.png) / [鸟瞰并排](overview-side-by-side.png) / [浏览两组对照](index.html)
- [原生工作视图](main-native.png) / [原生鸟瞰](overview-native.png)
- [Main 原稿](main-design.png) / [Overview 原稿](overview-design.png)

原生图来自 GPUI macOS/Metal 的 `render_to_image`，不是 HTML 仿制。逻辑视口严格断言为 1440×900；Retina 原图为 2880×1800，比较图等比例以 1440×900 显示。测试开关仅在 `visual-test` feature 中存在；默认构建没有导出流程或截图计时器。当前显示器可用工作区不足 900px，测试窗口临时使用 simple fullscreen；退出测试应用后恢复。未更改屏幕录制权限。

数据是 `capture-native.py` 建立的隔离 SQLite、真实 PTY 与模拟 CLI/JSONL，不是用户真实会话，也没有调用模型。第二个 Agent 被结束，shell 保留，用于展示退出后的状态。测试结束会关闭自己的 PTY、宿主和临时目录。原稿缺少 `support.js`，`render-design.cjs` 仅展开源稿已有的模板与数据，未改原稿的颜色、布局或示例内容；浏览器截图关闭外部字体网络请求，使用本机回退字体。

## 验收列表

- [x] 集中 token：`next/crates/keepline-app/src/theme.rs`。组件不再出现裸十六进制颜色、裸 `px(数字)` 或散落的间距工具类；终端 ANSI/OSC 颜色仍归 VT，未设置的终端前景/背景使用 Theme。
- [x] 工作视图：46px 顶栏、240px 左栏、34px 标签行、36px 待办条、32px 画面状态栏、6px 缝、26px 底栏；两画面按 Main 的左侧通高 / 右侧画面与空位排列。证据：原生工作视图。
- [x] 鸟瞰：20px 标题、未开始待办行、五组标题、最小 230px 的自适应会话卡片及结局计数。证据：原生鸟瞰。
- [x] 可见按钮和图标按钮有名称；保持新建、分屏、接管、收起、刷新、任务开始、打开会话、已验收、放弃等现有动作。
- [x] Codex 任务预填和续接命令构造统一 `--no-daemon`；新任务 shell 就绪并启用安全粘贴后才预填，不附加执行回车。证据：`codex_prefill_and_resume_use_no_daemon_and_preserve_shell_quoting`、Task::command、Pane 的事件驱动预填路径。续接按钮的完整交互尚不在本轮。
- [x] 用户提交未关联命令后，仅触发一次有去重的启动识别（500ms 启动宽限）；确认托管 Codex 尚未绑定时推送“改用 codex --no-daemon 才能跟踪”。不是给所有 shell 显示警告。证据：`manually_entered_unbound_codex_gets_a_hint_and_exit_clears_it`。特别慢的延迟启动仍依赖后续日志/输入事件，不声称覆盖任意 shell 函数。
- [x] 用一个阻塞式 kqueue 监听已证实 Agent PID 的 NOTE_EXIT；只结束对应 AgentSession.current，保留 task/outcome 与 shell。Hook 取已证明的祖先 PID，文件识别只取打开该日志的 PID。证据：`agent_exit_clears_current_without_stopping_shell_or_changing_outcome`，含 SQLite 落盘检查。
- [x] 工作区修订号与终端识别提示由现有状态订阅推送，客户端有变化才取新快照；空闲不定时刷新。
- [x] 本地测试：core 8 + daemon unit 6 + integration 10，共 24 项；另一个 ignored 是隔离 daemon fixture 的子进程入口。clippy（core/daemon，warnings denied）通过。完整测试回执：[validation.log](validation.log)。40 空闲 PTY 的 CPU 0%、磁盘 0 B、Unix 调用 +2；4 Agent / 30 秒累计 CPU 0.0184s、额外 ps/lsof 0 次、增量读取 87,780 B。编译包含原生客户端与视觉导出构建；上游 GPUI 的既有 deprecated 警告单列，不是无警告构建。
- [ ] 中文 IME 和实际点击体验：留待用户一起手测。
- [ ] 官方 Codex 退出路径再次验收、Claude Code Hook、长期功耗：未用合成截图或短测替代。

## 尚未对齐的地方

1. 本轮重做已有功能。原稿中的搜索、设置、空间拖动宽度、智能“你在这里”浅色卡片、任务详情/完成、Agent 选择分体按钮、逐个清理、空间过滤、离开后“新”标记尚无完整实现，未放入无效按钮或伪造内容。
2. “等你回答”和“待核对”没有已证明的业务状态来源，显示零及“尚无已确认的状态证据”；存活关联显示“进行中”，同时说明实际运行状态以终端为准。不会由进程存活推断 Agent 正在工作，也不会由退出推断完成。
3. 空位目前保留细实线边框，尚未对齐原稿的虚线；侧栏保持固定 240px；会话卡保留原有的显式结局按钮，因此比原稿的纯链接卡更高。无新的快捷键。
4. 参考稿示例有几十个会话，原生隔离样本为两段会话、三个待办，内容密度不能直接进行逐像素相等断言。Main 源稿的最小网格高度也可能让原稿底栏落在 900px 视口外；原生布局使用规范里的明确高度。

## 复现

在实现工作树运行（ZIG 依照 next/bootstrap.sh 或本机已验证的路径）：

```sh
cargo test --manifest-path next/Cargo.toml -p keepline-core -p keeplined -- --nocapture
cargo clippy --manifest-path next/Cargo.toml -p keepline-core -p keeplined --all-targets -- -D warnings
cargo build --manifest-path next/Cargo.toml -p keepline-app --features visual-test
python3 specs/terminal-workbench/evidence/visual-20261020/capture-native.py
PLAYWRIGHT_MODULE=/path/to/playwright node specs/terminal-workbench/evidence/visual-20261020/render-design.cjs
PLAYWRIGHT_MODULE=/path/to/playwright node specs/terminal-workbench/evidence/visual-20261020/compare.cjs
```

原生日志：[native-render.log](native-render.log)。本机检查日志保留在 `/tmp/kl-design-tests-final.log`、`/tmp/kl-design-clippy.log`、`/tmp/kl-design-build.log`。代码在 `feat/agent-workbench` 工作树，尚未提交、合并、推送；原 main 工作区没有替换。
