# 中文输入法复测：2026-10-20

## 用户手测补充（2026-10-20 21:55）

来源：用户反馈；豆包输入法，keepline-app 内运行真实 `cat`。本节是**用户手测**，不是 Computer Use、模拟或单元测试结论；没有新增截图。

- **通过**：候选窗紧贴终端光标。
- **通过**：两个画面之间切换后，真实拼音输入正确进入聚焦画面。
- **不通过**：Ctrl-C 无法退出 cat；已作为本轮键盘编码修复的验收项。

以下为此前自动复测的原始记录，保留当时的限制；其中候选窗及两画面的待验收项由上述用户手测补齐。


2026-10-20 后续决定：用户已结束本轮自动输入法复测，剩余候选窗位置与两画面真实拼音由用户手动确认；以下保留原证据和限制，不继续索取置前权限，也不将模拟验证补记为真实输入法通过。

当时决定：**尚不能宣称全部验收通过**。应用内真实组字、提交、退格已有 Computer Use 画面；系统候选窗位置和两画面间的真实拼音输入仍待取得证据。普通 Unicode 输入的焦点隔离不能替代真实输入法验证。

## 环境与边界

- 实现工作树：`keepline-agent-workbench`，分支 `feat/agent-workbench`。本轮不提交、不推送。
- 测试客户端 `/tmp/Keepline-IME-Retest.app` 是现有普通构建的逐字节副本，未启用 `visual-test`。因 Computer Use 无法绑定裸可执行文件，使用临时 `.app` 包装；摘要见 [environment.txt](environment.txt)。它连接真实后台，并非 fixture。
- 原客户端 PID 33275 保留；后台 PID 8926、启动时间 20:59:19 保持不变。用户原有三个运行终端 PID 10304、33651、33764 未停止、未接管输入。见 [processes-after.txt](processes-after.txt)。
- 所有测试文字只进入本轮新开的终端，运行 `stty -echo -icanon; cat; stty sane`，关闭 TTY 本地回显以避免把 cat 回显误判为重复提交。未启动新的 codex/claude。
- 输入、选字、画面操作及目前全部 PNG 均经 Computer Use。获用户授权后使用 AppKit 将测试 Keepline 置前。Computer Use 只能向目标应用投递按键，不保证系统输入源快捷键会被 macOS 全局处理。
- 输入源初始只读查询为苹果 `com.apple.inputmethod.SCIM.ITABC`，见 [input-source-before-activation.txt](input-source-before-activation.txt)。测试期间前台与输入源发生外部变化，工具也报告了用户改变应用状态；曾读到豆包及 ABC。因此不能仅凭初始输入源记录证明每次组字都来自苹果系统拼音。

## 此前自动复测逐项记录

| 项目 | 当时判定 | 证据与限制 |
|---|---|---|
| 1. 客户端重启、原终端重连、内容保留、继续输入 | 重连及内容保留通过；输入完整性有待定位疑点 | [原用户终端只读重连](01-existing-reconnected.png)；[重启前 cat](01-before-client-restart.png)、[重启后 cat](01-after-client-restart.png)、[继续输入](01-reconnected-input.png)。测试客户端 36942 → 45371，后台未动；9 条终端记录和 PID 全部一致，见前后 JSON。首次 `typeText('RESTART-A-CONTINUED')` 的画面少了首字母 R；后续逐键 `reconnect` 完整，左右切换后的 FIRST-KEY 也完整。未查清一次遗漏来自应用、工具还是焦点切换，不把复测成功当成原因已经消失。 |
| 2. 光标处组字、下划线、候选窗紧贴下方 | 组字画面通过；整项尚未通过验收 | [真实组字](02-real-pinyin-marked.png)可见 `ni'hao` 在当前光标处带下划线，组字时无终端块光标叠压；没有取得系统候选窗截图，无法判断其位置。 |
| 3. 选“你好”只出现一次、无残留 | 真实输入法操作通过；苹果系统拼音来源需同步复核 | 逐键 n/i/h/a/o 后按 Space，[结果](03-real-pinyin-committed.png)只有一次“你好”，无拼音或下划线残留。并非粘贴中文，也不是模拟 marked text。 |
| 4. Enter 提交字母；组字退格只删拼音 | 真实输入法操作通过；苹果系统拼音来源需同步复核 | [Enter 提交 nihao](04-real-return-latin.png)；新一轮 n/i/h/BackSpace 后仅剩带下划线 `ni`，[退格截图](04-real-backspace-marked.png)，上方已提交的“你好”和 nihao 保留。 |
| 5. 中文输出字距 | 真实窗口视觉通过 | [中文、日文、韩文 cat 输出](05-cjk-real-cat.png)。中文“实现反馈”没有之前每字间明显留缝；这是 Unicode 输出的渲染验证，不作为输入法证据。 |
| 6. 两个画面切换，中文仅进入焦点画面 | 普通中文输入通过；真实拼音部分尚未通过验收 | [左画面](06-focus-left.png)、[右画面](06-focus-right.png)为直接 Unicode 输入，左右文本未串入另一画面。[首字符复查](06-focus-first-key-recheck.png)完整；这些均不能替代两画面真实拼音组字/选字测试。 |
| 7. Interrupted 提示、两按钮、原位重开可输入 | 通过真实窗口操作 | [真实 Interrupted](07-interrupted-real.png)显示“已中断（后台重启，原进程已结束）”、“在这里重新开一个终端”和“关闭”。Computer Use 点击重开后，同一位置创建新 shell 并运行 cat，[可继续输入](07-reopened-cat-input.png)。没有启动新后台，也没有尝试恢复原进程。 |

