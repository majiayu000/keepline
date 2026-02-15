# Claude Code Autonomous Runner

让 Claude Code 24小时自主运行的最稳定方案。

## 核心设计

```
┌────────────────────────────────────────────────────────────┐
│                    Orchestrator                             │
│              (轻量级调度器，永远不会崩溃)                     │
│    • 健康检查   • 成本监控   • 崩溃恢复   • 智能终止         │
├────────────────────────────────────────────────────────────┤
│                    Memory Layer                             │
│              (Markdown 文件，跨会话共享)                      │
│    TASKS.md → CONTEXT.md → DONE.md                          │
├────────────────────────────────────────────────────────────┤
│                    Worker (Docker)                          │
│              (隔离执行，单次任务后退出)                       │
│    Claude Code + --dangerously-skip-permissions             │
└────────────────────────────────────────────────────────────┘
```

## 快速开始

### 1. 确保已登录 Claude Code

```bash
# 如果还没登录，先运行一次
claude
```

### 2. 编辑任务

```bash
vim memory/TASKS.md
```

添加你的任务：
```markdown
## 高优先级
- [ ] 为 src/auth.ts 添加单元测试
- [ ] 修复登录页面的类型错误

## 中优先级
- [ ] 重构 UserService 类
```

### 3. 编辑上下文

```bash
vim memory/CONTEXT.md
```

描述你的项目，帮助 Claude 理解背景。

### 4. 启动

```bash
chmod +x *.sh
./start.sh
```

## 命令

```bash
./start.sh                    # 启动（按 Ctrl+C 停止）
./start.sh --max-cost 5       # 限制成本 $5
./start.sh --max-iterations 10 # 限制 10 次迭代
./start.sh --no-docker        # 不使用 Docker（调试用）

./stop.sh                     # 停止运行
./status.sh                   # 查看状态
```

## 配置

编辑 `config.yaml`：

```yaml
# 安全限制
max_iterations: 100      # 最大迭代次数
max_cost_usd: 10.0       # 最大成本 (美元)
max_duration_hours: 4.0  # 最大运行时长
consecutive_no_progress: 3  # 连续无进展后停止

# 执行配置
use_docker: true         # 是否使用 Docker 隔离
cooldown_seconds: 10     # 迭代间隔
```

## 文件结构

```
autonomous-runner/
├── orchestrator.py      # 主控制器
├── config.yaml          # 配置文件
├── start.sh             # 启动脚本
├── stop.sh              # 停止脚本
├── status.sh            # 状态脚本
├── worker/
│   └── Dockerfile       # Worker 镜像
├── memory/              # 外部记忆（Claude 读写）
│   ├── TASKS.md         # 任务队列
│   ├── CONTEXT.md       # 项目上下文
│   └── DONE.md          # 完成历史
├── workspace/           # 工作目录（代码在这里）
└── logs/                # 运行日志
```

## 工作原理

### 为什么这样设计？

| 设计决策 | 解决的问题 |
|---------|-----------|
| Worker 短命 | Context window 永不耗尽 |
| Docker 隔离 | 安全 + 崩溃不影响主系统 |
| Markdown 记忆 | 人可读、可编辑、可版本控制 |
| 多重终止条件 | 防止无限循环和成本失控 |
| 无进展检测 | 防止重复劳动 |

### 执行流程

```
Orchestrator 启动
     ↓
┌→ 检查是否继续 (成本/时间/迭代/任务)
│    ↓ (继续)
│  启动 Docker Worker
│    ↓
│  Worker 读取 TASKS.md + CONTEXT.md
│    ↓
│  Worker 执行任务
│    ↓
│  Worker 更新 .md 文件
│    ↓
│  Worker 退出
│    ↓
│  Orchestrator 检测进展
│    ↓
│  冷却等待
│    ↓
└──────────────────┘
     ↓ (停止)
打印摘要，退出
```

## 高级用法

### 把代码放入 workspace

```bash
# 方式1: 克隆到 workspace
git clone https://github.com/your/repo workspace

# 方式2: 软链接已有项目
ln -s /path/to/your/project workspace
```

### 后台运行

```bash
# 使用 tmux
tmux new -s claude
./start.sh
# Ctrl+B, D 分离

# 使用 nohup
nohup ./start.sh > /dev/null 2>&1 &
```

### 监控日志

```bash
# 实时查看日志
tail -f logs/orchestrator_*.log

# 查看状态
./status.sh
```

## 安全说明

1. **Docker 隔离**：Worker 在容器中运行，无法访问宿主机
2. **成本限制**：`max_cost_usd` 硬限制 API 支出
3. **时间限制**：`max_duration_hours` 防止无限运行
4. **无进展检测**：连续无变化自动停止
5. **优雅退出**：Ctrl+C 会完成当前任务后停止

## 常见问题

### Q: 如何查看 Claude 做了什么？

```bash
# 查看完成历史
cat memory/DONE.md

# 查看日志
tail -100 logs/orchestrator_*.log
```

### Q: 如何干预正在运行的任务？

1. 直接编辑 `memory/TASKS.md` 添加紧急任务
2. 下一次迭代 Claude 会看到新任务

### Q: 成本如何计算？

目前通过 Claude Code 的 JSON 输出解析成本。如果解析失败，建议监控 Anthropic 控制台。

### Q: 不使用 Docker 安全吗？

不建议。`--dangerously-skip-permissions` 允许 Claude 执行任意命令，Docker 隔离是必要的安全措施。

## License

MIT
