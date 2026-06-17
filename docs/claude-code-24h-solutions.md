# Claude Code 24小时自主运行开源方案汇总

> 最后更新：2025-12-28

## 目录

- [一、核心循环运行方案](#一核心循环运行方案)
- [二、多 Agent 编排平台](#二多-agent-编排平台)
- [三、24小时自主运行方案](#三24小时自主运行方案)
- [四、官方支持方案](#四官方支持方案)
- [五、安全沙箱方案](#五安全沙箱方案)
- [六、替代工具](#六替代工具)
- [七、技术对比表](#七技术对比表)
- [八、最佳实践推荐](#八最佳实践推荐)

---

## 一、核心循环运行方案

### 1. Continuous Claude ⭐ 推荐

**GitHub:** https://github.com/AnandChowdhary/continuous-claude

**简介：** 在持续循环中运行 Claude Code，自动创建 PR、等待 CI 检查、自动合并，让多步骤项目在你睡觉时完成。

**技术原理：**
- 使用 `SHARED_TASK_NOTES.md` 作为外部记忆，在迭代间传递上下文
- 每次迭代创建新分支，生成 commit 并推送
- 通过 GitHub CLI 自动化 PR 流程

**关键命令：**
```bash
# 安装
npm install -g continuous-claude

# 无限运行直到手动停止
continuous-claude -m 0

# 设置预算上限（美元）
continuous-claude --max-cost 50

# 设置时间限制
continuous-claude --max-duration 8h

# 组合使用
continuous-claude --max-cost 100 --max-duration 12h
```

**架构特点：**
- 支持多个专门化 agent 同时运行（开发、测试、重构）
- 跨迭代保持上下文连续性
- 自动等待 CI 检查通过后合并

**适用场景：**
- 大规模测试覆盖率提升
- 代码重构项目
- 批量文档生成

---

### 2. Ralph Wiggum 循环技术

**官方插件:** https://github.com/anthropics/claude-code/tree/main/plugins/ralph-wiggum
**原创文章:** https://ghuntley.com/ralph/

**简介：** Geoffrey Huntley 发明的技术，通过简单的 while 循环让 Claude 持续迭代直到任务完成。

**最简实现：**
```bash
# 基础版本
while :; do
    cat PROMPT.md | claude --dangerously-skip-permissions
done

# 带迭代计数
for i in {1..100}; do
    echo "=== Iteration $i ==="
    claude -p "$(cat PROMPT.md)" --dangerously-skip-permissions
done
```

**工作机制：**
1. Stop hook 拦截 Claude 退出
2. 重新喂入原始 prompt
3. 每次迭代可看到之前运行的文件修改和 git 历史
4. `--max-iterations` 作为安全限制

**设计哲学：**
> "The technique is deterministically bad in an undeterministic world. It's better to fail predictably than succeed unpredictably." — Geoffrey Huntley

**实际案例：**
- 运行3个月创建了 "cursed" 编程语言（Go 语法 + Gen Z 俚语）
- Y Combinator 黑客松：一晚上发布 6+ 仓库，API 成本仅 $297

**社区增强版：** https://github.com/frankbria/ralph-claude-code
- 智能退出检测
- 速率限制保护
- 实时监控仪表板
- 结构化任务管理
- 防止无限循环的安全机制

---

### 3. Infinite Agentic Loop

**GitHub:** https://github.com/disler/infinite-agentic-loop

**简介：** 使用双 prompt 系统的无限 agent 循环实验项目。

**技术原理：**
- 自定义 slash command `/project:infinite`
- 编排多个 AI agent 并行运行
- 基于规格说明生成内容的演进迭代

**特点：**
- 双 prompt 系统架构
- 并行 agent 协调
- 主题化混合 UI 组件生成

---

## 二、多 Agent 编排平台

### 4. Claude-Flow ⭐ 企业级

**GitHub:** https://github.com/ruvnet/claude-flow

**简介：** 企业级 AI 编排平台，结合群体智能、持久记忆和 100+ MCP 工具。

**核心能力：**
- 64 个专门化 agent 系统
- 分布式群体智能（Swarm Intelligence）
- 100+ MCP 工具集成
- RAG 向量检索集成
- AgentDB：96x-164x 更快向量搜索

**安装：**
```bash
# NPX（推荐，始终最新）
npx claude-flow@alpha init --force

# 全局安装
npm install -g claude-flow@alpha

# 验证安装
claude-flow --version
```

**架构设计：**
```
┌─────────────────────────────────────┐
│           MCP Tools (Brain)         │
│    协调、规划、决策、调度              │
├─────────────────────────────────────┤
│         Claude Code (Hands)          │
│    执行、编码、测试、部署              │
├─────────────────────────────────────┤
│           AgentDB (Memory)           │
│    向量存储、语义搜索、上下文持久化     │
└─────────────────────────────────────┘
```

**工作流编排功能：**
- 并行执行多任务
- 依赖管理和任务排序
- 智能资源分配
- stream-json 链式实时通信

**25 个内置 Claude Skills：**
- 开发技能
- GitHub 集成
- 记忆管理
- 自动化工作流

---

### 5. Multi-Agent Orchestration

**GitHub:** https://github.com/wshobson/agents

**简介：** 99 个专门化 AI agent 的综合生产就绪系统。

**包含组件：**
| 类型 | 数量 |
|------|------|
| 专门化 Agent | 99 |
| 工作流编排器 | 15 |
| Agent 技能 | 107 |
| 开发工具 | 71 |
| 单一目标插件 | 67 |

**Agent 角色示例：**
- `backend-architect` - 后端架构设计
- `database-architect` - 数据库设计
- `frontend-developer` - 前端开发
- `test-automator` - 测试自动化
- `security-auditor` - 安全审计
- `deployment-engineer` - 部署工程
- `observability-engineer` - 可观测性

---

### 6. Claude User Memory

**GitHub:** https://github.com/VAMFI/claude-user-memory

**简介：** Claude Code CLI 的自主 agent 基础设施，提供 4.8-5.5x 开发速度提升。

**工作流：**
```
Research → Plan → Implement
    ↓         ↓         ↓
 质量门禁   TDD强制   多Agent协调
```

**关键特性：**
- 断路器防止无限循环
- 质量门禁自动检查
- TDD 测试驱动开发强制执行
- 基于 Anthropic 工程研究构建

---

## 三、24小时自主运行方案

### 7. Clopus-02 ⭐ 完整自主方案

**来源:** https://denislavgavrilov.com/p/clopus-02-a-24-hour-claude-code-run

**简介：** 成功实现 24 小时无干预运行的 Claude Code 实例。

**技术架构：**
```
┌─────────────────────────────────────┐
│            Watcher Agent            │
│      监控状态、触发任务、协调         │
├──────────────────┬──────────────────┤
│   Short-term     │   Long-term      │
│   Memory         │   Memory         │
│   (SQLite3)      │   (Qdrant)       │
├──────────────────┴──────────────────┤
│            Worker Agent             │
│      执行任务、浏览器操作             │
├─────────────────────────────────────┤
│            Chromium                 │
│      网页浏览、信息收集              │
└─────────────────────────────────────┘
```

**核心组件：**
- **SQLite3** - 短期记忆，存储会话状态和近期上下文
- **Qdrant** - 长期记忆，向量数据库存储语义知识
- **Chromium** - 浏览器访问能力
- **Watcher-Worker 架构** - 分离监控和执行

**运行成果：**
- 24 小时无人工干预持续运行
- 完全自主决策执行任务
- 自动恢复和错误处理

---

### 8. Auto Claude

**GitHub:** https://github.com/ruizrica/auto-claude

**简介：** 自主多会话 AI 编码系统。

**核心功能：**
- 上下文感知记忆
- 自动化任务执行
- 合并冲突自动解决
- 多终端支持
- 会话恢复
- GitHub 深度集成
- Kanban 看板实时监控

**特性：**
- 并行 agent 同时运行多个构建
- 上下文工程优化
- 自验证 QA 循环
- Git worktree 隔离工作区

---

## 四、官方支持方案

### 9. Claude Code Headless Mode

**文档:** https://code.claude.com/docs/en/headless

**简介：** 官方无头模式，用于 CI、pre-commit hooks、构建脚本和自动化。

**基本使用：**
```bash
# 简单 headless 模式
claude -p "分析代码质量并生成报告"

# 带输出格式
claude -p "Review code" --output-format json

# 流式 JSON 输出
claude -p "Generate tests" --output-format stream-json
```

**完全自动化模式：**
```bash
claude -p "执行任务" \
    --dangerously-skip-permissions \
    --output-format stream-json \
    --allowedTools "Bash,Read,Grep,Write"
```

**配置白名单（推荐）：**
```json
// ~/.claude/settings.json
{
  "allowedTools": ["Bash", "Read", "Grep", "Write", "Edit"],
  "blockedTools": ["rm -rf", "curl", "wget"]
}
```

**结合 Cron 定时任务：**
```bash
# /etc/crontab 或 crontab -e

# 每天凌晨 2 点执行代码质量检查
0 2 * * * claude -p "Run code quality check" --output-format json >> /var/log/claude-nightly.log

# 每周日凌晨执行安全扫描
0 3 * * 0 claude -p "Security audit" --output-format json >> /var/log/claude-security.log

# 工作日每小时执行测试
0 * * * 1-5 claude -p "Run test suite" --output-format json
```

---

### 10. Claude Code GitHub Actions

**GitHub:** https://github.com/anthropics/claude-code-action

**简介：** 官方 GitHub Actions 集成，通过 @claude 触发 AI 自动化。

**安装：**
```bash
claude /install-github-app
```

**功能：**
- 在 PR/Issue 中 @claude 触发
- 自动分析代码
- 创建 Pull Request
- 实现功能和修复 bug
- 遵循项目标准和规范

**工作流示例：**
```yaml
# .github/workflows/claude.yml
name: Claude Code Review
on:
  pull_request:
    types: [opened, synchronize]
  issue_comment:
    types: [created]

jobs:
  claude-review:
    runs-on: ubuntu-latest
    steps:
      - uses: anthropics/claude-code-action@v1
        with:
          anthropic_api_key: ${{ secrets.ANTHROPIC_API_KEY }}
```

---

### 11. Background Agents (v2.0.60+)

**文档:** https://code.claude.com/docs/en/sub-agents

**简介：** 官方后台 agent 功能，实现真正的多任务处理。

**工作方式：**
1. 启动 agent 执行任务
2. Agent 在后台运行
3. 继续其他工作
4. Agent 完成后返回结果

**与多终端的区别：**
- Claude 自己管理多个 agent
- 协调多个 agent 工作
- 整合各 agent 结果
- 统一上下文管理

**子 Agent 特性：**
- 独立上下文窗口
- 只返回相关信息给主 agent
- 适合大量信息筛选任务
- 支持并行工作流

---

### 12. Claude Agent SDK

**文档:** https://www.anthropic.com/engineering/building-agents-with-the-claude-agent-sdk

**简介：** 官方 SDK，用于构建自主 AI agent 应用。

**核心能力：**
- 读取文件
- 编写代码
- 执行命令
- 集成自定义工具
- 保持上下文自主运行

**Python 示例：**
```python
from claude_code_sdk import Agent, PermissionMode

# 创建 agent
agent = Agent(
    permission_mode=PermissionMode.SAFE,
    allowed_tools=["Read", "Write", "Bash", "Edit"],
    disallowed_tools=["rm", "curl"],
)

# 自主执行任务
result = agent.run("""
Find and fix all type errors in src/
Run tests after each fix
Commit changes with descriptive messages
""")

# 获取结果
print(result.summary)
for file in result.modified_files:
    print(f"Modified: {file}")
```

**后台任务模式：**
```python
# 不需要实时输出时（CI/后台任务）
result = agent.run(
    "Generate API documentation",
    streaming=False  # 收集所有消息后返回
)
```

---

## 五、安全沙箱方案

### 13. Docker Sandbox（官方）

**文档:** https://docs.docker.com/ai/sandboxes

**简介：** Docker 官方支持的 Claude Code 沙箱环境。

**要求：** Docker Desktop 4.50+

**使用方法：**
```bash
# 启动沙箱
docker sandbox run claude-code

# 带工作目录
docker sandbox run claude-code --workdir /path/to/project

# 查看运行中的沙箱
docker sandbox ls
```

**特点：**
- `--dangerously-skip-permissions` 默认启用但安全隔离
- 当前工作目录自动 bind mount
- 严格边界隔离
- 网络隔离可选

**安全效果：**
> 在 Anthropic 内部使用中，沙箱安全地减少了 84% 的权限提示

---

### 14. 社区 Docker 方案

#### textcortex/claude-code-sandbox

**GitHub:** https://github.com/textcortex/claude-code-sandbox

**使用：**
```bash
git clone https://github.com/textcortex/claude-code-sandbox
cd claude-code-sandbox
docker-compose up -d
```

#### rvaidya/claude-code-sandbox

**GitHub:** https://github.com/rvaidya/claude-code-sandbox

**特点：**
- `--dangerously-skip-permissions` 自动启用但安全
- 即使 Claude 尝试修改系统文件，也只影响容器
- 主机系统完全保护

**使用：**
```bash
# 构建
docker build -t claude-sandbox .

# 运行
docker run -it -v $(pwd):/workspace claude-sandbox
```

---

### 15. DevContainer 方案

**来源:** https://codewithandrea.com/articles/run-ai-agents-inside-devcontainer/

**简介：** 使用 VS Code DevContainer 创建隔离环境。

**配置示例：**
```json
// .devcontainer/devcontainer.json
{
  "name": "Claude Code Sandbox",
  "image": "mcr.microsoft.com/devcontainers/base:ubuntu",
  "features": {
    "ghcr.io/devcontainers/features/node:1": {},
    "ghcr.io/devcontainers/features/python:1": {}
  },
  "postCreateCommand": "npm install -g @anthropic-ai/claude-code",
  "mounts": [
    "source=${localWorkspaceFolder},target=/workspace,type=bind"
  ],
  "runArgs": ["--network=none"]  // 可选：禁用网络
}
```

**优势：**
- Claude Code 只能访问挂载的项目
- 可安全使用 `--dangerously-skip-permissions`
- 防止 prompt injection 攻击
- VS Code 原生支持

---

## 六、替代工具

### 16. OpenHands (原 OpenDevin)

**GitHub:** https://github.com/OpenHands/OpenHands
**官网:** https://openhands.dev/

**简介：** MIT 许可的开源 AI 软件开发平台，支持多种 LLM。

**能力：**
- 修改代码
- 执行命令
- 浏览网页
- 调用 API
- 自动化软件开发生命周期

**工具套件：**
- 聊天界面
- 命令终端
- 工作流规划器
- 代码编辑器
- 集成浏览器

**使用方式：**
```bash
# CLI 方式
pip install openhands
openhands start

# Docker 方式
docker run -it openhands/openhands

# 云服务
# 访问 openhands.dev 获得 $10 免费额度
```

**LLM 支持：** Claude, GPT, 本地模型

---

### 17. Cline

**GitHub:** https://github.com/cline/cline
**VS Code:** https://marketplace.visualstudio.com/items?itemName=saoudrizwan.claude-dev

**简介：** VS Code 中的自主编码 agent，4M+ 开发者使用。

**核心特性：**
- 创建/编辑文件
- 执行命令
- 浏览器自动化（Computer Use）
- MCP 协议扩展
- 工作区快照和回滚

**后台执行：**
- "Proceed While Running" 按钮
- 开发服务器等长时间任务后台运行
- Claude 继续处理其他任务

**安全机制：**
- 人在回路（Human-in-the-loop）
- 每个文件修改和命令需批准
- 工作区快照可回滚

**安装：**
```bash
# VS Code 扩展市场搜索 "Cline" 安装
# 或命令行
code --install-extension saoudrizwan.claude-dev
```

---

### 18. Aider

**GitHub:** https://github.com/Aider-AI/aider

**简介：** 终端中的 AI 结对编程工具。

**核心能力：**
- 每次修改后自动 lint 和测试
- 自动修复检测到的问题
- 支持几乎所有 LLM

**最佳模型支持：**
- Claude 3.7 Sonnet
- DeepSeek R1 & Chat V3
- OpenAI o1, o3-mini, GPT-4o

**使用：**
```bash
# 安装
pip install aider-chat

# 使用 Claude
export ANTHROPIC_API_KEY=your-key
aider --model claude-3-sonnet

# 使用本地模型
aider --model ollama/codellama
```

---

### 19. RA.Aid

**GitHub:** https://github.com/ai-christianson/RA.Aid

**简介：** 基于 LangGraph 的独立自主编码 agent。

**特点：**
- 自主软件开发
- LangGraph agent 框架
- 可通过 `--use-aider` 集成 Aider

**使用：**
```bash
pip install ra-aid
ra-aid "Build a REST API with FastAPI"

# 集成 Aider
ra-aid --use-aider "Refactor authentication module"
```

---

## 七、技术对比表

### 功能对比

| 方案 | 类型 | 记忆持久化 | 安全隔离 | 多 Agent | 上手难度 | 维护成本 |
|------|------|-----------|---------|----------|----------|----------|
| Continuous Claude | 循环脚本 | Markdown 文件 | 无 | 支持 | ⭐ | 低 |
| Ralph Wiggum | 官方插件 | Git 历史 | 无 | 单 | ⭐ | 低 |
| Claude-Flow | 企业平台 | AgentDB | 可选 | 64+ | ⭐⭐⭐ | 中 |
| Clopus-02 | 自定义架构 | SQLite + Qdrant | 无 | Watcher-Worker | ⭐⭐⭐⭐ | 高 |
| Docker Sandbox | 容器 | 无 | 强 | 单 | ⭐⭐ | 低 |
| Agent SDK | SDK | 可编程 | 可配置 | 可编程 | ⭐⭐⭐ | 中 |
| OpenHands | 完整平台 | 有 | 有 | 是 | ⭐⭐ | 中 |
| Cline | VS Code 扩展 | 快照 | 人工审批 | 单 | ⭐ | 低 |

### 适用场景

| 场景 | 推荐方案 |
|------|---------|
| 快速上手、简单任务 | Ralph Wiggum / Cline |
| 大规模代码生成 | Continuous Claude |
| 企业级多 Agent | Claude-Flow |
| 完全自主 24h 运行 | Clopus-02 架构 |
| CI/CD 集成 | GitHub Actions + Headless |
| 安全敏感环境 | Docker Sandbox |
| 自定义开发 | Agent SDK |

---

## 八、最佳实践推荐

### 入门级：快速开始

**推荐：** Continuous Claude + Docker Sandbox

```bash
# 1. 安装 Docker Desktop
# 2. 启动安全沙箱
docker sandbox run claude-code

# 3. 在沙箱内安装 continuous-claude
npm install -g continuous-claude

# 4. 开始运行
continuous-claude --max-cost 20 --max-duration 4h
```

**优点：**
- 5 分钟内可以开始
- 安全隔离
- 成本可控

---

### 中级：生产环境

**推荐：** Claude-Flow + 自定义工作流 + 监控

```bash
# 1. 安装 Claude-Flow
npx claude-flow@alpha init --force

# 2. 配置工作流
# 编辑 .claude-flow/config.yaml

# 3. 启动编排器
claude-flow orchestrate --workflow my-workflow

# 4. 监控仪表板
claude-flow dashboard
```

**配合监控：**
- 使用 Codex Hub（本项目）监控所有会话
- 设置成本告警
- 日志收集和分析

---

### 高级：极限自主

**推荐：** Clopus-02 架构复刻

**技术栈：**
```
Python 3.11+
├── Claude Agent SDK
├── SQLite3 (短期记忆)
├── Qdrant (长期向量记忆)
├── Playwright/Selenium (浏览器)
└── Supervisor/PM2 (进程管理)
```

**架构设计：**
```python
# watcher.py - 监控进程
class Watcher:
    def __init__(self):
        self.short_memory = SQLiteMemory()
        self.long_memory = QdrantMemory()
        self.worker = Worker()

    async def run(self):
        while True:
            state = await self.assess_state()
            task = await self.decide_task(state)
            await self.worker.execute(task)
            await self.update_memory(task, result)
            await asyncio.sleep(60)  # 1分钟检查间隔

# worker.py - 执行进程
class Worker:
    def __init__(self):
        self.agent = Agent(
            permission_mode="accept_all",
            allowed_tools=["*"],
        )
        self.browser = Browser()

    async def execute(self, task):
        result = await self.agent.run(task.prompt)
        return result
```

---

### CI/CD 集成

**推荐：** GitHub Actions + Headless Mode

```yaml
# .github/workflows/claude-ci.yml
name: Claude CI/CD

on:
  push:
    branches: [main, develop]
  pull_request:
  schedule:
    - cron: '0 2 * * *'  # 每天凌晨 2 点

jobs:
  claude-review:
    runs-on: ubuntu-latest
    container:
      image: anthropic/claude-code:latest
    steps:
      - uses: actions/checkout@v4

      - name: Code Review
        run: |
          claude -p "Review changes for bugs and security issues" \
            --output-format json > review.json
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}

      - name: Generate Tests
        run: |
          claude -p "Generate missing unit tests" \
            --dangerously-skip-permissions \
            --allowedTools "Read,Write,Bash"

      - name: Create PR if changes
        run: |
          if [ -n "$(git status --porcelain)" ]; then
            git checkout -b claude/auto-tests-${{ github.sha }}
            git add .
            git commit -m "test: auto-generated tests by Claude"
            gh pr create --fill
          fi
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

---

## 附录：常见问题

### Q: 如何防止无限循环和成本失控？

**A:** 多重保护措施：
1. `--max-iterations` 限制迭代次数
2. `--max-cost` 限制 API 成本
3. `--max-duration` 限制运行时间
4. 断路器模式检测无进展循环
5. Docker 资源限制（CPU/内存）

### Q: 如何处理 context window 耗尽？

**A:**
1. 使用外部记忆（Markdown 文件/数据库）
2. 每次迭代重新启动 Claude（Ralph 技术）
3. 使用向量数据库存储长期记忆
4. 子 Agent 分担上下文

### Q: 安全性如何保障？

**A:**
1. Docker 沙箱隔离
2. 工具白名单限制
3. 网络隔离（可选）
4. 定期审计日志
5. 成本上限告警

---

## 参考链接

- [Continuous Claude](https://github.com/AnandChowdhary/continuous-claude)
- [Ralph Wiggum 原创](https://ghuntley.com/ralph/)
- [Claude-Flow](https://github.com/ruvnet/claude-flow)
- [Clopus-02 24小时运行](https://denislavgavrilov.com/p/clopus-02-a-24-hour-claude-code-run)
- [Claude Code Headless 文档](https://code.claude.com/docs/en/headless)
- [Claude Code 沙箱](https://www.anthropic.com/engineering/claude-code-sandboxing)
- [Docker Sandboxes](https://docs.docker.com/ai/sandboxes)
- [Claude Agent SDK](https://www.anthropic.com/engineering/building-agents-with-the-claude-agent-sdk)
- [OpenHands](https://github.com/OpenHands/OpenHands)
- [Cline](https://github.com/cline/cline)
- [Aider](https://github.com/Aider-AI/aider)
