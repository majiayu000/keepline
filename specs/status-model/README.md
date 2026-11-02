# 会话状态模型统一（2026-11-02）

## 要支撑的决定

同一个会话，在侧栏、窗格、标签、状态栏、看板、鸟瞰、命令面板、菜单栏、插件和手机端，是否始终显示**同一个词、同一个符号、同一个颜色**。

指标：

- 判定会话状态的入口只有一个（现在有 4 个：`Content::group`、`session_group`、`sidebar_status`、`board_group`）。
- 状态之间的合并映射只写一处（现在 5 处各写一份：`palette::counted`、`sidebar_list::bucket`、`sidebar_wasm::group_name`、`statusbar` 的过滤、`render::status_glyphs`）。
- 非会话的语义色（表单报错、额度、连接、插件装饰）不再借用会话状态色（现在约 60 处借用）。
- 状态色只有一份来源（现在 3 份：`status_colors.rs`、`theme.rs` 菜单栏的 `MenuPalette` 硬编码、手机端 `theme.ts`）。

放弃或降级条件：合并以后，看板列的归属发生了用户依赖的变化（例如 Keepline 外按日志判定的会话换了列），就把「在 Keepline 终端里」保留为一个标记，只统一词和颜色。

## 现状（分析结论）

触发这次重构的是截图：侧栏「在跑」是黄点，状态栏「工作中」弹窗是绿点。原因是两个界面走了不同的判定：

| 判定 | 位置 | 用的地方 | 对「Keepline 终端里正在跑」的结果 |
|---|---|---|---|
| `Content::group` → `Inbox` | core `content.rs` | 菜单栏计数 | `Running` |
| `session_group` → `Group` | app `session_state.rs` | 状态栏、看板列头、鸟瞰、速览、任务详情、顶栏「需要你」 | `Running`（绿） |
| `sidebar_status` → `Group` | app `session_state.rs` | 侧栏、窗格、标签、命令面板、会话详情 | `Working`（黄） |
| `board_group` → `Group` | app `board.rs` | 看板卡片 | `Running`（绿） |

同一类问题还有：

- 「等你验收」有三种颜色：侧栏里没看过的是 `Done` 绿，看过的是 `Ready` 灰，看板、鸟瞰、菜单栏里是 `Review` 紫。
- `session_group` 用 `_ => Paused` 兜底，把「做完了」和「稍后再看」都显示成「停下了」。
- 在 Keepline 终端里被中断的一轮，侧栏显示「等你验收」；终端外显示「停下了」。
- 「等后台」：在 Keepline 终端里是看板「等你验收」列，终端外是「在跑」列。
- 词也有分叉：菜单栏写「待核对」，命令面板写「待验收」，其他地方写「等你验收」。
- 2026-10-26 的设计文档本来就把会话分成两套（「进行中」绿、「在跑」黄），`kanban.html` 的 `--run` 也是绿色。

## 目标结构

### 1. 唯一的状态判定（`keepline-core::status`）

```
Status = Waiting 等你回答 | Error 出错了 | Running 在跑 | Background 等后台
       | Review 等你验收 | Fresh 新开的 | Paused 停下了 | Stale 很久没动
       | Deferred 稍后再看 | Closed 做完了
status::of(workspace, session, now) -> Status
```

- 每个状态固定对应 `key()`（配置和插件协议用）、`label()`、`glyph()`。
- 「日志说在跑」和「画面说在跑」只是证据来源不同，记在「为什么是这个状态」里，不再拆成两个状态。
- 「没看过」是要你注意的标记，不是状态，所以 `Done`/`Ready` 合并进 `Review`。没看过的画实心点，看过的画空心圈。
- 判定顺序：
  - Keepline 终端里活着的会话：先看实时状态，等你回答 → 在跑（含后台子 Agent 在跑）。
  - 停下以后依次判断：做完了 / 稍后再看 → 新开的 → 出错了 → 等后台 → 停下了（被中断）→ 等你验收。
  - 其余会话按记录判断：做完了 / 稍后再看 → 很久没动 → 等你回答 → 出错了 → 等后台 → 等你验收 → 在跑 → 停下了。
- 删除 `Inbox`、`session_group`、`sidebar_status`、`board_group`、`live_phase` 的 `Phase` 判定分支。插件的 `Bucket`、看板的 `Column`、命令面板的计数都从 `Status` 派生，各写一处。

