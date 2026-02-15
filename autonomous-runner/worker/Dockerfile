# Claude Code Worker Container
# =============================
# 隔离、安全、可重复的执行环境

FROM node:20-slim

LABEL maintainer="Claude Hub"
LABEL description="Isolated worker for Claude Code autonomous execution"

# 安装系统依赖
RUN apt-get update && apt-get install -y \
    git \
    python3 \
    python3-pip \
    python3-venv \
    curl \
    jq \
    ripgrep \
    fd-find \
    && rm -rf /var/lib/apt/lists/*

# 安装 Claude Code
RUN npm install -g @anthropic-ai/claude-code

# 创建非 root 用户
RUN useradd -m -s /bin/bash worker
RUN mkdir -p /workspace /memory && chown -R worker:worker /workspace /memory

# 切换到工作用户
USER worker

# 设置工作目录
WORKDIR /workspace

# Git 配置（避免警告）
RUN git config --global user.email "claude-worker@localhost" && \
    git config --global user.name "Claude Worker" && \
    git config --global init.defaultBranch main

# 挂载点
VOLUME ["/workspace", "/memory"]

# 环境变量
ENV HOME=/home/worker
ENV TERM=xterm-256color

# 入口点
ENTRYPOINT ["claude"]
