#!/usr/bin/env python3
"""
Claude Code Autonomous Orchestrator
====================================
最小化设计，最大化稳定性

核心原则：
1. Worker 短命，Orchestrator 长存
2. 外部记忆 > 内部记忆
3. 多重终止条件防止失控
"""

import subprocess
import time
import json
import os
import sys
import hashlib
import signal
import shutil
from pathlib import Path
from datetime import datetime, timedelta
from dataclasses import dataclass, field
from typing import Optional
import yaml
import logging

# ============================================================
# Configuration
# ============================================================

@dataclass
class Config:
    """运行配置"""
    # 限制
    max_iterations: int = 100
    max_cost_usd: float = 50.0
    max_duration_hours: float = 8.0
    consecutive_no_progress: int = 3
    stop_when_empty: bool = True

    # 执行
    cooldown_seconds: int = 10
    worker_timeout_seconds: int = 1800  # 30分钟

    # Docker
    use_docker: bool = True
    docker_image: str = "claude-worker"

    # 路径
    memory_dir: str = "memory"
    workspace_dir: str = "workspace"
    log_dir: str = "logs"

    @classmethod
    def from_yaml(cls, path: str) -> "Config":
        if Path(path).exists():
            with open(path) as f:
                data = yaml.safe_load(f) or {}
            return cls(**{k: v for k, v in data.items() if hasattr(cls, k)})
        return cls()


# ============================================================
# Logging Setup
# ============================================================

def setup_logging(log_dir: str) -> logging.Logger:
    """配置日志"""
    Path(log_dir).mkdir(exist_ok=True)

    logger = logging.getLogger("orchestrator")
    logger.setLevel(logging.DEBUG)

    # 文件日志
    fh = logging.FileHandler(
        Path(log_dir) / f"orchestrator_{datetime.now():%Y%m%d_%H%M%S}.log"
    )
    fh.setLevel(logging.DEBUG)
    fh.setFormatter(logging.Formatter(
        "%(asctime)s | %(levelname)-8s | %(message)s"
    ))

    # 控制台日志
    ch = logging.StreamHandler()
    ch.setLevel(logging.INFO)
    ch.setFormatter(logging.Formatter(
        "%(asctime)s | %(message)s", datefmt="%H:%M:%S"
    ))

    logger.addHandler(fh)
    logger.addHandler(ch)

    return logger


# ============================================================
# Memory Manager
# ============================================================

class MemoryManager:
    """管理外部记忆文件"""

    def __init__(self, memory_dir: str):
        self.dir = Path(memory_dir)
        self.dir.mkdir(exist_ok=True)
        self._init_files()

    def _init_files(self):
        """初始化记忆文件（如果不存在）"""
        tasks_file = self.dir / "TASKS.md"
        if not tasks_file.exists():
            tasks_file.write_text("""# 待办任务

## 高优先级
- [ ] 添加你的第一个任务

## 中优先级

## 低优先级

---
*创建时间: {now}*
""".format(now=datetime.now().strftime("%Y-%m-%d %H:%M")))

        context_file = self.dir / "CONTEXT.md"
        if not context_file.exists():
            context_file.write_text("""# 项目上下文

## 项目概述
描述你的项目...

## 最近完成的工作
（暂无）

## 当前阻塞问题
（暂无）

## 下一步建议
查看 TASKS.md 中的任务列表

---
*最后更新: {now}*
""".format(now=datetime.now().strftime("%Y-%m-%d %H:%M")))

        done_file = self.dir / "DONE.md"
        if not done_file.exists():
            done_file.write_text("""# 完成历史

| 时间 | 任务 | 备注 |
|------|------|------|

""")

    def read_tasks(self) -> str:
        return (self.dir / "TASKS.md").read_text()

    def read_context(self) -> str:
        return (self.dir / "CONTEXT.md").read_text()

    def read_done(self) -> str:
        return (self.dir / "DONE.md").read_text()

    def get_content_hash(self) -> str:
        """获取所有记忆文件的哈希，用于检测变化"""
        content = self.read_tasks() + self.read_context()
        return hashlib.md5(content.encode()).hexdigest()[:8]

    def has_pending_tasks(self) -> bool:
        """检查是否有待办任务"""
        tasks = self.read_tasks()
        return "- [ ]" in tasks

    def count_completed_tasks(self) -> int:
        """统计已完成任务数"""
        done = self.read_done()
        return done.count("|") // 3 - 1  # 减去表头


# ============================================================
# Worker Runner
# ============================================================

