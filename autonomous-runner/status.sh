#!/bin/bash
# Claude Code Autonomous Runner - 状态查看脚本
# =============================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# 颜色定义
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}"
echo "╔════════════════════════════════════════════════════════════╗"
echo "║                    运行状态                                 ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

# ============================================================
# Orchestrator 状态
# ============================================================

echo -e "${YELLOW}Orchestrator 进程:${NC}"
if pgrep -f "python3 orchestrator.py" > /dev/null; then
    PID=$(pgrep -f "python3 orchestrator.py")
    echo -e "  ${GREEN}● 运行中${NC} (PID: $PID)"
else
    echo "  ○ 未运行"
fi
echo ""

# ============================================================
# Docker 容器状态
# ============================================================

echo -e "${YELLOW}Docker 容器:${NC}"
CONTAINER=$(docker ps -q --filter "ancestor=claude-worker" 2>/dev/null)
if [ -n "$CONTAINER" ]; then
    echo -e "  ${GREEN}● Worker 运行中${NC} (ID: $CONTAINER)"
else
    echo "  ○ 无运行中的 Worker"
fi
echo ""

# ============================================================
# 任务统计
# ============================================================

echo -e "${YELLOW}任务统计:${NC}"
if [ -f memory/TASKS.md ]; then
    PENDING=$(grep -c "\- \[ \]" memory/TASKS.md 2>/dev/null || echo 0)
    COMPLETED=$(grep -c "\- \[x\]" memory/TASKS.md 2>/dev/null || echo 0)
    echo "  待办任务: $PENDING"
    echo "  已完成:   $COMPLETED"
else
    echo "  TASKS.md 不存在"
fi
echo ""

# ============================================================
# 最近日志
# ============================================================

echo -e "${YELLOW}最近日志:${NC}"
LATEST_LOG=$(ls -t logs/orchestrator_*.log 2>/dev/null | head -1)
if [ -n "$LATEST_LOG" ]; then
    echo "  文件: $LATEST_LOG"
    echo ""
    echo "  最后 10 行:"
    echo "  ─────────────────────────────────────────"
    tail -10 "$LATEST_LOG" | sed 's/^/  /'
else
    echo "  无日志文件"
fi
echo ""

# ============================================================
# 完成历史
# ============================================================

echo -e "${YELLOW}最近完成:${NC}"
if [ -f memory/DONE.md ]; then
    # 跳过表头，显示最后 5 条记录
    tail -5 memory/DONE.md | grep "^|" | grep -v "时间" | sed 's/^/  /'
    if [ -z "$(tail -5 memory/DONE.md | grep "^|" | grep -v "时间")" ]; then
        echo "  （暂无完成记录）"
    fi
else
    echo "  DONE.md 不存在"
fi
