# 远程协议：多台本机、多台远程、手机端共用一套

状态：第一步完成，结论「采用」（2026-11-01）。用户 11-01 要求：
- 手机 App（跨平台）也要能控制，App 由用户自己写；
- 要支持多台本机、多台远程机器；
- 不用 Tailscale。

这件事在 10-31 排在「首个可验收本地 App」里程碑之后，用户 11-01 改为提前，并说「你可以做第一步」。

## 要支撑的决定

一套协议能不能同时满足下面两点：
- 手机 App 和另一台机器的工作台，看见一台机器上的全部终端和会话状态，并且能看画面、打字、按键、答审批；
- Keepline App 不用改，后台只多一种只看的连接方式。

如果能，后面的手机连接、桌面连远程机器都只是换传输方式，协议不动。

## 结构

```
 手机 App ─┐                           ┌─ 本机 Mac       keeplined + remote
 桌面 App ─┼── 同一套 JSON 行协议 ──────┼─ 另一台 Mac     keeplined + remote
 脚本     ─┘                           └─ Linux 服务器   keeplined + remote
             传输：① SSH 管道（已做） ② 点对点加中继（下一步选） ③ 局域网直连
```

- 每台机器有一个固定的机器 ID，存在运行目录的 `machine-id`。客户端按「机器 ID + 终端 ID」区分不同机器的终端。
- `keeplined remote` 是单独的进程，对内用本机 Unix socket 连后台。后台的信任模型不变：只认本机当前用户（peer uid）。网络层出问题，最多影响这个进程。

## 不用 Tailscale 的连接方式（手机怎么连到机器）

| 方案 | 怎么连 | 要自己运维吗 | 内容谁能看到 | 跨平台客户端 | 结论 |
|---|---|---|---|---|---|
| iroh（n0-computer，11-01 否定：要在 App 里嵌 Rust） | 按公钥拨号（扫码拿到机器的公钥），QUIC 打洞；官方说大约九成网络能直连，连不上就走中继 | 不用，可以先用 n0 的公共中继；以后自建中继，要一台有公网 IP 和域名的服务器 | 端到端加密，中继看不到内容 | 原生 Rust；官方另有 Swift、Kotlin、Python、Node 绑定 | 不采用 |
| **自建 WebSocket 中继**（Happy、alma-relay 的做法） | 机器和手机都主动连你的中继，由中继转发 | 要部署一个常年在线的服务，Rust 单文件 2.5 MB；没有服务器时可以用 alma-relay 的 Cloudflare 版 | 端到端加密（Noise），中继只看到密文 | 任何能用 WebSocket 的平台 | **采用**（11-01），见 [relay.md](relay.md) |
| 局域网直连 | 用 Bonjour 发现机器，在同一个 Wi-Fi 下直接连 | 不用 | 只在局域网里，再加令牌和 TLS | 都行 | 顺手做，出门就不能用 |
| Cloudflare Tunnel / ngrok / frp | 把本机端口暴露成公网地址 | 要账号或服务器 | 隧道方能解密（Cloudflare 能看到明文） | 都行 | 不推荐：等于开公网入口 |
| WebRTC | 先通过信令服务器交换地址，再点对点直连 | 要信令服务器和 TURN 服务器 | 端到端加密 | 都有，但接起来最重 | 不推荐：iroh 已经把打洞和中继做好了 |
| Headscale / ZeroTier / NetBird | 自建或第三方虚拟局域网 | 视方案而定 | WireGuard 加密 | 要装客户端 | 和 Tailscale 同一类，按用户意思不选 |

跨平台 App 的做法改看 [phone-app.md](phone-app.md)（11-02）。原先建议的 Tauri 2 和 iroh 都要在 App 里嵌 Rust，和 11-01「App 里不嵌 Rust」的决定冲突，已撤回。

## 协议 v1（第一步已实现）

传输层只负责一件事：两边各送一串 UTF-8 的 JSON，一行一个对象，每行不超过 1 MiB。
- SSH 下就是 `ssh 主机 keeplined remote --stdio` 的标准输入和输出。
- 经中继时，同样的字节放进 Noise 加密通道里传（见 [relay.md](relay.md)），每条消息的内容不变。

### 请求（客户端 → 机器）

每个请求都带一个数字 `id`，第一个必须是 `hello`。普通请求按发出的顺序一个接一个执行，所以先发文字、再发回车，到达终端的顺序也不会乱。

| op | 字段 | 回复 `ok` |
|---|---|---|
| `hello` | `version`（可选） | `{version: 1, machine: {id, name}, ops: [...]}` |
| `list` | — | `{machine, spaces: [{id, name, cwd}], terminals: [卡片]}` |
| `read` | `terminal`, `lines`（默认 50，最多 2000） | `{text}`：画面最后几行纯文字 |
| `send` | `terminal`, `text`, `enter` | `{}`：文字按粘贴放进去；`enter` 为真就再按回车 |
| `keys` | `terminal`, `keys: [...]` | `{}`：按键名见下；一批里有一个不认识，就一个都不发 |
| `answer` | `terminal`, `question`, `allow` | `{}`：答审批。先重读画面，审批不在了、或光标不在第一个选项上，就报错，不敲键 |
| `chat` | `terminal` 或 `session`，`after` / `before`（可选），`limit`（默认 50，最多 500） | `{session, messages: [{seq, at, role: user\|agent, text}], more}`：一段对话里用户和 Agent 说的话，从旧到新。不带 `after` / `before` 时给最新的 `limit` 条；`after` 给比这个 `seq` 新的，`before` 给更旧的；`more` 表示更前面还有。工具调用和输出不在里面（11-02 加，手机聊天用） |
| `events` | — | `{stream}`，之后持续推事件 |
| `watch` | `terminal` | `{stream, cols, rows}`，之后持续推画面 |
| `close` | `stream` | `{}`：结束那个流 |

