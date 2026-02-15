#!/bin/bash
# Claude Code Autonomous Runner - 停止脚本
# ==========================================

echo "发送停止信号..."

# 查找并停止 orchestrator 进程
pkill -f "python3 orchestrator.py" 2>/dev/null

# 停止可能运行的 Docker 容器
CONTAINER=$(docker ps -q --filter "ancestor=claude-worker" 2>/dev/null)
if [ -n "$CONTAINER" ]; then
    echo "停止 Docker 容器..."
    docker stop $CONTAINER
fi

echo "已停止"
