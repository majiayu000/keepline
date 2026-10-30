# 驱动 Keepline 终端

给 Coding Agent 看：怎样用 `keeplined` 读别的 Keepline 终端、给里面的 Agent 发话、等它做完。

## 先确认在 Keepline 里

```sh
[ "$KEEPLINE_ENV" = 1 ] || echo "不在 Keepline 终端里"
```

`KEEPLINE_ENV` 不是 `1` 就停下，告诉用户这些命令只在 Keepline 打开的终端里能用，不要猜运行目录、不要自己启动 `keeplined serve`。`KEEPLINE_TERMINAL` 是你自己所在终端的编号，别把话发给自己。

## 只动用户点名的终端

- 只读、只发给用户明确要你看或要你驱动的终端。别的终端里是用户自己的工作，没被要求就别碰。
- 拿不准是哪个，先 `list` 给用户看，问清楚再动。
- 发出去的话等于用户在那个终端里打字，收不回来。

## 命令

`<终端>` 可以是完整编号、唯一的编号前缀，或唯一的标签名；只认正在运行的终端。每个命令都接受 `--runtime <目录>`，在 Keepline 终端里不用写（读 `KEEPLINE_RUNTIME`）。

| 命令 | 做什么 |
|---|---|
| `keeplined list [--json]` | 正在运行的终端：编号、Agent、状态、标签、目录。`--json` 每项有 `id` `cwd` `tab` `agent` `phase` `session` |
| `keeplined read <终端> [--lines N]` | 画面最后 N 行，默认 50；画面下方的空行不算 |
| `keeplined send <终端> <文字\|-> [--enter]` | 把文字作为粘贴放进终端；`--enter` 再按回车。`-` 从标准输入读（最多 64 KiB），长文字不用转义 |
| `keeplined prompt <终端> <文字\|-> [--wait] [--timeout 秒] [--force]` | 给终端里的 Agent 发一条并回车 |
| `keeplined wait <终端> [--until idle\|waiting\|working] [--timeout 秒]` | 等 Agent 进入某个状态，默认 `idle`；超时默认 1800 秒 |
| `keeplined explain <终端> [--json]` | 画面规则怎么读这块屏：状态、命中的规则、看到的行 |

状态（`phase`）：`working` 在干活，`waiting` 在等用户回答（审批或选择题），`idle` 这一轮做完了，`-` 不是可识别的 Agent 对话（shell 或别的程序）。

## 要点

- `prompt` 只发给 Agent：终端里没有可识别的 Agent 对话时会拒绝，发给 shell 用 `send`。
- Agent 是 `waiting` 时 `prompt` 拒绝发送，因为你打的字会被当成回答。先 `read` 看它在问什么，把问题转给用户；用户明确让你代答时才加 `--force`。
- `prompt --wait` 发完一直等到 Agent 变成 `idle` 或 `waiting`，打印最后的状态。刚发出时状态可能还是上一轮的 `idle`，它会等到这一轮开始，最多 20 秒没动静才认 `idle`。
- 等待超时以非零退出，打印当时的状态；终端退出也会报错。
- 做完后 `read` 看结果，再向用户汇报，别只报状态。

## 例子

```sh
keeplined list
keeplined read review --lines 80
keeplined prompt review "跑一遍测试，把失败的列出来" --wait --timeout 600
keeplined read review --lines 120
printf '%s\n' "很长的说明……" | keeplined prompt 3f2a - --wait
```