class WorkerRunner:
    """运行 Claude Code Worker"""

    def __init__(self, config: Config, memory: MemoryManager, logger: logging.Logger):
        self.config = config
        self.memory = memory
        self.logger = logger
        self.workspace = Path(config.workspace_dir).absolute()
        self.workspace.mkdir(exist_ok=True)

    def build_prompt(self) -> str:
        """构建给 Worker 的 prompt"""
        context = self.memory.read_context()
        tasks = self.memory.read_tasks()
        memory_path = self.memory.dir.absolute()

        return f"""你是一个自主编码 Agent，正在持续执行任务。你的目标是不断改进项目。

## 当前上下文
{context}

## 待办任务
{tasks}

## 你的工作流程
1. 阅读上面的上下文，了解当前状态
2. 如果有待办任务（- [ ]），选择一个执行
3. 如果没有待办任务，你需要**自主发现新任务**：
   - 审查代码质量，发现可优化的地方
   - 添加新功能或改进现有功能
   - 重构代码，提高可维护性
   - 添加测试、文档、错误处理
   - 性能优化、用户体验改进
4. 更新记忆文件（路径: {memory_path}）：
   - 编辑 TASKS.md：将完成的任务标记为 [x]，添加新发现的任务
   - 编辑 CONTEXT.md：更新"最近完成的工作"和"下一步建议"
   - 在 DONE.md 追加一行记录：| {datetime.now():%Y-%m-%d %H:%M} | 任务描述 | 备注 |

## 重要规则
- 每次只完成 1-2 个任务，不要贪多
- **永远保持 TASKS.md 中有未完成的任务**（自己发现新任务）
- 如果任务太大，拆分成子任务添加到 TASKS.md
- 完成后必须更新 .md 文件，这是你和下一次运行沟通的方式
- 持续改进，没有"完成"的概念，总有可以优化的地方

## 开始工作
查看任务列表，如果为空则自主发现新任务，然后执行。"""

    def run_with_docker(self, prompt: str) -> dict:
        """在 Docker 中运行 Worker"""
        memory_abs = self.memory.dir.absolute()
        workspace_abs = self.workspace.absolute()

        # Claude Code 使用 ~/.claude 目录存储认证信息
        claude_config = Path.home() / ".claude"

        cmd = [
            "docker", "run", "--rm",
            "-v", f"{memory_abs}:/memory",
            "-v", f"{workspace_abs}:/workspace",
            "-v", f"{claude_config}:/home/worker/.claude:ro",  # 挂载认证
            "-w", "/workspace",
            self.config.docker_image,
            "-p", prompt,
            "--dangerously-skip-permissions",
            "--output-format", "json"
        ]

        self.logger.debug(f"Docker command: {' '.join(cmd[:10])}...")

        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=self.config.worker_timeout_seconds
        )

        return self._parse_result(result)

    def run_direct(self, prompt: str) -> dict:
        """直接运行 Claude Code（不使用 Docker）"""
        cmd = [
            "claude",
            "-p", prompt,
            "--dangerously-skip-permissions",
            "--output-format", "json"
        ]

        env = os.environ.copy()

        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=self.config.worker_timeout_seconds,
            cwd=self.workspace,
            env=env
        )

        return self._parse_result(result)

    def _parse_result(self, result: subprocess.CompletedProcess) -> dict:
        """解析运行结果"""
        output = {
            "success": result.returncode == 0,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "cost": 0.0,
            "tokens": 0
        }

        # 尝试解析 JSON 输出
        try:
            if result.stdout:
                lines = result.stdout.strip().split('\n')
                for line in lines:
                    if line.startswith('{'):
                        data = json.loads(line)
                        if "cost" in data:
                            output["cost"] = data.get("cost", 0)
                        if "usage" in data:
                            output["tokens"] = data["usage"].get("total_tokens", 0)
        except json.JSONDecodeError:
            pass

        return output

    def run(self) -> dict:
        """运行 Worker"""
        prompt = self.build_prompt()

        if self.config.use_docker:
            return self.run_with_docker(prompt)
        else:
            return self.run_direct(prompt)


# ============================================================
# Orchestrator
# ============================================================

