#!/bin/bash
# Claude Code Autonomous Runner - 启动脚本
# ==========================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}"
echo "╔════════════════════════════════════════════════════════════╗"
echo "║       Claude Code Autonomous Runner                        ║"
echo "║       让 Claude 24小时自主运行                              ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

# ============================================================
# 检查前置条件
# ============================================================

echo -e "${YELLOW}[1/5] 检查环境...${NC}"

# 检查 Python
if ! command -v python3 &> /dev/null; then
    echo -e "${RED}错误: 未找到 python3${NC}"
    exit 1
fi
echo "  ✓ Python3 已安装"

# 检查 Docker（如果配置使用）
USE_DOCKER=$(grep "use_docker:" config.yaml | awk '{print $2}')
if [ "$USE_DOCKER" = "true" ]; then
    if ! command -v docker &> /dev/null; then
        echo -e "${RED}错误: 未找到 Docker，请安装或设置 use_docker: false${NC}"
        exit 1
    fi
    echo "  ✓ Docker 已安装"
fi

# 检查 Claude Code
if ! command -v claude &> /dev/null; then
    echo -e "${RED}错误: 未找到 claude 命令${NC}"
    echo "  请安装: npm install -g @anthropic-ai/claude-code"
    exit 1
fi
echo "  ✓ Claude Code 已安装"

# 检查是否已登录
if [ ! -d "$HOME/.claude" ]; then
    echo -e "${YELLOW}  ⚠ 可能未登录，请先运行 'claude' 登录${NC}"
fi

# ============================================================
# 安装 Python 依赖
# ============================================================

echo -e "${YELLOW}[2/5] 检查 Python 依赖...${NC}"

if ! python3 -c "import yaml" 2>/dev/null; then
    echo "  安装 PyYAML..."
    pip3 install pyyaml -q
fi
echo "  ✓ 依赖已就绪"

# ============================================================
# 构建 Docker 镜像（如果需要）
# ============================================================

if [ "$USE_DOCKER" = "true" ]; then
    echo -e "${YELLOW}[3/5] 检查 Docker 镜像...${NC}"

    IMAGE_NAME=$(grep "docker_image:" config.yaml | awk '{print $2}' | tr -d '"')

    if ! docker image inspect "$IMAGE_NAME" &> /dev/null; then
        echo "  构建 Docker 镜像 ($IMAGE_NAME)..."
        docker build -t "$IMAGE_NAME" ./worker
        echo "  ✓ Docker 镜像已构建"
    else
        echo "  ✓ Docker 镜像已存在"
    fi
else
    echo -e "${YELLOW}[3/5] 跳过 Docker（直接执行模式）${NC}"

    # 检查 claude 命令
    if ! command -v claude &> /dev/null; then
        echo -e "${RED}错误: 未找到 claude 命令${NC}"
        echo "  请安装: npm install -g @anthropic-ai/claude-code"
        exit 1
    fi
    echo "  ✓ Claude Code 已安装"
fi

# ============================================================
# 检查任务文件
# ============================================================

echo -e "${YELLOW}[4/5] 检查任务配置...${NC}"

if grep -q "这是一个示例任务" memory/TASKS.md 2>/dev/null; then
    echo -e "${YELLOW}  ⚠ TASKS.md 还是示例内容，请编辑后重新运行${NC}"
    echo ""
    echo "  vim memory/TASKS.md"
    echo ""
    read -p "  是否继续运行? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 0
    fi
fi
echo "  ✓ 任务文件已检查"

# ============================================================
# 显示配置
# ============================================================

echo -e "${YELLOW}[5/5] 当前配置:${NC}"
echo ""
echo "  最大迭代次数:  $(grep "max_iterations:" config.yaml | awk '{print $2}')"
echo "  最大成本:      \$$(grep "max_cost_usd:" config.yaml | awk '{print $2}')"
echo "  最大时长:      $(grep "max_duration_hours:" config.yaml | awk '{print $2}') 小时"
echo "  使用 Docker:   $USE_DOCKER"
echo ""

# ============================================================
# 启动
# ============================================================

echo -e "${GREEN}启动 Orchestrator...${NC}"
echo "  按 Ctrl+C 优雅退出"
echo ""

# 解析命令行参数
ARGS=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --max-iterations)
            ARGS="$ARGS --max-iterations $2"
            shift 2
            ;;
        --max-cost)
            ARGS="$ARGS --max-cost $2"
            shift 2
            ;;
        --max-duration)
            ARGS="$ARGS --max-duration $2"
            shift 2
            ;;
        --no-docker)
            ARGS="$ARGS --no-docker"
            shift
            ;;
        *)
            shift
            ;;
    esac
done

python3 orchestrator.py $ARGS
