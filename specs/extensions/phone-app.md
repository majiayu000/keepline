# 手机 App：开工说明

状态：电脑端的聊天接口已完成（`chat`，2026-11-02），App 未开工。App 要跨平台（iOS 和 Android）。电脑端已经做好的部分：[远程协议](remote-protocol.md)、[自建中继](relay.md)、按设备分权限、后台托管，以及 App 里的配对面板。

## 11-02 用户定的方向

- **手机上不做终端，只做聊天。** 看 Agent 说了什么，回一句话，答审批。看画面、按方向键这类操作留在电脑上。
- 框架、做法按建议来：React Native（Expo）。不嵌 Rust，也不用 Tauri。不用 Tailscale。

## 要支撑的决定

离开电脑时，只靠手机聊天能不能让 Agent 不白等：
- 看到哪个会话在等你、它问了什么；
- 读到它最近说的话；
- 回一句、答一个审批，它接着干活。

能的话，手机端就停在聊天，不往终端画面上投入。

## 已定的约束

- App 里不嵌 Rust：不用 iroh，也不用 Tauri 的 Rust 层。
- 不用 Tailscale 一类的虚拟局域网。
- 中继和手机都不可信，权限只在电脑端检查。手机按 `hello` 回复里的 `ops` 决定显示哪些按钮，这只是为了界面好看，不算安全措施。

## 手机端用到的协议

| 做什么 | 用哪个操作 |
|---|---|
| 会话列表 | `list`：每张卡片有 `phase`（working / waiting / idle / shell）、`agent`、`title`、`waiting`（它在问什么）、`approval`（屏幕上的审批问题） |
| 状态变化 | `events` 流：started、waiting、done、approval、approval_gone 等；收到 `lagged` 就重新 `list` |
| 聊天记录 | `chat`：先取最新 50 条；往上翻页用 `before`；有新消息用 `after` |
| 发一句话 | `send`，带 `text` 和 `enter: true`。Agent 正在忙时，Claude Code 会把它排进队列 |
| 审批 | `answer`，带 `question`（取卡片上的 `approval`）和 `allow` |
| 选项题（例如 AskUserQuestion） | `keys`，发 `["1"]` 一类的按键；问题文字在卡片的 `waiting` 里 |

`watch` 和 `read` 手机用不到。

**新消息怎么来**：`events` 只在状态变化时推送，Agent 一轮里中途说的话不会触发。所以聊天页打开时每 2 秒发一次 `chat after=<最后一条 seq>`。这是一次 SQLite 查询，开销很小。以后如果要省电，可以在电脑端加一个推送新消息的流。

**`chat` 不包含的内容**：工具调用和工具输出（改了哪些文件、跑了什么命令）。手机上先只显示对话。要看这些，以后可以加 `tools: true`。

## 分步

每一步都可以单独验收，做完在本文件末尾写：采用 / 否定 / 待验证。

1. **加密和帧**
   - 用纯 JS 实现 `Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s`（prologue 为 `keepline remote 1`，手机先发），以及 DATA/ACK 帧和流量控制。库用 @noble/curves、@noble/ciphers、@noble/hashes。
   - DATA 单条最多 32 KiB；每收下一段就回 ACK，否则电脑端在 1 MiB 后停止发送。
   - 逐字节对照 `next/crates/keepline-remote/vectors/v1.json`。
   - 验收：向量全部一致；在本机起 `keepline-relay`，和真 `keeplined` 握手成功，`hello` 能拿到回复。参考实现是 `next/crates/keepline-relay/examples/phone.rs`（68 行）。
2. **配对**
   - 扫码，解析 `keepline://pair?v=2&relay=…&code=…&device=…&key=…&machine=…&name=…`。
   - 设备密钥存进 Keychain / Keystore（expo-secure-store）。一部手机可以配对多台机器，按 `machine` 区分。
   - 验收：电脑上 `keeplined remote --devices` 看得到这台手机；`--revoke` 之后手机收到 4401，并提示「已取消配对」，不要无限重连。
3. **会话列表**
   - 等你的置顶；按机器分组，其余按最近活动排序。每行显示标题、Agent、状态和一行原因。
   - 验收：电脑上 Agent 进入等你，手机 1 秒内置顶。
4. **聊天页**
   - 用气泡显示 `chat` 的内容，Agent 的回复按 Markdown 渲染；往上滑加载更早的。底部是输入框，走 `send`。
   - 顶部条：有 `approval` 时显示「允许 / 拒绝」；有 `waiting` 时显示问题，有选项就给出按钮。
   - 只看的设备不显示输入框和按钮；能答审批的设备只显示审批按钮。
   - 验收：在手机上发一句话，电脑上的 Claude Code / Codex 收到并开始干活，回复 3 秒内出现在手机上。
