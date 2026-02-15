#!/usr/bin/env python3
"""
SpaceView Autonomous Orchestrator
==================================
专为 SpaceView 项目（Tauri + React + Rust）设计的自主优化编排器
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
from dataclasses import dataclass
import yaml
import logging

# ============================================================
# Configuration
# ============================================================

@dataclass
class Config:
    """运行配置"""
    # 限制 (0 = 无限制)
    max_iterations: int = 0
    max_cost_usd: float = 0
    max_duration_hours: float = 0
    consecutive_no_progress: int = 5
    stop_when_empty: bool = False

    # 执行
    cooldown_seconds: int = 10
    worker_timeout_seconds: int = 1800  # 30分钟

    # Docker
    use_docker: bool = False

    # 路径
    workspace_path: str = "/Users/lifcc/Desktop/code/OpenSource/SpaceView"
    memory_path: str = "./memory"
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

    def __init__(self, memory_path: str):
        self.dir = Path(memory_path)
        self.dir.mkdir(exist_ok=True)

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
        self.workspace = Path(config.workspace_path).absolute()

    def build_prompt(self) -> str:
        """构建给 Worker 的 prompt - SpaceView 专用"""
        context = self.memory.read_context()
        tasks = self.memory.read_tasks()
        memory_path = self.memory.dir.absolute()

        return f"""你是一个专业的 Tauri + React + Rust 开发者，正在自主优化 SpaceView 项目。

## 项目信息
SpaceView 是一个 macOS 磁盘空间分析工具，技术栈：
- 前端: React 19 + TypeScript + Vite
- 后端: Rust + Tauri 2.0
- 样式: CSS Variables 主题系统

## 当前上下文
{context}

## 待办任务
{tasks}

## 你的工作流程
1. 阅读上面的上下文和待办任务
2. 如果有待办任务（- [ ]），选择一个执行
3. 如果没有待办任务，**自主发现新任务**：
   - 审查代码质量，发现可优化的地方
   - 重构过大的文件（>300行的组件应拆分）
   - 添加缺失的 TypeScript 类型
   - 改进错误处理和用户体验
   - 性能优化（React 渲染、Rust 算法）
   - 添加测试覆盖
4. 更新记忆文件（路径: {memory_path}）：
   - TASKS.md：将完成的任务标记为 [x]，添加新发现的任务
   - CONTEXT.md：更新"最近完成的工作"
   - DONE.md：追加一行 | {datetime.now():%Y-%m-%d %H:%M} | 任务描述 | 备注 |

## 技术规范
- React 组件使用函数式组件 + hooks
- 使用 React.memo 优化渲染性能
- Rust 代码遵循 Clippy 建议
- 保持主题系统兼容性
- 每次改动后确保 `pnpm build` 通过

## 开发命令
- `pnpm dev` - 前端开发服务器
- `pnpm tauri:dev` - 完整 Tauri 开发模式
- `pnpm build` - 构建检查（包含 TypeScript 类型检查）

## 重要规则
- 每次只完成 1-2 个任务
- **永远保持 TASKS.md 中有未完成的任务**
- 大任务要拆分成子任务
- 完成后必须更新 .md 文件
- 不要破坏现有功能

## 开始工作
查看任务列表，选择一个执行。如果列表为空，自主发现新的优化任务。"""

    def run_direct(self, prompt: str) -> dict:
        """直接运行 Claude Code"""
        cmd = [
            "claude",
            "-p", prompt,
            "--dangerously-skip-permissions",
            "--output-format", "json"
        ]

        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=self.config.worker_timeout_seconds,
            cwd=self.workspace,
            env=os.environ.copy()
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
        return self.run_direct(prompt)


# ============================================================
# Orchestrator
# ============================================================

class Orchestrator:
    """主控制器"""

    def __init__(self, config_path: str = "config.yaml"):
        self.config = Config.from_yaml(config_path)
        self.logger = setup_logging(self.config.log_dir)
        self.memory = MemoryManager(self.config.memory_path)
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
                    SpaceView 优化运行结束
{'='*60}
总迭代次数:     {self.iteration}
总运行时间:     {elapsed}
总成本:         ${self.total_cost:.4f}
完成任务数:     {completed}
{'='*60}
"""
        self.logger.info(summary)

    def run(self):
        """主循环"""
        self.logger.info("SpaceView Orchestrator 启动")
        self.logger.info(f"目标项目: {self.config.workspace_path}")
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

    parser = argparse.ArgumentParser(description="SpaceView Autonomous Orchestrator")
    parser.add_argument("-c", "--config", default="config.yaml", help="配置文件路径")

    args = parser.parse_args()

    # 检查 Claude Code 是否可用
    if not shutil.which("claude"):
        print("错误: 未找到 claude 命令")
        print("请安装: npm install -g @anthropic-ai/claude-code")
        sys.exit(1)

    orchestrator = Orchestrator(args.config)
    orchestrator.run()


if __name__ == "__main__":
    main()
