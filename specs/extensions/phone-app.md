# 手机 App：开工说明

状态：未开工（2026-11-02 写）。App 由用户自己写，要跨平台（iOS 和 Android）。电脑端已经做好的部分：[远程协议](remote-protocol.md)、[自建中继](relay.md)、按设备分权限、后台托管，以及 App 里的配对面板。

## 要支撑的决定

用户自己写的跨平台 App 只靠三样东西：WebSocket、Noise 加密、一行一个 JSON。这样能不能在真手机上经过公网中继完全控制电脑上的终端？完全控制指看画面、打字、按键、答审批、收状态变化。

- 能：手机端走这条路，电脑端协议冻结在 v1，之后只加字段、不改旧字段。
- 不能：看卡在哪里。卡在加密库就换框架；卡在中继延迟，见放弃条件。

## 已定的约束

- **App 里不嵌 Rust**（11-01）。所以不用 iroh，也不用 Tauri 的 Rust 层。之前 `remote-protocol.md` 里推荐的 Tauri 2 已经撤回。
- 不用 Tailscale 一类的虚拟局域网。
- 中继和手机都不可信，权限只在电脑端检查。手机按 `hello` 回复里的 `ops` 决定显示哪些按钮，这只是为了界面好看，不算安全措施。

## 框架（要你定）

| 选项 | 终端画面 | 加密 | 说明 |
|---|---|---|---|
| **React Native / Expo，终端放在 WebView 里**（建议） | xterm.js：协议的 `vt` 直接喂进去 | 纯 JS 的 @noble/curves、@noble/ciphers、@noble/hashes 已经有 X25519、ChaChaPoly、BLAKE2s；Noise 的状态机自己写，约 200 行 | xterm.js 成熟，Web 技术写界面；推送用 expo-notifications |
| Flutter | xterm.dart | Dart 的 `cryptography` 包 | 终端组件不如 xterm.js 成熟 |
| 先做 PWA（手机网页） | xterm.js | 同第一行 | 不用上架，最快能用上；iOS 后台和推送受限，可以当第 0 版 |

**没验**：上面几个库能不能对上 `vectors/v1.json`，还没试过。只有 Rust 的 snow 一种实现跑过 Noise 那部分。

## 分步

每一步都可以单独验收，做完在本文件末尾写：采用 / 否定 / 待验证。

1. **加密和帧**
   - 实现 `Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s`（prologue 为 `keepline remote 1`，手机先发），以及 DATA/ACK 帧和流量控制。DATA 单条最多 32 KiB；每收下一段就回 ACK，否则电脑端在 1 MiB 后停止发送。
   - 逐字节对照 `next/crates/keepline-remote/vectors/v1.json`，覆盖令牌、令牌哈希、预共享密钥、握手消息、加密消息、中继帧、配对链接。
   - 验收：向量全部一致；在本机起 `keepline-relay`，和真 `keeplined` 握手成功，`hello` 能拿到回复。参考实现是 `next/crates/keepline-relay/examples/phone.rs`（68 行）。
2. **配对**
   - 扫码，解析 `keepline://pair?v=2&relay=…&code=…&device=…&key=…&machine=…&name=…`。
   - 设备密钥存进 Keychain / Keystore，不放普通存储。一部手机可以配对多台机器，按 `machine` 区分。
   - 验收：电脑上 `keeplined remote --devices` 看得到这台手机；`--revoke` 之后手机收到 4401，并提示「已取消配对」，不要无限重连。
3. **列表**
   - `hello` → `list` → `events`。按机器、空间分组，显示 `phase` 和 `waiting`。
   - 收到 `lagged` 就重新 `list`。
   - 验收：电脑上开、关终端，Agent 进入等你，手机 1 秒内跟着变。
4. **终端页**
   - `watch` 收到的 `screen` 和 `output` 喂进 xterm.js；`resized` 时调整模拟器尺寸；`exited` 时显示退出码。
   - 输入走 `send`（文字，可以带回车）和 `keys`。屏幕上加一条快捷键：esc、ctrl-c、↑ ↓、tab、shift-tab、y、1、2。
   - 验收：Claude Code 和 Codex 的画面能完整还原（颜色、光标、备用屏幕）；按键顺序不乱。
5. **审批**
   - 卡片上有 `approval` 时显示允许 / 拒绝，走 `answer`。电脑端会重读画面，审批已经不在了就报错，手机要把这条错误显示出来。
   - 只看的设备不显示操作按钮。
6. **断线和回前台**
   - 指数退避重连（1 秒起，最多 30 秒）。回到前台时重新 `hello`、`list`，并对正在看的终端重新 `watch`。
   - 中继关闭码：4401 是被拒、被撤销或改了权限（先提示，不要立刻重连）；1012 是机器断开（重连）；1013 是太慢被断开（重连）。
7. **推送**（单独立项，依赖电脑端）
   - 中继代发 APNs / FCM，要你自己的开发者密钥。iOS 锁屏后连接会断，全靠推送补上。

## 验收门槛

| 指标 | 门槛 |
|---|---|
| 真手机经过公网中继，按键到回显 | p95 ≤ 300 ms |
| 同时看 4 个正在输出的终端 | 不卡顿，电脑端工作台不受影响（电脑端已经测过：一个手机不读不会拖慢别人） |
| 锁屏或切到后台再回来 | ≤ 2 s 恢复到最新画面 |
| 撤销、改权限 | 手机 ≤ 3 s 断开，并显示原因 |

## 电脑端要配合的事

- 一个公网中继：自己部署 `keepline-relay` 加 HTTPS 反向代理，或者用 relay.alma.now。两种都要你授权。
- **手机屏幕窄**：`watch` 只看，不改尺寸，120 列的终端在手机上会很小。可以先用 xterm.js 缩放加横向滚动应付。要真正适配，得做 [多客户端第三步](multi-client-events.md)（按最后操作的客户端调尺寸），会改到工作台里的画面，要单独定。
- 如果协议要加新操作（例如新开终端），在 `remote-protocol.md` 里升版本，并且在 `hello` 的 `ops` 里声明，老 App 不受影响。

## 放弃条件

- 选定的框架里，加密库一周内对不上向量 → 换框架（第一步就能暴露）。
- 真手机经公网中继，按键回显 p95 超过 300 ms，而且瓶颈在中继本身 → 重新考虑直连。直连仍然不能要求 App 里嵌 Rust。
- 只要推送和答审批就够用、平时不打字 → 停在第 5 步，不往完全控制上投入。

## 没验

- 各框架的 Noise 实现、iOS 后台的 WebSocket 存活时间、公网延迟，都没有亲手测过。
- 没和真 alma-relay 联调过（见 relay.md）。