5. **断线和回前台**
   - 指数退避重连（1 秒起，最多 30 秒）。回到前台时重新 `hello`、`list`，聊天页用 `after` 补上中间的消息。
   - 中继关闭码：4401 是被拒、被撤销或改了权限（先提示，不要立刻重连）；1012 是机器断开（重连）；1013 是太慢被断开（重连）。
6. **推送**（单独立项，依赖电脑端）
   - 中继代发 APNs / FCM，要你自己的开发者密钥。iOS 锁屏后连接会断，「有会话在等你」要靠推送送到。

## 验收门槛

| 指标 | 门槛 |
|---|---|
| 手机发一句话，到电脑终端收到 | p95 ≤ 300 ms（真手机经公网中继） |
| Agent 回复，到手机显示 | ≤ 3 s（2 秒轮询加网络） |
| 锁屏或切到后台再回来 | ≤ 2 s 补齐消息 |
| 撤销、改权限 | 手机 ≤ 3 s 断开，并显示原因 |

## 电脑端要配合的事

- 一个公网中继：自己部署 `keepline-relay` 加 HTTPS 反向代理，或者用 relay.alma.now。两种都要你授权。
- 推送（第 6 步）。
- 不需要再做手机上的终端画面，所以「按最后操作的客户端调尺寸」对手机不再是前提。

## 放弃条件

- JS 的 Noise 实现一周内对不上向量 → 改用各平台原生的加密库，通过 Expo 模块接进来。
- 真手机经公网中继，按键回显 p95 超过 300 ms，而且瓶颈在中继本身 → 重新考虑直连。直连仍然不能要求 App 里嵌 Rust。
- 2 秒轮询明显耗电，或者回复延迟让人觉得慢 → 在电脑端加推送新消息的流，不再轮询。

## 没验

- JS 的 Noise 实现、iOS 后台的 WebSocket 存活时间、公网延迟，都没有亲手测过。
- 没和真 alma-relay 联调过（见 relay.md）。
- `chat` 只在测试里用 Claude 格式的日志验过；Codex 的消息走的是同一张表（`session_records`），但没单独测。

## 结果

### `chat` 接口（2026-11-02）：采用

- 后台新增 `Request::Chat`，从索引的 `session_records` 表读用户和 Agent 的消息，`seq` 用的是行号。远程协议加了 `chat` 操作，可以按终端或会话取，三档权限都能用。
- 测试 `remote_chat_pages_a_conversation_by_terminal_and_session`，经过 `keeplined remote --stdio` 验证了：
  - 最新一页的顺序和角色正确，`more` 为真；
  - `before` 能取到更早的，取完后 `more` 为假；
  - 日志追加一条回复后，`after` 只返回这一条；
  - 不带 `terminal` / `session` 时报错。
- 相邻的远程测试和中继测试照常通过：`remote_watches…`、`a_stalled_remote_watcher…`、两个中继端到端测试。
- 已知限制：日志被重新索引（换文件、截断）时 `seq` 会变。客户端发现 `after` 取回的 `seq` 比手上的小，或者取不到，就重新取最新一页。

### 第 1 步：加密和帧（2026-11-02）：采用

- App 放在 `next/mobile`，用 Expo SDK 58、React Native 0.88、TypeScript。加密层在 `src/remote/`：`noise.ts`（握手、DATA/ACK）、`link.ts`（配对链接、令牌、中继地址）、`client.ts`（WebSocket、按 id 对应请求、流、ACK）。只用纯 JS 的 @noble 库，没有原生模块。
- `bun test`：`vectors/v1.json` 里的码、令牌、令牌哈希、PSK、两条握手消息、三条加密消息、中继帧、配对链接，全部逐字节一致；改错密钥、篡改、重放都会被拒。这是 Noise 部分第一次有第二种实现交叉验证。
- 端到端测试 `the_js_phone_client_chats_through_a_real_relay`（`cargo test -p keeplined --test managed_session -- --ignored the_js_phone`，需要 bun）：起真的 `keeplined` 和 `keepline-relay`，配对一台 full 权限的设备，JS 客户端经中继完成握手（本机 25 ms），收到 hello、list、events，读到对话。手机发的话原样进了终端，Agent 的回复 377 ms 后经 `chat after` 读回。
- 还要处理：React Native 的 Hermes 没有 `crypto.getRandomValues`，第 2 步接 App 时用 expo-crypto 补上。
