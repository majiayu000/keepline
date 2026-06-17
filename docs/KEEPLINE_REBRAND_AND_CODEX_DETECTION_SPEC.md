# Keepline Rebrand and Codex Detection Spec

## 背景

项目原名为 Claude Hub，核心能力是扫描 Claude Code 会话、进程、成本和恢复状态。当前目标是把产品主品牌升级为 Keepline，并新增 Codex CLI 会话检测与恢复。此版本不保留旧品牌兼容入口、旧品牌环境变量或旧品牌数据目录 fallback。

GitHub 跟踪 issue: https://github.com/majiayu000/claude-hub/issues/34

## 目标

1. 产品主品牌、CLI 主命令、Web 标题、README 和主要用户可见文案改为 Keepline。
2. CLI 只暴露 `keepline` 主命令，不保留 `claude-hub` 或 `codex-hub` alias。
3. 会话模型增加 `client` 字段，用于区分 `claude` 与 `codex`。
4. 扫描 `~/.codex/sessions/**/rollout-*.jsonl`，解析 Codex 会话元数据、消息、工具调用和当前文件。
5. 进程扫描同时检测 Claude Code 和 Codex CLI 主进程，并过滤 Codex.app、app-server、chronicle、node_repl 等非交互会话 helper。
6. 同目录多客户端会话不能互相匹配错误进程。
7. API、Web UI、内嵌终端和 recovery flow 都能处理 Codex session。
8. 数据目录固定为 `~/.keepline`，仅支持 `KEEPLINE_HOME` 显式覆盖。

## 非目标

1. 不移除 Claude Code 支持。
2. 不把 Claude hooks、Claude plans、Claude usage 的内部实现一次性重写为通用 provider 架构。
3. 不在本 PR 内完成 GitHub 仓库真实 rename 或 npm package 发布。

## 用户体验

### CLI

主命令改为：

```bash
keepline
keepline list
keepline watch
keepline recover <session-id>
keepline web
```

旧命令 `claude-hub` / `codex-hub` 不再作为 bin 暴露。

### Web

1. 页面标题为 Keepline。
2. Header 显示 Keepline。
3. Session card 展示 client badge: `Claude` 或 `Codex`。
4. 搜索框支持搜索 `claude` / `codex`。
5. Terminal tab 的历史会话列表展示 client tag，并按 client 选择 resume 命令。

## 数据模型

### Session

新增字段：

```ts
type AgentClient = 'claude' | 'codex'

interface Session {
  sessionId: string
  client: AgentClient
}
```

### 数据库迁移

新增 migration `006_session_client`：

```sql
ALTER TABLE sessions ADD COLUMN client TEXT NOT NULL DEFAULT 'claude';
CREATE INDEX IF NOT EXISTS idx_sessions_client ON sessions(client);
```

旧数据默认视为 Claude sessions。

### Codex Session ID

Codex 的原始 session id 是 UUID。为了避免与 Claude session id 共享同一个 `session_id` 唯一空间，持久化时使用 scoped id：

```text
codex_<uuid>
```

恢复命令需要去掉前缀：

```bash
codex resume <uuid>
```

## Codex 文件扫描

扫描根目录：

```text
~/.codex/sessions/
```

匹配文件：

```text
**/rollout-*.jsonl
```

解析规则：

1. `session_meta.payload.id` 是原始 Codex session id。
2. `session_meta.payload.cwd` 是工作目录。
3. `response_item.payload.type === "message"` 计入 message。
4. user 的 `input_text` 作为 `firstMessage`。
5. assistant 的 `output_text` 作为 `lastMessage`。
6. `response_item.payload.type === "function_call"` 计入 tool。
7. function arguments 支持 JSON string 和 object。
8. `path`、`file_path`、`filePath`、`notebook_path` 提取为 current file。

## 进程检测

扫描命令：

```bash
ps -eo pid,pcpu,pmem,tty,lstart,args | grep -Ei '[c]laude|[c]odex'
```

保留：

1. `claude`
2. `claude-code`
3. `codex` CLI 主进程

过滤：

1. `/Applications/Codex.app/`
2. `codex app-server`
3. `codex_chronicle`
4. `node_repl`
5. `chrome-native-host`
6. shell wrapper，如 `/bin/zsh -lc codex`

匹配规则：

1. 先按 `client + cwd` 分组。
2. 同组内优先 PID 连续性。
3. 再按 session start time 与 process start time 的距离匹配。
4. 最近活动作为 tie-breaker。

## Recovery

Claude:

```bash
claude --resume <session-id>
claude --continue
claude <initial-prompt>
```

Codex:

```bash
codex resume <uuid>
codex resume --last
codex <initial-prompt>
```

Codex unsafe flag:

```bash
--dangerously-bypass-approvals-and-sandbox
```

Claude unsafe flag:

```bash
--dangerously-skip-permissions
```

Terminal allowlist 必须包含 `codex`。

## API

### `GET /api/sessions`

新增 query:

```text
client=claude|codex
sort=client
```

返回 session 包含：

```json
{
  "client": "codex"
}
```

### `GET /api/sessions/:id/tools`

`codex_` 前缀 session 走 Codex parser，否则走 Claude parser。

### `GET /api/sessions/:id/usage`

Codex 暂无 token/cost 明细时返回 0 统计。

### `GET /api/sessions/:id/full`

Codex sessions 返回 tools/details；sub-agents 为空。Claude sessions 保留原 sub-agent 逻辑。

## 存储迁移

新安装默认数据目录：

```text
~/.keepline
```

环境变量：

```text
KEEPLINE_HOME
```

解析优先级：

1. `KEEPLINE_HOME`
2. `~/.keepline`

不读取 `CLAUDE_HUB_HOME`、`CODEX_HUB_HOME`、`~/.claude-hub`、`~/.codex-hub` 或 `~/.tasker`。不自动移动旧目录或改写旧文件名。

## 测试要求

1. TypeScript:

```bash
bun run typecheck
```

2. Parser:

```bash
bun test src/__tests__/codex.parser.test.ts
```

3. Process parser:

```bash
bun test src/__tests__/process-parser.test.ts
```

4. Process matching:

```bash
bun test src/__tests__/session.process-matcher.test.ts
```

5. Recovery:

```bash
bun test src/__tests__/recovery-security.test.ts
```

6. Full suite before merge:

```bash
bun test
```

## 验收标准

1. `keepline --help` 显示 Keepline 说明。
2. `claude-hub --help` 和 `codex-hub --help` 不作为本包 bin 提供。
3. `bun run typecheck` 通过。
4. `bun test` 通过，或明确记录非本 PR 引入的失败。
5. 数据库 migration 后旧 sessions 的 `client` 为 `claude`。
6. 本机存在 Codex JSONL 时，sync 后能看到 `client: "codex"` 的 session。
7. Codex session card 可以展开查看 first message、last response、tool calls。
8. Codex lost session recovery command 使用 `codex resume <raw uuid>`。
9. 同一个 cwd 下的 Claude 和 Codex 进程不会互相抢占 session match。
10. README 和 Web 首屏只显示 Keepline 作为产品主名。

## 后续事项

1. GitHub repository 从 `claude-hub` rename 到 `keepline`。
2. npm 发布 `keepline`。
3. 把 Claude-specific plans/hooks/usage 模块逐步抽象为 multi-client provider。
4. 增加 Codex usage/cost 解析能力，如果 Codex CLI 后续公开稳定 token schema。
