<div align="center">

<img src="docs/assets/logo.svg" alt="Keepline Logo" width="120" />

# Keepline

### 再也不丢 Codex 和 Claude Code 的工作。

**给 Coding Agent 用的原生 macOS 工作台。每个会话都是一个真终端；Keepline 告诉你哪个在等你，关掉窗口它们照样跑，崩了也能接着聊。**

[![GitHub stars](https://img.shields.io/github/stars/majiayu000/keepline?style=flat-square&color=ffcc00)](https://github.com/majiayu000/keepline)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)

[从源码构建](#从源码构建) | [能做什么](#能做什么) | [常见问题](#常见问题) | [English](README.md)

</div>

---

> **现状：** [`next/`](next/) 里的工作台已经在日常使用，但还没有发布，没有安装包，需要从源码构建。上一代（Bun 命令行、网页仪表盘、Tauri 菜单栏，发布到 v1.1.4）的介绍在 [docs/previous-generation_CN.md](docs/previous-generation_CN.md)。

## 从源码构建

需要 macOS、Xcode、[Zig](https://ziglang.org/download/) 0.16.x 和 [rustup](https://rustup.rs)（Rust 版本固定在 `next/rust-toolchain.toml`）。

```sh
git clone https://github.com/majiayu000/keepline.git
cd keepline/next
./bootstrap.sh            # 拉取固定版本的 GPUI 和 libghostty 源码
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer ./run.sh
```

第一次构建还会下载 Ghostty 源码，要等一会儿。在 Keepline 的终端里运行 `claude` 或 `codex`，会话就会出现在左栏。

## 能做什么

### 01 · 一眼看出哪个 Agent 在等你

每个会话只有一个状态，左栏、标签、窗格和看板上的说法一致：**等你回答**（提问或审批）、**在跑**、**等你验收**（一轮跑完）、**做完了**。一轮跑完不等于活干完了，只有你点了标记完成才算做完。鸟瞰把所有会话按列摊开，底栏显示 Codex、Claude、Grok 的额度。

### 02 · 关掉窗口，它们照样跑

终端跑在后台进程（`keepline serve`）里，不在窗口里。退出、更新或 App 崩溃，再打开时终端都还在。连后台也没了（比如重启电脑）时，被打断的会话会显示**接着聊**：点一下就用 `claude --resume` 或 `codex resume` 回到同一段对话，启动时带的参数也一起带上。

### 03 · 什么 Agent 都能接

内置 Claude Code、Codex、Grok Build、Cursor Agent、Qoder CLI，Keepline 读它们的对话记录和 hook 来判断状态、找回答和验证证据。别的 CLI 用一个 JSON 插件接入（自带 OpenCode 示例）。任何程序都能用 OSC 7501 状态协议，一行 `printf` 报告自己的状态：

```sh
printf '\033]7501;state=working:app=deploy:progress=30\033\\'
```

### 04 · Agent 能驱动 Keepline，也能互相派活

在 Keepline 终端里，Agent 可以用 `keepline` 命令读别的终端、给别的 Agent 发话。`keepline run` 派另一个 Agent 去做一件事；它每结束一轮，结果会以 `<keepline from=…>` 自动回到派活那一方的输入框，谁都不用轮询。

```sh
keepline list
keepline run codex "审一下这个分支的改动，只列真正的 bug"
keepline --skill          # 写给 Agent 看的完整说明
```

### 05 · 用插件改它，或者让 Agent 替你改

插件就是 `~/.config/keepline/plugins/` 下的普通文件：右键菜单和 ⌘P 里的命令、左栏排法、窗格上显示的信息、状态变化时的 hook（通知、webhook）、验证规则、完成动画、新的 Agent CLI。直接跟终端里的 Agent 说，比如「加一个右键命令，在旁边跑 npm test」，它会读 `keepline --skill plugins`，写好插件并检查：

```sh
keepline plugins          # 哪些插件生效了，没生效的为什么
```

## 常见问题

**Agent 没出现在左栏。**
1. 要在 Keepline 的终端里运行；在别处启动的会话只在看板上以「外部·只读」出现。
2. 套在包装器（沙盒、虚拟机）里运行时，启动前设 `KEEPLINE_AGENT=claude`（或 `codex` 等），让 Keepline 认得它。
3. `keepline explain <终端>` 看 Keepline 怎么读这块屏；`keepline hooks status` 看 Agent 的 hook 装了没有、是不是最新。

**退出、更新或重启会怎样？**
退出或更新 App，终端都继续跑。重启电脑会结束它们，被打断的会话会提示在原目录接着聊。

**数据放在哪，会不会传出去？**
终端和会话索引在 `~/.local/share/keepline-workbench`，设置和插件在 `~/.config/keepline`。Keepline 只读 Agent 自己的对话记录，不上传。手机中继要你自己开启，内容端到端加密。

**Keepline 会改我的 Agent 配置吗？**
只有安装 hook（`keepline hooks install`）时会在 Agent 的配置文件里加上 Keepline 的条目，`keepline hooks uninstall` 会移除。不装 hook 时，状态从对话记录和画面读。

## 仓库结构

- `next/`：原生工作台。`crates/keepline-app` 是 GPUI 窗口，`crates/keeplined` 是后台和 `keepline` 命令，`crates/keepline-core` 是共用的模型和协议，`plugins/` 是示例插件。
- `specs/`：产品和设计记录。
- `src/`、`menubar-tauri/`、`sdk/`：上一代，见 [docs/previous-generation_CN.md](docs/previous-generation_CN.md)。

欢迎贡献，见 [CONTRIBUTING.md](CONTRIBUTING.md)。问题反馈：[GitHub Issues](https://github.com/majiayu000/keepline/issues)。

<div align="center">

MIT License

</div>