class Orchestrator:
    """主控制器"""

    def __init__(self, config_path: str = "config.yaml"):
        self.config = Config.from_yaml(config_path)
        self.logger = setup_logging(self.config.log_dir)
        self.memory = MemoryManager(self.config.memory_dir)
        self.worker = WorkerRunner(self.config, self.memory, self.logger)

        # 运行状态
        self.start_time = datetime.now()
        self.total_cost = 0.0
        self.total_tokens = 0
        self.iteration = 0
        self.no_progress_count = 0
        self.last_hash = ""
        self.running = True

        # 信号处理
        signal.signal(signal.SIGINT, self._handle_signal)
        signal.signal(signal.SIGTERM, self._handle_signal)

    def _handle_signal(self, signum, frame):
        """优雅退出"""
        self.logger.info(f"收到信号 {signum}，准备退出...")
        self.running = False

    def elapsed_time(self) -> timedelta:
        return datetime.now() - self.start_time

    def should_continue(self) -> tuple[bool, str]:
        """检查是否应该继续运行"""
        if not self.running:
            return False, "用户中断"

        # 时间限制 (0 = 无限)
        if self.config.max_duration_hours > 0:
            elapsed_hours = self.elapsed_time().total_seconds() / 3600
            if elapsed_hours >= self.config.max_duration_hours:
                return False, f"达到时间限制 ({self.config.max_duration_hours}h)"

        # 成本限制 (0 = 无限)
        if self.config.max_cost_usd > 0 and self.total_cost >= self.config.max_cost_usd:
            return False, f"达到成本限制 (${self.config.max_cost_usd})"

        # 迭代限制 (0 = 无限)
        if self.config.max_iterations > 0 and self.iteration >= self.config.max_iterations:
            return False, f"达到迭代限制 ({self.config.max_iterations})"

        # 无进展检测
        if self.no_progress_count >= self.config.consecutive_no_progress:
            return False, f"连续 {self.config.consecutive_no_progress} 次无进展"

        # 任务队列为空
        if self.config.stop_when_empty and not self.memory.has_pending_tasks():
            return False, "所有任务已完成"

        return True, ""

    def detect_progress(self) -> bool:
        """检测是否有进展（通过文件变化）"""
        current_hash = self.memory.get_content_hash()
        has_progress = current_hash != self.last_hash
        self.last_hash = current_hash
        return has_progress

    def print_status(self):
        """打印当前状态"""
        elapsed = self.elapsed_time()
        completed = self.memory.count_completed_tasks()

        self.logger.info(f"""
{'='*60}
迭代 #{self.iteration} | 运行时间 {elapsed} | 成本 ${self.total_cost:.4f}
已完成任务: {completed} | 无进展计数: {self.no_progress_count}/{self.config.consecutive_no_progress}
{'='*60}""")

    def print_summary(self):
        """打印运行摘要"""
        elapsed = self.elapsed_time()
        completed = self.memory.count_completed_tasks()

        summary = f"""
{'='*60}
                    运行结束摘要
{'='*60}
总迭代次数:     {self.iteration}
总运行时间:     {elapsed}
总成本:         ${self.total_cost:.4f}
总 tokens:      {self.total_tokens:,}
完成任务数:     {completed}
{'='*60}
"""
        self.logger.info(summary)

        # 保存摘要到文件
        summary_file = Path(self.config.log_dir) / "summary.txt"
        with open(summary_file, "a") as f:
            f.write(f"\n--- {datetime.now():%Y-%m-%d %H:%M:%S} ---\n")
            f.write(summary)

    def run(self):
        """主循环"""
        self.logger.info("Orchestrator 启动")
        self.logger.info(f"配置: max_iterations={self.config.max_iterations}, "
                        f"max_cost=${self.config.max_cost_usd}, "
                        f"max_duration={self.config.max_duration_hours}h")

        self.last_hash = self.memory.get_content_hash()

        while True:
            should_run, reason = self.should_continue()
            if not should_run:
                self.logger.info(f"停止原因: {reason}")
                break

            self.iteration += 1
            self.print_status()

            try:
                self.logger.info("启动 Worker...")
                result = self.worker.run()

                # 更新统计
                self.total_cost += result.get("cost", 0)
                self.total_tokens += result.get("tokens", 0)

                if result["success"]:
                    if self.detect_progress():
                        self.no_progress_count = 0
                        self.logger.info("Worker 完成，检测到进展")
                    else:
                        self.no_progress_count += 1
                        self.logger.warning(f"Worker 完成，但无进展 ({self.no_progress_count})")
                else:
                    self.logger.error(f"Worker 失败: {result.get('stderr', 'unknown error')}")
                    self.no_progress_count += 1

            except subprocess.TimeoutExpired:
                self.logger.warning(f"Worker 超时 ({self.config.worker_timeout_seconds}s)")
                self.no_progress_count += 1

            except Exception as e:
                self.logger.error(f"Worker 异常: {e}")
                self.no_progress_count += 1

            # 冷却
            if self.running:
                self.logger.debug(f"冷却 {self.config.cooldown_seconds}s...")
                time.sleep(self.config.cooldown_seconds)

        self.print_summary()
        self.logger.info("Orchestrator 退出")


# ============================================================
# Entry Point
# ============================================================

def main():
    import argparse

    parser = argparse.ArgumentParser(description="Claude Code Autonomous Orchestrator")
    parser.add_argument("-c", "--config", default="config.yaml", help="配置文件路径")
    parser.add_argument("--max-iterations", type=int, help="最大迭代次数")
    parser.add_argument("--max-cost", type=float, help="最大成本 (USD)")
    parser.add_argument("--max-duration", type=float, help="最大运行时间 (小时)")
    parser.add_argument("--no-docker", action="store_true", help="不使用 Docker")

    args = parser.parse_args()

    # 检查 Claude Code 是否可用
    if not shutil.which("claude"):
        print("错误: 未找到 claude 命令")
        print("请安装: npm install -g @anthropic-ai/claude-code")
        sys.exit(1)

    orchestrator = Orchestrator(args.config)

    # 命令行参数覆盖配置
    if args.max_iterations:
        orchestrator.config.max_iterations = args.max_iterations
    if args.max_cost:
        orchestrator.config.max_cost_usd = args.max_cost
    if args.max_duration:
        orchestrator.config.max_duration_hours = args.max_duration
    if args.no_docker:
        orchestrator.config.use_docker = False

    orchestrator.run()


if __name__ == "__main__":
    main()
