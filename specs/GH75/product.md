# Hook Pipeline Recovery Product Spec

Issue: https://github.com/majiayu000/keepline/issues/75

## Summary

Keepline 的 Claude Code hook 安装必须让实时监控和跨会话上下文入口实际收到 Claude Code 事件。安装后，Claude Code 的 `PreToolUse`、`PostToolUse`、`Notification`、`Stop`、`UserPromptSubmit` 事件应全部转发到本地 hook server。

## User Problem

用户安装 hooks 后预期看到实时会话状态、工具活动、Stop 自动完成和首次 prompt 上下文注入。如果安装器只注册 `PostToolUse`，或依赖 Claude Code 不提供的环境变量拼接 payload，功能会静默失效：

1. `Stop` 不会把会话标记为 completed。
2. `UserPromptSubmit` 不会触发上下文注入。
3. 工具事件无法稳定携带 session、cwd、tool input 和 tool response。
4. `|| true` 会隐藏 hook 命令失败，用户只看到实时功能退化。

## Product Behavior

1. `keepline hooks install` 写入 Claude Code 当前 hook schema。
2. 同一个 Keepline command hook 注册到 `PreToolUse`、`PostToolUse`、`Notification`、`Stop`、`UserPromptSubmit`。
3. Command hook 从 stdin 接收 Claude Code JSON，并原样 POST 到 Keepline hook server。
4. Hook server 接受 Claude Code 官方字段：`hook_event_name`、`session_id`、`cwd`、`tool_name`、`tool_input`、`tool_response`、`prompt`、`message`。
5. Hook server 继续兼容旧 Keepline payload：`event_type`、`timestamp`、`tool_output`。
6. 无效 payload 必须返回 400，不得被当作有效事件降级处理。

## Non-Goals

- 不在本 issue 引入 hook shared secret 或 Host/Origin 校验；安全加固由 GH79 覆盖。
- 不改变 compression queue 或 memory extractor 的内部事件 contract。
- 不要求 hook server 在 `keepline web` 默认启动；daemon/web 启动边界由其他 issue 覆盖。
- 不移除旧 hook ownership detection；旧安装需要可升级。

## Acceptance Criteria

1. 安装器不再依赖 `$CLAUDE_EVENT_TYPE`、`$CLAUDE_SESSION_ID`、`$CLAUDE_TOOL_NAME`、`$CLAUDE_TOOL_INPUT`。
2. 安装器注册全部五类已消费事件，而不只是 `PostToolUse`。
3. `PostToolUse` 官方 `tool_response` 能被转为内部 `tool_output` 并进入现有 tool event/压缩路径。
4. `UserPromptSubmit` 官方 payload 能通过 validation 并触发现有 prompt handling。
5. 旧 Keepline payload 仍可被 hook server 接受。
6. 回归测试覆盖安装结构、legacy 升级、官方 payload normalization 和 malformed payload 拒绝。
7. 验证命令通过：focused hook tests、`bun run typecheck`、`bun test`、`bun run build`。
