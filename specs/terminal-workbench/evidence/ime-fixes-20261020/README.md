# 中文输入法、重启状态与宽字符修复（2026-10-20）

本轮决定：采用光标处的本地组字覆盖层、显式中断占位、按两格适配宽字形；不增加产品功能，不提交、不推送。
验收目标是组字绘制与候选窗坐标一致、已中断终端不再 attach、连续中日韩字符无额外格间留缝。
若原生测量或实际渲染位置不一致，不以单元测试通过代替验收。

用户此前人工确认：系统拼音选字上屏、回车上屏字母、退格正常。本轮修改后仍须复测这些路径及候选窗定位。

## 实现与状态归属

- `pane.rs`：marked text 只在客户端内存中；不经过 Input/PTY。光标处覆盖绘制、下划线、完整单元格背景遮罩；组字时隐藏 VT 光标。标记变更/取消/提交立即请求重绘，没有周期计时器。
- `screen.rs`：`CellMetrics` 是绘制、PTY resize、组字和 `bounds_for_range` 的同一个实际字体测量。IME 范围按 UTF-16 转换成字节索引，再使用实际 shaping 的横向位置。返回行高范围，macOS 可将候选窗贴在行下方。保留最近一次已知光标；从未获知位置时返回 None，禁止假定 (0,0)。光标可见性与位置分离。
- `workbench.rs`：在构造 Pane/Connection 之前过滤 Interrupted。占位提示为“已中断（后台重启，原进程已结束）”，提供“在这里重新开一个终端”和“关闭”。重开创建新 PTY，保留原空间、工作目录及格子位置；关闭只移除格子。旧 Terminal/AgentSession 历史与结局不删除。
- `theme.rs`：删除 `CELL_WIDTH_FALLBACK`，候选定位与绘制不再依赖备用字宽。
- `main.rs`、`visual-20261020/capture-native.py`：仅扩展既有验收路径；原生字体测量、真实 EntityInputHandler 的范围与渲染帧下划线位置交叉检查，支持隔离后台重启 fixture。普通构建不包含这些验收逻辑。

## 逐条证据

- [x] 隐藏光标仍能定位；暂时没有 viewport 时保留上次位置：`hidden_cursor_keeps_ime_anchor_and_missing_position_keeps_last_known`。
- [x] UTF-16 范围不误用 UTF-8 字节，不拆代理对：`ime_offsets_are_utf16_not_utf8_and_never_split_surrogates`。
- [x] 中日韩字符每个占两列，组合字符不混入 ASCII run：`captured_cjk_occupies_two_columns_and_combining_text_stays_separate`。
- [x] Interrupted 不进入连接构造列表：`interrupted_terminals_never_enter_the_connection_creation_list`。
- [x] 四格满标签仍可原位重开，保留历史：`reopening_full_tab_replaces_exact_slot_and_preserves_history`。
- [x] 关闭只移除摆放：`closing_interrupted_pane_only_removes_its_placement`。
- [x] 真实 PTY 的两段对话在后台重启后仍保持独立归属：既有集成测试 `two_cli_conversations_in_one_real_pty_keep_separate_ownership_after_restart` 本轮复跑通过。
- [x] 原生 Menlo 实测单格 7.525635 × 20 logical px；中日韩 advance 15.051269 px，与两格 15.051270 px 一致；emoji 居中且无越格。不是 GPUI mock 字体。
- [x] 实际 Pane 的组字整体范围 (261,385,37.628178,20)，请求 2..2 范围返回 (276.05127,385,0,20)；渲染帧中下划线也在该范围，非 pane 底部。[组字与宽字符原生图](ime-native.png)。
- [x] 隔离后台真实重启后新客户端显示中断提示和两按钮。[中断原生图](interrupted-native.png)。
- [x] 普通 `keepline-app` 构建通过；详见 [本轮日志](validation.log)。现有 GPUI macOS 依赖的 deprecated 警告仍存在。
- [ ] 系统拼音候选窗的实际贴合、选字/回车/退格在本轮构建上的人工复测，尚未替用户确认。

## Ghostty 对照

直接检查本地编译所用 Ghostty commit `3425025e585a3403340d4dc6d65132f42e05e605`：
`src/font/main.zig` 的默认回退字体调整为 `ic_width`；`src/font/Collection.zig::scaleFactor` 按字体指标调整；
`src/font/face/coretext.zig` 在字形约束后居中。GPUI 的回退字体由 shaping 选择，本实现把宽字形的 shaped advance 等比适配两格，并限制在行高内，再居中；不改 ASCII 字号，不声称逐像素复制 Ghostty 字体栅格器。

## 重跑

从实现 worktree 执行，构建环境同 `next/run.sh`（ZIG、ReleaseFast、Xcode）：

```sh
cargo test --manifest-path next/Cargo.toml -p keepline-app
cargo test --manifest-path next/Cargo.toml -p keeplined --test managed_session two_cli_conversations_in_one_real_pty_keep_separate_ownership_after_restart -- --nocapture
cargo build --manifest-path next/Cargo.toml -p keepline-app --features visual-test
KEEPLINE_VISUAL_OUT=/tmp/keepline-ime-native python3 specs/terminal-workbench/evidence/visual-20261020/capture-native.py
KEEPLINE_VISUAL_OUT=/tmp/keepline-interrupted-native KEEPLINE_VISUAL_RESTART=1 python3 specs/terminal-workbench/evidence/visual-20261020/capture-native.py
cargo build --manifest-path next/Cargo.toml -p keepline-app
```

fixture 使用临时目录、模拟 CLI 和真实 PTY，不读取用户 Agent 历史，不触碰用户正在运行的后台。原生导出图为 1440×900 logical px、Retina 2×；组字验收明确调用 GPUI draw 生成新帧后再导出并检查下划线原语；不依赖窗口显示链路调度或固定等待，不会把上一帧当作证据。
