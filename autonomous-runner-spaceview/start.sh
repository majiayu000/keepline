#!/bin/bash
# SpaceView Autonomous Runner 启动脚本

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# 检查 Python
if ! command -v python3 &> /dev/null; then
    echo "错误: 未找到 python3"
    exit 1
fi

# 检查 Claude Code
if ! command -v claude &> /dev/null; then
    echo "错误: 未找到 claude 命令"
    echo "请安装: npm install -g @anthropic-ai/claude-code"
    exit 1
fi

# 检查依赖
pip3 install -q pyyaml 2>/dev/null

echo "=========================================="
echo "  SpaceView Autonomous Runner"
echo "=========================================="
echo "目标项目: /Users/lifcc/Desktop/code/OpenSource/SpaceView"
echo "Memory: $SCRIPT_DIR/memory"
echo "按 Ctrl+C 停止"
echo ""

python3 orchestrator.py "$@"