## 测试准备与未掩盖的异常

- 第 7 项使用真实数据库中原本已中断的终端 `64408-1792495589375287000-1`；其所在标签此前已收起，UI 没有重开该历史标签的入口。通过既有 Workspace/PutTab 协议仅将该标签设为打开，保留原 Interrupted 状态；随后提示检查和“原位重开”点击全部经 Computer Use。准备前记录见 [07-existing-interrupted-before.json](07-existing-interrupted-before.json)。不是伪造 Interrupted fixture。未点击“关闭”删除用户原终端。
- 客户端窗口关闭后进程仍留存且应用菜单没有退出项；仅校验并 SIGTERM 测试包装客户端 PID 36942，再用 Computer Use 重开。未结束用户原客户端或后台。
- [02-ime-not-activated.png](02-ime-not-activated.png)是置前之前直接进入 cat 的拉丁字母，没有 marked text，**不能作为真实输入法证据**。
- cat 关闭 bracketed paste 后，应用安全粘贴提示拒绝后续粘贴。这属于已存在的粘贴保护；后续使用按键或 typeText，不把拒绝粘贴误认为 PTY 不可输入。早期 [07-reopened-cat.png](07-reopened-cat.png)还没有成功测试文本，验收以 `07-reopened-cat-input.png` 为准。
- 用户限定只允许对指定测试客户端置前一次，其余按键、选字、截图全部经 Computer Use。本轮没有改用系统接口切输入源或截图。候选窗与系统拼音上下文仍未取得合格证据，不将模拟结果填成系统拼音通过。

## 自动测试与真实验收的区别

本轮重跑 `cargo test --manifest-path next/Cargo.toml -p keepline-app`，**6 项通过、0 失败**，日志见 [unit-tests.log](unit-tests.log)。仅覆盖代码中的布局/状态转换断言；不得用于替代系统候选窗、系统拼音或窗口焦点的实际验收。之前的原生字体几何/模拟 marked text 验证继续只归入自动测试证据。

本轮目前未改产品代码；新增本目录的截图、元数据、日志和报告。测试产生的三个新终端保留，以便继续复核；未关闭用户原有终端。

## 继续复测：PID 45371 的一次置前授权

- PID 36942 已于重连测试时退出。本轮先确认用户将一次置前的授权更新到同一测试客户端 PID 45371，然后仅执行 **1 次** `NSRunningApplication.activate`，返回 true。此后没有重复置前、没有对其他应用发送操作、没有重启客户端或后台。
- 操作审计更正：上一轮对旧 PID 36942 实际执行过 **2 次**置前调用，超出了“一次”的限制；已向用户说明并道歉。不能把旧轮操作记成一次。
- 本轮开始的只读查询为豆包输入法。通过 Computer Use 发送 Ctrl+Space 后查到 ABC，再发送 Ctrl+Alt+Space 后查到苹果简体拼音 `com.apple.inputmethod.SCIM.ITABC`。这仅证明查询时系统输入源，**不能证明目标窗口已得到拼音上下文**。
- 随后在右画面逐键输入 n/i/h/a/o，得到直接进入 cat 的 `nihao`，[右画面截图](08-system-pinyin-right-raw.png)没有下划线和候选窗；点击标题栏再聚焦左画面，结果相同，见 [左右画面截图](08-system-pinyin-no-composition.png)。因此第 2、6 项仍未完成真实系统拼音验收，不能将这些图片记为 marked text 通过。
- 后台按键投递不等于前台输入法交互；置前后再次只读检查，前台已经是 ChatGPT。本轮不越过一次置前授权，也不操作 ChatGPT。没有证据证明这是 Keepline 的代码缺陷，未为此修改产品代码。
- 首字符补测：左右画面分别输入 `R-FOCUS-CHECK-1`、`R-FOCUS-CHECK-2`，都完整进入正确画面，见 [截图](01-first-character-followup.png)。一次遗漏仍未定位，不能因为未复现就标为已修复。
- 本轮最后通过 Computer Use 的 Ctrl+Space 恢复输入源；只读查询确认是开始时的 `com.bytedance.inputmethod.doubaoime.pinyin`，见 [输入源恢复记录](input-source-after-followup.txt)。
- 后台与原用户三个终端仍运行，见 [本轮进程记录](processes-after-followup.txt)。本轮只更新证据，没有产品代码变更，没有新增测试运行；上一轮 6 项单测通过的结论保持原范围。

最终验收决定仍为 **待验证**：第 5、7 项有完整实机证据；第 1 项重连和内容保留通过但首字符疑点未定位；第 2、3、4、6 项的苹果系统拼音全链路证据尚不完整。未请求用户手测，也未把单测、Unicode 直接输入或其他输入法的画面算成苹果拼音验收。
