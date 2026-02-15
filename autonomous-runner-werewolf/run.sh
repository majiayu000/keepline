#!/bin/bash

# Werewolf Runner 启动脚本
cd "$(dirname "$0")"

# 默认参数
MAX_ITERATIONS=${1:-0}  # 0 = 无限
MAX_COST=${2:-0}        # 0 = 无限
MAX_DURATION=${3:-0}    # 0 = 无限 (小时)

echo "🐺 启动狼人杀 Runner..."
echo "最大迭代: ${MAX_ITERATIONS:-无限}"
echo "最大成本: \$${MAX_COST:-无限}"
echo "最大时长: ${MAX_DURATION:-无限}h"
echo ""

python3 orchestrator.py \
    --max-iterations $MAX_ITERATIONS \
    --max-cost $MAX_COST \
    --max-duration $MAX_DURATION \
    --no-docker

echo ""
echo "Runner 已停止"