终端卡片：
```
{id, space, tab, cwd,
 phase: working|waiting|idle|shell,
 agent, session, title, approval, waiting}
```

按键名：`enter` `esc` `tab` `shift-tab` `backspace` `space` `up` `down` `left` `right` `home` `end` `page-up` `page-down` `delete`，`ctrl-a` 到 `ctrl-z`，以及任意单个字符（例如 `1`、`y`）。方向键按程序当前的光标键模式（DECCKM）发送，和本机敲键盘一样。

出错时回复 `{id, error: "原因"}`。不是 JSON、没有 `id` 的行，回复 `{error}`，连接不断。

### 流（机器 → 客户端）

流里的每条消息都带 `stream`，值就是打开它的那个请求的 `id`。

- `events`：`{stream, type: "event", event: Notice}`。
  - 先推一份现状，之后有变化就推。事件和 `keeplined events --json` 是同一套：started、waiting、error、done、approval、approval_gone、terminal_started、terminal_exited、command_done、command_failed、lagged。
  - 收到 `lagged` 说明中间丢了事件，客户端要重新 `list` 一次。
- `watch`：
  - `{type: "screen", cols, rows, vt}`：整屏。`vt` 是 base64 编码的转义序列，喂给一个干净的终端模拟器就能还原画面，包括光标、模式（括号粘贴、光标键模式）、颜色和备用屏幕。客户端跟不上被跳过一段时，会再收到一次 `screen`。
  - `{type: "output", vt}`：之后的原始输出，接着喂进去就行。
  - `{type: "resized", cols, rows}`：别的客户端（例如工作台）改了尺寸。
  - `{type: "exited", code}`：程序退出了。
- 流结束时：`{stream, type: "end", reason}`。客户端自己关掉的，`reason` 是 `"closed"`。

`watch` 只看：不会成为可写的那一方，也不会改终端尺寸；工作台关掉后也不会被它接管。手机打字和按键走 `send` 和 `keys`，和工作台里正在打字的人并排写进终端，跟 `keeplined send` 一样。

### 后台新增的两处

- `Attach { watch: true }`：只看的连接，永远不会被选为写入者。旧版后台会忽略这个字段。
- `Request::SendKeys { id, keys }`：按键名转成字节，并且看程序当前的光标键模式。

## 第一步验收

| 指标 | 门槛 | 结果 |
|---|---|---|
| 看、打字、按键、事件，全部走 stdio | 端到端用例全部通过 | `remote_watches_and_drives_a_terminal_without_taking_or_resizing_it` 通过：hello 之前的请求被拒；list；watch 回放出「ready」，并带上光标键模式；坏按键整批不发；up/esc/ctrl-c/y 加上文字 hi 按顺序到达；events 推出 terminal_started 和 terminal_exited；close 后收到 `end: closed`；断开后进程退出；重启后机器 ID 不变 |
| 只看不接管、不改尺寸 | 工作台离开后，下一个工作台不用接管就能写；一条 resized 都没有 | 同上，通过 |
| 按键到画面回来 | ≤ 200 ms | 本机 stdio 0.7 ms；**经过真实 ssh 一跳**（ssh localhost），20 次按键 p95 0.2 ms，hello 含 ssh 建连 177 ms |
| 手机端不读的时候 | 工作台不受影响 | `a_stalled_remote_watcher_slows_nobody_and_gets_the_screen_again`：40 万行输出到工作台，单独 342–358 ms，旁边挂一个不读的远程只看连接时 352–392 ms，连跑 6 次；远程连接恢复读取后，收到含最新内容的整屏 |
| 空闲时的开销 | 没有远程连接时零开销 | 后台只在有只看连接时多一次集合过滤；远程进程只是阻塞读，不轮询 |

## 放弃条件

- ~~第二步（iroh）试下来连不稳就退回自建中继~~：11-01 直接改成自建中继，这一条不再适用。
- 手机 App 和桌面多机各自的放弃条件见 [phone-app.md](phone-app.md)、[multi-machine.md](multi-machine.md)。

## 下一步（11-02 更新）

1. **手机怎么连**：已定，并完成了第一版（11-01）。
   - 用自己写的 Rust 中继，加两端 Noise 加密，协议沿用 alma-relay，详见 [relay.md](relay.md)。
   - iroh 不做了：它要求手机 App 里嵌 Rust，用户不接受。
2. **配对和权限**：已完成（11-01），见 [relay.md](relay.md)。SSH 管道下不需要，SSH 本身就是认证。
3. **手机 App**：用户自己写，开工说明见 [phone-app.md](phone-app.md)。
4. **桌面工作台连远程机器**：开工说明见 [multi-machine.md](multi-machine.md)。