### 2. 两套互不相干的颜色

- **状态色**（`StatusColors`，`colors.json` 的 `status`）：每个 `Status` 一个色位，只给会话的点、符号、标签、卡片使用。
- **语义色**（`Tone`：`ok` / `warn` / `danger`，`colors.json` 的 `tone`）：给表单、额度、后台连接、手机连接、插件装饰使用。
- 菜单栏 `MenuPalette` 的状态色从 `StatusColors` 推出，删掉硬编码。手机端 `theme.ts` 的 `working` 跟着改成黄色。

默认状态色（2026-11-02 用户定）：

| 状态 | 色 | 说明 |
|---|---|---|
| 在跑、等后台 | 黄 `#e8c547` | 等后台靠 ◔ 符号区分 |
| 等你回答 | 橙 `#e8823a` | |
| 出错了 | 红 `#e5534b` | |
| 等你验收 | 绿 `#4cc38a` | 没看过实心，看过空心（同日改：原为紫） |
| 做完了 | 次要文字色，✓ | 你处理完了，退到背景（同日改：原为绿） |
| 停下了、新开的、稍后再看 | 灰蓝 `#8a9bb0` | 靠 ‖ / ○ / ◷ 区分 |
| 很久没动 | 次要文字色 | |

### 3. 防止回退

- 旧的判定函数删掉以后，靠编译器保证不会再出现第二个判定入口。
- 守卫测试：`Status::X.colors()` 这种字面量只允许出现在白名单文件里（画会话状态的界面），其他文件必须用 `Tone`。
- core 单测把判定顺序逐条列出来；app 测试检查 `Bucket`、`Column`、插件状态词、计数分档都覆盖了全部 `Status`。

## 分步

1. core 加 `status` 模块，app 改用它，删掉旧判定。
2. 拆出 `Tone`，菜单栏和手机端的颜色改为从同一份来源推出。
3. 加守卫测试，更新设计文档。

每步单独编译、测试、提交。

## 结论（2026-11-02）：采用

- 判定入口 4 → 1（`keepline_core::status::of`）。合并映射 5 → 2：`sidebar_list::bucket`（插件分档和插件状态词都从它推出）和 `session_state::section`（命令面板计数和鸟瞰分组）。会话状态色的语义借用约 60 → 0（守卫测试 `named_status_colors_stay_where_sessions_are_drawn`）。状态色来源 3 → 1：菜单栏从 `StatusColors` 推出；手机端还是一份手写的色表，色相已对齐，但没有测试锁住。
- 变基时把同天合进 main 的 OSC 7501 程序状态也收进来了（`status::of_program`），它原来自带的「停着」改用「停下了」。
- 放弃条件没有触发。看板列的变化只有一处：Keepline 终端里的「等后台」从「等你验收」列移到「在跑」列，这本来就是规则说的「算在跑那一档」。

截图（`evidence/`，隔离的 HOME 和后台，样例数据，没有调用模型）：

- `dense-status-standard-{light,dark}-pop-native.png`：侧栏「在跑」◐ 和状态栏「工作中」弹窗的点、标题，同一个黄
- `overview-native.png`：看板「在跑」列图标、卡片计时和侧栏同一个黄；「等你验收」紫，「做完了」绿
- `menubar-native.png`：菜单栏写「等你验收」，「在跑」的点是黄色

没验：

- 手机端没有真机截图，只跑了 tsc。
- 插件 wasm 侧栏只验了状态词映射的单测，没截图。
- `colors.json` 写旧键（`working` / `done`）只有单测覆盖。

遗留：`components_take_colors_and_sizes_from_tokens` 在 main 上就失败（`complete.rs` 里的尺寸字面量，来自 2ad0e3e2），不属于这次的改动。

## 改色（2026-11-02 晚）

用户看了实际效果后定：「等你验收」改绿色，「做完了」不再有颜色。理由是一轮跑完、要你去看的会话应该醒目；你已经标记完成的会话应该退到背景。之前「做完了」用绿色，反而比「等你验收」更抢眼。

- 「做完了」用次要文字色的 ✓，不用空心圈。空心圈已经表示「看过的等你验收」，两者要能分开。
- 原来的紫色不再使用。
- 标记完成那一下的印章动画仍是绿色，改从 `Tone::Ok` 取：它表示「这一下完成了」，不是会话停在的状态。
