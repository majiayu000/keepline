#!/usr/bin/env python3
"""生成 next/ 架构图：python3 specs/architecture/gen.py → specs/architecture/next.html

图都是声明式数据：盒子（中心或左上角 + 尺寸）和边（起止、弯曲、标注）。
改状态机时改这里的数据再重跑，别手改 HTML。
"""
import html
import math
from pathlib import Path

OUT = Path(__file__).with_name("next.html")


def esc(s):
    return html.escape(s, quote=True)


# ---------- 几何 ----------
class Box:
    def __init__(self, id, x, y, w, h, text, tone="", kind="", sub="", center=True):
        if center:
            x, y = x - w / 2, y - h / 2
        self.id, self.x, self.y, self.w, self.h = id, x, y, w, h
        self.text, self.tone, self.kind, self.sub = text, tone, kind, sub

    @property
    def cx(self):
        return self.x + self.w / 2

    @property
    def cy(self):
        return self.y + self.h / 2

    def edge(self, tx, ty):
        """从中心朝 (tx,ty) 的射线与矩形边的交点。"""
        dx, dy = tx - self.cx, ty - self.cy
        if dx == 0 and dy == 0:
            return self.cx, self.cy
        sx = (self.w / 2) / abs(dx) if dx else math.inf
        sy = (self.h / 2) / abs(dy) if dy else math.inf
        s = min(sx, sy)
        return self.cx + dx * s, self.cy + dy * s


def lines_of(text):
    return text.split("\n")


def text_block(x, y, text, cls, size=13, lh=None):
    lh = lh or size * 1.3
    ls = lines_of(text)
    y0 = y - (len(ls) - 1) * lh / 2
    out = [f'<text class="{cls}" x="{x:.1f}" y="{y0:.1f}" text-anchor="middle" dominant-baseline="central">']
    for i, l in enumerate(ls):
        out.append(f'<tspan x="{x:.1f}" dy="{0 if i == 0 else lh:.1f}">{esc(l)}</tspan>')
    out.append("</text>")
    return "".join(out)


def draw_box(b):
    cls = f"st {b.tone} {b.kind}".strip()
    r = 12 if b.kind != "mod" else 7
    out = [f'<g class="{cls}" data-id="{b.id}">']
    out.append(f'<rect x="{b.x:.1f}" y="{b.y:.1f}" width="{b.w:.1f}" height="{b.h:.1f}" rx="{r}"/>')
    if b.kind == "final":
        out.append(f'<rect class="inner" x="{b.x+4:.1f}" y="{b.y+4:.1f}" width="{b.w-8:.1f}" height="{b.h-8:.1f}" rx="{r-3}"/>')
    if b.sub:
        out.append(text_block(b.cx, b.cy - 8, b.text, "t-main", 14))
        out.append(text_block(b.cx, b.cy + 11, b.sub, "t-sub", 11))
    else:
        out.append(text_block(b.cx, b.cy, b.text, "t-main" if b.kind != "mod" else "t-mod", 13 if b.kind != "mod" else 12))
    out.append("</g>")
    return "".join(out)


def draw_group(x, y, w, h, title, cls="grp"):
    return (f'<g class="{cls}"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="14"/>'
            f'<text class="g-title" x="{x+14}" y="{y+22}">{esc(title)}</text></g>')


def label_at(x, y, text, cls="e-label", size=11.5):
    return text_block(x, y, text, cls, size, size * 1.35)


def point(boxes, p, toward):
    if isinstance(p, tuple):
        return p
    return boxes[p].edge(*toward)


def anchor(boxes, p):
    if isinstance(p, tuple):
        return p
    return boxes[p].cx, boxes[p].cy


def draw_edge(boxes, e):
    a, b = e["from"], e["to"]
    cls = "edge " + e.get("cls", "")
    marker = '' if "nohead" in e.get("cls", "") else 'marker-end="url(#ah)"'
    if e.get("both"):
        marker += ' marker-start="url(#ah)"'
    lbl = e.get("label", "")
    dx, dy = e.get("ldx", 0), e.get("ldy", 0)
    if a == b:  # 自环：画在盒子上方（或 side 指定方向）
        bx = boxes[a]
        side = e.get("side", "top")
        if side == "top":
            x1, y1 = bx.cx - 22, bx.y
            x2, y2 = bx.cx + 22, bx.y
            c1, c2 = (x1 - 18, y1 - 48), (x2 + 18, y2 - 48)
            lx, ly = bx.cx, bx.y - 52
        else:  # bottom
            x1, y1 = bx.cx - 22, bx.y + bx.h
            x2, y2 = bx.cx + 22, bx.y + bx.h
            c1, c2 = (x1 - 18, y1 + 48), (x2 + 18, y2 + 48)
            lx, ly = bx.cx, bx.y + bx.h + 54
        d = f"M{x1:.1f},{y1:.1f} C{c1[0]:.1f},{c1[1]:.1f} {c2[0]:.1f},{c2[1]:.1f} {x2:.1f},{y2:.1f}"
        s = f'<path class="{cls}" d="{d}" {marker}/>'
        if lbl:
            s += label_at(lx + dx, ly + dy, lbl, "e-label " + e.get("cls", ""))
        return s
    via = e.get("via")
    if via:  # 折线
        pts = [point(boxes, a, via[0])] + list(via) + [point(boxes, b, via[-1])]
        d = "M" + " L".join(f"{x:.1f},{y:.1f}" for x, y in pts)
        s = f'<path class="{cls}" d="{d}" {marker}/>'
        if lbl:
            i = e.get("seg", len(pts) // 2)
            mx = (pts[i - 1][0] + pts[i][0]) / 2
            my = (pts[i - 1][1] + pts[i][1]) / 2
            s += label_at(mx + dx, my + dy, lbl, "e-label " + e.get("cls", ""))
        return s
    ax, ay = anchor(boxes, a)
    bx_, by_ = anchor(boxes, b)
    bend = e.get("bend", 0)
    mx, my = (ax + bx_) / 2, (ay + by_) / 2
    length = math.hypot(bx_ - ax, by_ - ay) or 1
    nx, ny = -(by_ - ay) / length, (bx_ - ax) / length
    cx, cy = mx + nx * bend, my + ny * bend
    p0 = point(boxes, a, (cx, cy))
    p2 = point(boxes, b, (cx, cy))
    if bend:
        d = f"M{p0[0]:.1f},{p0[1]:.1f} Q{cx:.1f},{cy:.1f} {p2[0]:.1f},{p2[1]:.1f}"
        lx = 0.25 * p0[0] + 0.5 * cx + 0.25 * p2[0]
        ly = 0.25 * p0[1] + 0.5 * cy + 0.25 * p2[1]
    else:
        d = f"M{p0[0]:.1f},{p0[1]:.1f} L{p2[0]:.1f},{p2[1]:.1f}"
        lx, ly = (p0[0] + p2[0]) / 2, (p0[1] + p2[1]) / 2
    s = f'<path class="{cls}" d="{d}" {marker}/>'
    if e.get("num"):
        s += f'<g class="num"><circle cx="{lx+dx:.1f}" cy="{ly+dy:.1f}" r="10"/>' + text_block(lx + dx, ly + dy, e["num"], "num-t", 11) + "</g>"
    elif lbl:
        s += label_at(lx + dx, ly + dy, lbl, "e-label " + e.get("cls", ""))
    return s


def svg(w, h, boxes, edges, extra_before="", extra_after="", title=""):
    bmap = {b.id: b for b in boxes}
    out = [f'<svg viewBox="0 0 {w} {h}" role="img" aria-label="{esc(title)}" xmlns="http://www.w3.org/2000/svg">',
           '<defs><marker id="ah" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
           '<path d="M0,0 L10,5 L0,10 z" class="ah"/></marker></defs>', extra_before]
    for e in edges:
        out.append(draw_edge(bmap, e))
    for b in boxes:
        out.append(draw_box(b))
    out.append(extra_after)
    out.append("</svg>")
    return "\n".join(out)


def init_dot(x, y, to_box, label=""):
    bx, by = to_box.edge(x, y)
    s = f'<circle class="init" cx="{x}" cy="{y}" r="7"/><path class="edge" d="M{x+7},{y} L{bx:.1f},{by:.1f}" marker-end="url(#ah)"/>'
    if label:
        s += label_at((x + bx) / 2, y - 14, label)
    return s


# ---------- 图 1：系统总图 ----------
def fig_system():
    B = []
    g = []
    g.append(draw_group(20, 60, 240, 600, "Agent CLI · 终端里的进程"))
    for i, (id_, t) in enumerate([("claude", "Claude Code"), ("codex", "Codex"),
                                  ("others", "Grok · Cursor · Qoder"), ("declared", "插件声明的 Agent\n(opencode …)")]):
        B.append(Box(id_, 35, 100 + i * 58, 210, 48, t, "", "mod", center=False))
    B.append(Box("logs", 35, 380, 210, 52, "对话日志 JSONL\n~/.claude · ~/.codex …", "", "mod", center=False))
    B.append(Box("hookcli", 35, 452, 210, 52, "CLI 自带 Hook\n(已装进 settings.json)", "", "mod", center=False))
    B.append(Box("officialcli", 35, 524, 210, 52, "claude agents --json\n官方会话状态", "", "mod", center=False))

    g.append(draw_group(300, 60, 520, 600, "keeplined serve · 宿主（每用户一个，持有全部 PTY）", "grp host"))
    g.append(draw_group(315, 95, 240, 250, "终端", "grp sub"))
    for i, t in enumerate(["pty.rs\n生成 PTY · SIGHUP 结束", "host/live.rs\nVT 快照 · 广播给观看者",
                           "command_marks\nOSC 133 命令起止", "handoff.rs\n升级时交出 PTY fd"]):
        B.append(Box(f"t{i}", 330, 128 + i * 52, 210, 46, t, "", "mod", center=False))
    g.append(draw_group(315, 360, 240, 290, "识别 · 读信号", "grp sub"))
    for i, t in enumerate(["file_events + log_index\nFSEvents → 日志增量索引", "keeplined hook\n接收 Agent Hook",
                           "official.rs\n读官方状态（按需）", "recognition\nps / lsof 绑定对话",
                           "screen_rules\n安静画面里认提问框"]):
        B.append(Box(f"r{i}", 330, 392 + i * 51, 210, 45, t, "", "mod", center=False))
    g.append(draw_group(600, 95, 205, 250, "对外", "grp sub"))
    B.append(Box("control", 612, 128, 180, 52, "control.rs\nworkbench.sock", "", "mod", center=False))
    B.append(Box("remote", 612, 196, 180, 52, "remote.rs · relay.rs\n远程会话（Noise）", "", "mod", center=False))
    B.append(Box("cliapi", 612, 264, 180, 52, "keeplined list · split\n给终端里的 Agent 用", "", "mod", center=False))
    g.append(draw_group(600, 360, 205, 290, "状态", "grp sub"))
    B.append(Box("content", 612, 392, 180, 52, "content/\n回合解析 → Turn", "run", "mod", center=False))
    B.append(Box("store", 612, 460, 180, 52, "store.rs\nWorkspace · SQLite", "", "mod", center=False))
    B.append(Box("phooks", 612, 528, 180, 52, "plugin_hooks\nhooks.json 通知", "", "mod", center=False))
    B.append(Box("diag", 612, 596, 180, 44, "diagnostics · quota", "", "mod", center=False))

    g.append(draw_group(860, 60, 300, 390, "keepline-app · 桌面工作台（GPUI）", "grp app"))
    for i, t in enumerate(["workbench/\n侧栏 · 看板 · 标签 · 命令面板", "terminal/\n终端窗格 · libghostty-vt",
                           "model/session_state\n派生 Group · Phase", "keepline-plugin-host\nwasmtime 侧栏插件沙箱",
                           "plugins/\nactions · decorations · rules"]):
        B.append(Box(f"a{i}", 875, 98 + i * 68, 270, 56, t, "", "mod", center=False))

    g.append(draw_group(860, 470, 300, 190, "远程", "grp"))
    B.append(Box("relay", 875, 508, 128, 60, "keepline-relay\naxum WebSocket", "", "mod", center=False))
    B.append(Box("mobile", 1017, 508, 128, 60, "mobile/\nExpo · React Native", "", "mod", center=False))
    note = label_at(1010, 600, "中继只转发密文；手机按角色\n只看 / 能答审批 / 完全控制", "e-label")

    core = ('<g class="grp core"><rect x="20" y="680" width="1140" height="40" rx="10"/>'
            '<text class="t-mod" x="590" y="700" text-anchor="middle" dominant-baseline="central">'
            'keepline-core（共享类型）：Workspace 模型 · Request/Reply 协议 · Content / Turn / Inbox · Agent 定义 · 布局 · 插件清单'
            '　｜　keepline-remote：远程协议 · 配对链接 · Noise 通道 · 中继帧</text></g>')

    E = [
        {"from": (245, 180), "to": (330, 151), "both": True, "num": "1"},
        {"from": "logs", "to": "r0", "num": "2"},
        {"from": "hookcli", "to": "r1", "num": "3"},
        {"from": "officialcli", "to": "r2", "num": "4"},
        {"from": (555, 470), "to": (612, 418), "num": "5"},
        {"from": "content", "to": "store", "cls": "thin"},
        {"from": "store", "to": "control", "via": [(805, 486), (812, 486), (812, 154), (805, 154)], "cls": "thin"},
        {"from": (540, 203), "to": (612, 168), "num": "6"},
        {"from": "control", "to": "a0", "both": True, "num": "7"},
        {"from": "remote", "to": "relay", "both": True, "via": [(838, 222), (838, 538)]},
        {"from": "relay", "to": "mobile", "both": True},
        {"from": "cliapi", "to": (245, 290), "cls": "dash", "via": [(600, 330), (270, 330)]},
    ]
    nums8 = '<g class="num"><circle cx="838" cy="380" r="10"/>' + text_block(838, 380, "8", "num-t", 11) + "</g>"
    return svg(1180, 730, B, E, extra_before="".join(g), extra_after=note + core + nums8, title="系统总图")


# ---------- 图 2：信号 → 状态 的管线 ----------
def fig_pipeline():
    B = []
    src = [("s0", "对话日志 JSONL"), ("s1", "CLI Hook 事件"), ("s2", "安静画面的底部"),
           ("s3", "claude agents --json"), ("s4", "子 Agent 日志"), ("s5", "进程表 ps / lsof")]
    ing = ["log_index 增量读", "keeplined hook", "screen_rules", "official.rs", "children()", "recognition"]
    for i, (id_, t) in enumerate(src):
        B.append(Box(id_, 110, 50 + i * 56, 170, 42, t, "", "mod"))
        B.append(Box(f"i{i}", 320, 50 + i * 56, 150, 42, ing[i], "", "mod"))
    B.append(Box("card", 570, 160, 190, 250, "Content 卡片\n\nturn  回合状态\nwaiting  在等什么\nbackground  后台任务\nevidence  测试/提交/改文件\nsignal  哪个信号定的", "run", "mod"))
    B.append(Box("bind", 570, 330, 190, 50, "AgentSession\nbinding · current", "", "mod"))
    B.append(Box("user", 570, 395, 190, 42, "你的操作 → Edit", "user", "mod"))
    B.append(Box("ws", 800, 220, 170, 90, "Workspace\nrevision += 1\n存 SQLite · 推送", "", "mod"))
    B.append(Box("inbox", 1050, 110, 220, 60, "Content::group → Inbox\n收件箱 / 看板列", "wait", "mod"))
    B.append(Box("phase", 1050, 200, 220, 60, "session_state → Group·Phase\n侧栏圆点 / 窗格", "wait", "mod"))
    B.append(Box("notify", 1050, 290, 220, 60, "hook Event → hooks.json\nstarted/waiting/done/error", "", "mod"))
    B.append(Box("phone", 1050, 375, 220, 50, "远程 cards → 手机", "", "mod"))
    E = []
    for i in range(6):
        E.append({"from": f"s{i}", "to": f"i{i}", "cls": "thin"})
    for i in range(5):
        E.append({"from": f"i{i}", "to": (475, 160 - 60 + i * 30), "cls": "thin"})
    E.append({"from": "i5", "to": "bind", "cls": "thin"})
    E += [{"from": (665, 160), "to": "ws"}, {"from": "bind", "to": "ws"}, {"from": "user", "to": "ws"},
          {"from": "ws", "to": "inbox"}, {"from": "ws", "to": "phase"}, {"from": "ws", "to": "notify"},
          {"from": "ws", "to": "phone"}]
    cap = label_at(570, 438, "Outcome 只由你改（标记完成 / 稍后再看 / 放弃）", "e-label user")
    return svg(1180, 460, B, E, extra_after=cap, title="信号到状态的管线")


# ---------- 状态机 ----------
def fig_process():
    B = [Box("starting", 150, 150, 150, 56, "Starting", "", sub="启动中"),
         Box("running", 420, 150, 150, 56, "Running", "run", sub="在跑"),
         Box("draining", 690, 150, 150, 56, "Draining", "wait", sub="已退出 · 读余下输出"),
         Box("exited", 960, 150, 150, 56, "Exited", "done", "final", sub="结束（含退出码）"),
         Box("failed", 150, 340, 150, 56, "Failed", "err", "final", sub="没起来"),
         Box("interrupted", 555, 340, 170, 56, "Interrupted", "err", "final", sub="宿主重启 · 原 PTY 丢了")]
    E = [
        {"from": "starting", "to": "running", "label": "pty::spawn 成功\n写入 pid", "ldy": -26},
        {"from": "starting", "to": "failed", "label": "spawn 报错\n记 error", "ldx": -46},
        {"from": "running", "to": "draining", "label": "子进程退出\n输出未读完", "ldy": 24},
        {"from": "draining", "to": "exited", "label": "输出读完", "ldy": -16},
        {"from": "running", "to": "exited", "bend": -95, "label": "退出时输出已读完"},
        {"from": "running", "to": "running", "label": "新宿主接管：fd 经 SCM_RIGHTS 传过去，状态不变", "ldy": -2},
        {"from": "starting", "to": "interrupted", "bend": 30, "cls": "err"},
        {"from": "running", "to": "interrupted", "cls": "err"},
        {"from": "draining", "to": "interrupted", "cls": "err", "label": "宿主重启时还活着的终端\n写 lost_agents，可用原 CLI 恢复", "ldx": 120, "ldy": 0},
    ]
    return svg(1080, 420, B, E, extra_before=init_dot(30, 150, B[0], ""), title="终端进程状态机")


def fig_turn():
    B = [Box("unknown", 95, 230, 130, 56, "Unknown", "muted", sub="没读到开头"),
         Box("running", 360, 230, 150, 56, "Running", "run", sub="在跑"),
         Box("waiting", 680, 90, 150, 56, "Waiting", "wait", sub="等你回答"),
         Box("interrupted", 1010, 90, 150, 56, "Interrupted", "muted", sub="中断了"),
         Box("completed", 680, 390, 150, 56, "Completed", "done", sub="这一轮停了"),
         Box("error", 360, 470, 150, 56, "Error", "err", sub="出错了")]
    E = [
        {"from": "unknown", "to": "running", "label": "用户发话 · UserPromptSubmit\nPre/PostToolUse", "ldy": -30},
        {"from": "running", "to": "waiting", "bend": -60,
         "label": "AskUserQuestion / request_user_input\n工具结果「需要审批」\nPermissionRequest · Notification Hook\n画面规则 · 官方 Waiting · 子 Agent 在等",
         "ldx": -230, "ldy": -40},
        {"from": "waiting", "to": "running", "bend": -60,
         "label": "工具结果回来（已回答）\nHook 等待后的 PostToolUse\n官方 Busy · 子 Agent 不等了", "ldx": 60, "ldy": -10},
        {"from": "waiting", "to": "interrupted", "label": "官方 Idle\n等待中没收到 Stop", "ldy": 24},
        {"from": "waiting", "to": "completed", "label": "官方 Idle\n等待中已收到 Stop", "ldx": 70, "ldy": 40},
        {"from": "running", "to": "completed", "bend": 40, "label": "Stop Hook\n日志回合结束", "ldx": 45, "ldy": 15},
        {"from": "completed", "to": "running", "bend": 40, "label": "新的用户发话\n后台在跑时又来工具调用（被唤醒）", "ldx": 70, "ldy": -6},
        {"from": "running", "to": "interrupted", "via": [(360, 24), (1060, 24)], "both": True, "label": "→ 日志：turn_aborted / aborted　　← 新的用户发话", "ldx": 120, "ldy": -2},
        {"from": "running", "to": "error", "label": "StopFailure Hook\n日志 API 错误", "ldx": -70},
        {"from": "error", "to": "running", "bend": 60, "cls": "thin", "label": "新的用户发话", "ldx": 25, "ldy": 70},
    ]
    note = label_at(910, 240, "等待中收到 Stop：仍留在 Waiting\n记下 completed_while_waiting", "e-label")
    return svg(1110, 560, B, E, extra_before=init_dot(20, 160, B[0], ""), extra_after=note, title="回合状态机")


def fig_outcome():
    B = [Box("open", 260, 190, 160, 60, "Open", "user", sub="没定结局"),
         Box("completed", 680, 70, 170, 60, "Completed", "done", sub="做完了 ✓（你标的）"),
         Box("deferred", 680, 190, 170, 60, "Deferred{until}", "wait", sub="稍后再看"),
         Box("abandoned", 680, 310, 170, 60, "Abandoned", "muted", sub="不做了")]
    E = [
        {"from": "open", "to": "completed", "bend": 20, "label": "标记完成", "ldy": -16},
        {"from": "open", "to": "deferred", "bend": -28, "label": "稍后再看（定时间）", "ldy": -12},
        {"from": "open", "to": "abandoned", "bend": -20, "label": "放弃", "ldy": 14},
        {"from": "deferred", "to": "open", "bend": -28, "label": "到期 expire_outcomes", "ldy": 12},
        {"from": "completed", "to": "open", "via": [(900, 70), (900, 420), (260, 420)], "cls": "thin",
         "label": "Agent 开了新回合（turn_key 变了）→ 自动回到 Open，记进 outcome_history", "seg": 4, "ldy": 14},
        {"from": "abandoned", "to": (900, 310), "cls": "thin nohead"},
        {"from": "deferred", "to": (900, 190), "cls": "thin nohead"},
    ]
    return svg(960, 450, B, E, extra_before=init_dot(60, 190, B[0], "新会话"), title="会话结局状态机")


def fig_binding():
    B = [Box("seen", 170, 120, 200, 64, "只在日志里看到", "muted", sub="terminal = None · 不打扰你"),
         Box("unbound", 170, 320, 200, 64, "终端里在跑，未对上", "wait", sub="窗格提示「未对上对话：」"),
         Box("bound", 560, 220, 210, 70, "绑定 · 当前对话", "run", sub="current = true · binding = …"),
         Box("hist", 940, 220, 190, 64, "历史对话", "muted", sub="current = false · 保留 terminal")]
    E = [
        {"from": "seen", "to": "bound", "label": "唯一的证据指向某个终端", "ldy": -16},
        {"from": "unbound", "to": "bound", "label": "证据唯一时绑定\n有歧义就拒绝、继续等", "ldy": 18},
        {"from": "bound", "to": "hist", "label": "终端退出 · 宿主重启\n同一终端换了对话", "ldy": -26},
        {"from": "hist", "to": "bound", "bend": -70, "label": "恢复：用 agent_launch 里的原参数\n在终端里 resume 这段对话", "ldy": 30},
    ]
    legend = label_at(560, 410, "证据强弱：Process（进程报出会话 ID）> Hook（CLI Hook 上报）> OpenFile（进程开着对话文件）> Screen（最新提示词只在一块屏上）\n同一 cwd 从来不算证据；每个终端最多一个当前对话", "e-label")
    return svg(1080, 440, B, E, extra_before=init_dot(30, 120, B[0], "") + init_dot(30, 320, B[1], ""), extra_after=legend, title="对话绑定")


def fig_recognition():
    B = [Box("starting", 150, 110, 150, 56, "Starting", "muted", sub="启动"),
         Box("healthy", 420, 110, 150, 56, "Healthy", "done", sub="识别正常"),
         Box("interrupted", 420, 300, 170, 56, "Interrupted", "err", sub="识别中断"),
         Box("recovering", 760, 300, 170, 56, "Recovering", "wait", sub="补扫中")]
    E = [
        {"from": "starting", "to": "healthy", "label": "FSEvents 订阅就绪", "ldy": -16},
        {"from": "healthy", "to": "interrupted", "bend": 40, "cls": "err", "label": "事件不完整 · ID 回绕\n订阅出错（从检查点重订）", "ldx": -95},
        {"from": "interrupted", "to": "recovering", "bend": -30, "label": "补扫范围先写盘\n重启也能续上", "ldy": -26},
        {"from": "recovering", "to": "healthy", "bend": 30, "label": "补扫完成", "ldx": 30, "ldy": -14},
        {"from": "recovering", "to": "interrupted", "bend": -30, "cls": "err", "label": "日志目录暂不可用\n等目录重建的事件", "ldy": 26},
    ]
    return svg(900, 400, B, E, extra_before=init_dot(30, 110, B[0], ""), title="识别健康状态机")


def fig_remote():
    B = [Box("off", 120, 110, 130, 56, "Off", "muted", sub="没开远程"),
         Box("connecting", 380, 110, 160, 56, "Connecting", "wait", sub="连中继 · 重连等待"),
         Box("connected", 690, 110, 160, 56, "Connected", "done", sub="在线 · 手机可连"),
         Box("failed", 380, 300, 160, 56, "Failed", "err", "final", sub="不再重试 · detail 说原因")]
    E = [
        {"from": "off", "to": "connecting", "label": "remote --relay 启动", "ldy": -16},
        {"from": "connecting", "to": "connected", "bend": -30, "label": "注册成功（退避重置为 1 秒）", "ldy": -14},
        {"from": "connected", "to": "connecting", "bend": -30, "label": "和中继断开 → 1s 起翻倍，最多 30s", "ldy": 14},
        {"from": "connecting", "to": "failed", "cls": "err", "label": "中继拒绝这台机器\n进程退出前上报", "ldx": -70},
        {"from": "connected", "to": "failed", "cls": "err"},
    ]
    note = label_at(700, 290, "每台手机：offline → connecting → online / refused\n角色变更 Role · Kick 断开重连 · Revoke 解除配对", "e-label")
    return svg(860, 380, B, E, extra_before=init_dot(20, 110, B[0], ""), extra_after=note, title="远程连接状态机")


def fig_handoff():
    W, H = 900, 400
    xs = {"new": 200, "old": 520, "pty": 790}
    out = [f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="宿主升级交接时序" xmlns="http://www.w3.org/2000/svg">',
           '<defs><marker id="ah2" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" class="ah"/></marker></defs>']
    for k, t in [("new", "新 keeplined serve"), ("old", "旧 keeplined serve"), ("pty", "shell / Agent")]:
        x = xs[k]
        out.append(f'<g class="st mod"><rect x="{x-90}" y="20" width="180" height="40" rx="8"/></g>')
        out.append(text_block(x, 40, t, "t-main", 13))
        out.append(f'<line class="life" x1="{x}" y1="60" x2="{x}" y2="{H-20}"/>')
    msgs = [("new", "old", "Hello{major}", 95), ("old", "new", "Hello（大版本一致）", 130),
            ("new", "old", "Handoff", 165), ("old", "new", "Handoff{terminals}", 200),
            ("old", "new", "每个终端：SNAPSHOT 帧 … READY", 235), ("old", "new", "PTY 主端 fd（SCM_RIGHTS）", 270),
            ("new", "old", "ACK HANDOFF_TAKEN", 305)]
    for a, b, t, y in msgs:
        x1, x2 = xs[a], xs[b]
        d = 1 if x2 > x1 else -1
        out.append(f'<path class="edge" d="M{x1},{y} L{x2 - d*4},{y}" marker-end="url(#ah2)"/>')
        out.append(label_at((x1 + x2) / 2, y - 11, t))
    out.append(label_at(520, 345, "旧宿主退出", "e-label err"))
    out.append(label_at(790, 200, "全程不知情\n进程照跑", "e-label"))
    out.append(label_at(200, 345, "以 Reaper::Pid 盯住进程\n退出码此后未知", "e-label"))
    out.append("</svg>")
    return "\n".join(out)


def ladder(rows):
    out = ['<ol class="ladder">']
    for cond, chip, tone in rows:
        out.append(f'<li><span class="cond">{cond}</span><span class="arrow">→</span><span class="chip {tone}">{esc(chip)}</span></li>')
    out.append("</ol>")
    return "".join(out)


INBOX = ladder([
    ("结局是 <code>Completed</code> 或 <code>Abandoned</code>", "做完了", "done"),
    ("结局是 <code>Deferred</code> 且没到期", "稍后再看", "wait"),
    ("没在跑，且距上一轮（或上次验收/到期）超过 <code>stale_days</code> 天", "很久没动", "muted"),
    ("<code>turn = Waiting</code>", "等你回答", "wait"),
    ("<code>turn = Error</code>，或所在终端 Failed / Interrupted / 退出码 ≠ 0", "出错了", "err"),
    ("<code>turn = Completed</code>，是当前对话，后台还有子 Agent / 监视 / 定时唤醒", "在跑", "run"),
    ("<code>turn = Completed</code>（不管 Agent 说没说完成）", "等你验收", "user"),
    ("<code>turn = Running</code>，且是当前对话或 15 分钟内有更新", "在跑", "run"),
    ("其余（Interrupted、Unknown、日志里的 Running 已过期）", "停下了", "muted"),
])

PANE = ladder([
    ("<code>turn = Running</code>，或 Completed 但子 Agent 还在跑", "Working · 在跑", "run"),
    ("<code>turn = Waiting</code>", "Waiting · 等你回答（要你注意）", "wait"),
    ("Idle 且还没开始第一轮（没提问、turn = Unknown）", "Fresh · 新开的", "muted"),
    ("Idle 且结局 Completed", "Closed · 做完了", "done"),
    ("Idle 且 turn = Error", "Error · 出错了（要你注意）", "err"),
    ("Idle 且后台有 shell / 监视 / 定时唤醒", "Background · 等后台", "muted"),
    ("Idle，这一轮在你没看着时停的", "Done · 等你验收（要你看一眼）", "user"),
    ("Idle，其余", "Ready · 等你验收", "user"),
])

CODE = [
    ("终端进程", "next/crates/keepline-core/src/model.rs:256", "ProcessState；转移在 host/client.rs:459、host/live.rs:178、store.rs:100"),
    ("回合", "next/crates/keepline-core/src/content.rs:37", "Turn；转移在 keeplined/src/content/mod.rs（message / phase / tool / hook_event / official / children / settle）"),
    ("结局", "next/crates/keepline-core/src/model.rs:266", "Outcome；到期 model.rs:422；新回合重开 host/association.rs:72"),
    ("收件箱分组", "next/crates/keepline-core/src/content.rs:426", "Content::group，先命中先算"),
    ("窗格 / 侧栏", "next/crates/keepline-app/src/model/session_state.rs:262", "sidebar_status · idle_status · track_unseen"),
    ("绑定", "next/crates/keeplined/src/recognition.rs:446", "reconcile；Binding 定义在 model.rs:206"),
    ("识别健康", "next/crates/keeplined/src/host/association.rs:271", "recover_logs；RecognitionPhase 在 protocol.rs:364"),
    ("远程", "next/crates/keeplined/src/relay.rs:140", "Live::state；RemoteState 在 protocol.rs:218；手机端 next/mobile/src/remote/hub.ts"),
    ("宿主交接", "next/crates/keeplined/src/host/mod.rs:650", "take_over；fd 传递在 handoff.rs"),
]

CSS = """
:root{--bg:#f6f5f2;--card:#ffffff;--fg:#1d1f23;--muted:#6b7079;--line:#c9ccd2;--grp:#eceae5;--sub:#f9f8f5;
--run:#2f6fdb;--run-bg:#e8f0fe;--wait:#b7791f;--wait-bg:#fdf3e1;--done:#1f8a5b;--done-bg:#e3f5ec;
--err:#cc3d3d;--err-bg:#fbe9e9;--mut:#7a808a;--mut-bg:#eef0f2;--user:#7651c8;--user-bg:#f0eafc;--edge:#5d636d}
@media (prefers-color-scheme: dark){:root:not([data-theme="light"]){--bg:#15171a;--card:#1e2125;--fg:#e7e8ea;--muted:#9aa0a8;--line:#3a3f46;--grp:#1b1d21;--sub:#202328;
--run:#7aa7ff;--run-bg:#1c2a44;--wait:#e3b05a;--wait-bg:#3a2e17;--done:#5fcf98;--done-bg:#173327;--err:#ff8080;--err-bg:#3d1d1d;--mut:#a3a9b2;--mut-bg:#262a30;--user:#b49cff;--user-bg:#2c2440;--edge:#a6acb5}}
:root[data-theme="dark"]{--bg:#15171a;--card:#1e2125;--fg:#e7e8ea;--muted:#9aa0a8;--line:#3a3f46;--grp:#1b1d21;--sub:#202328;
--run:#7aa7ff;--run-bg:#1c2a44;--wait:#e3b05a;--wait-bg:#3a2e17;--done:#5fcf98;--done-bg:#173327;--err:#ff8080;--err-bg:#3d1d1d;--mut:#a3a9b2;--mut-bg:#262a30;--user:#b49cff;--user-bg:#2c2440;--edge:#a6acb5}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.6 -apple-system,BlinkMacSystemFont,"PingFang SC","Helvetica Neue",sans-serif}
main{max-width:1240px;margin:0 auto;padding:40px 16px 80px}
h1{font-size:28px;margin:0 0 6px}h2{font-size:20px;margin:48px 0 6px}h3{font-size:16px;margin:24px 0 6px}
p.lead{color:var(--muted);margin:0 0 20px;max-width:820px}p.note{color:var(--muted);max-width:900px;margin:6px 0 12px}
nav{display:flex;flex-wrap:wrap;gap:8px;margin:18px 0 8px}nav a{color:var(--fg);text-decoration:none;border:1px solid var(--line);border-radius:999px;padding:3px 12px;font-size:13px;background:var(--card)}
.fig{background:var(--card);border:1px solid var(--line);border-radius:16px;padding:16px;margin:12px 0;overflow:hidden}
.fig svg{width:100%;height:auto;display:block}
.two{display:grid;grid-template-columns:1fr 1fr;gap:16px}@media (max-width:900px){.two{grid-template-columns:1fr}}
.legend{display:flex;flex-wrap:wrap;gap:14px;font-size:13px;color:var(--muted);margin:8px 0 0}
.legend span::before{content:"";display:inline-block;width:12px;height:12px;border-radius:3px;margin-right:6px;vertical-align:-1px;border:1.5px solid currentColor}
.legend .run{color:var(--run)}.legend .wait{color:var(--wait)}.legend .done{color:var(--done)}.legend .err{color:var(--err)}.legend .muted{color:var(--mut)}.legend .user{color:var(--user)}
ol.flows{margin:8px 0 0;padding-left:22px;font-size:14px}ol.flows li{margin:2px 0}
code{font:12.5px ui-monospace,SFMono-Regular,Menlo,monospace;background:var(--mut-bg);padding:1px 5px;border-radius:5px}
ol.ladder{list-style:none;counter-reset:n;margin:0;padding:0}
ol.ladder li{counter-increment:n;display:grid;grid-template-columns:28px 1fr 24px auto;align-items:center;gap:8px;padding:7px 0;border-bottom:1px dashed var(--line);font-size:14px}
ol.ladder li::before{content:counter(n);width:22px;height:22px;border-radius:50%;background:var(--mut-bg);color:var(--muted);font-size:12px;display:grid;place-items:center}
.arrow{color:var(--muted);text-align:center}
.chip{border-radius:999px;padding:2px 10px;font-size:13px;white-space:nowrap;border:1.5px solid}
.chip.run{color:var(--run);background:var(--run-bg)}.chip.wait{color:var(--wait);background:var(--wait-bg)}.chip.done{color:var(--done);background:var(--done-bg)}
.chip.err{color:var(--err);background:var(--err-bg)}.chip.muted{color:var(--mut);background:var(--mut-bg)}.chip.user{color:var(--user);background:var(--user-bg)}
table{border-collapse:collapse;width:100%;font-size:14px}td,th{border-bottom:1px solid var(--line);padding:8px 6px;text-align:left;vertical-align:top}th{color:var(--muted);font-weight:500}
.toggle{float:right;border:1px solid var(--line);background:var(--card);color:var(--fg);border-radius:8px;padding:4px 10px;cursor:pointer;font-size:13px}
/* svg */
.grp rect{fill:var(--grp);stroke:var(--line);stroke-width:1}.grp.sub rect{fill:var(--sub);stroke-dasharray:4 3}
.grp.host rect{stroke:var(--run);stroke-width:1.4}.grp.app rect{stroke:var(--user);stroke-width:1.4}.grp.core rect{fill:var(--mut-bg)}
.g-title{fill:var(--muted);font-size:13px;font-weight:600}
.st rect{fill:var(--card);stroke:var(--edge);stroke-width:1.6}.st .inner{fill:none;stroke-width:1}
.st.mod rect{stroke:var(--line);stroke-width:1.2}
.st.run rect{fill:var(--run-bg);stroke:var(--run)}.st.wait rect{fill:var(--wait-bg);stroke:var(--wait)}.st.done rect{fill:var(--done-bg);stroke:var(--done)}
.st.err rect{fill:var(--err-bg);stroke:var(--err)}.st.muted rect{fill:var(--mut-bg);stroke:var(--mut)}.st.user rect{fill:var(--user-bg);stroke:var(--user)}
.st.run .inner{stroke:var(--run)}.st.done .inner{stroke:var(--done)}.st.err .inner{stroke:var(--err)}
.t-main{fill:var(--fg);font-size:14px;font-weight:600}.t-sub{fill:var(--muted);font-size:11px}.t-mod{fill:var(--fg);font-size:12px}
.edge{fill:none;stroke:var(--edge);stroke-width:1.5}.edge.thin{stroke-width:1.1;opacity:.75}.edge.err{stroke:var(--err)}.edge.dash{stroke-dasharray:5 4;opacity:.8}
.ah{fill:var(--edge)}.init{fill:var(--fg)}
.e-label{fill:var(--fg);font-size:11.5px;paint-order:stroke;stroke:var(--card);stroke-width:5px;stroke-linejoin:round}
.e-label.err{fill:var(--err)}.e-label.user{fill:var(--user)}
.num circle{fill:var(--fg)}.num-t{fill:var(--card);font-size:11px;font-weight:700}
.life{stroke:var(--line);stroke-dasharray:4 4}
"""

LEGEND = ('<div class="legend"><span class="run">在跑</span><span class="wait">等待 / 过渡</span><span class="done">正常结束</span>'
          '<span class="err">出错 / 中断</span><span class="muted">中性</span><span class="user">由你决定</span>'
          '<span style="color:var(--muted)">双边框 = 终态</span></div>')


def page():
    code_rows = "".join(f"<tr><td>{esc(a)}</td><td><code>{esc(b)}</code></td><td>{esc(c)}</td></tr>" for a, b, c in CODE)
    return f"""<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Keepline next 架构</title><style>{CSS}</style></head>
<body><main>
<button class="toggle" onclick="var r=document.documentElement;r.dataset.theme=(r.dataset.theme==='dark'||(!r.dataset.theme&&matchMedia('(prefers-color-scheme: dark)').matches))?'light':'dark'">明 / 暗</button>
<h1>Keepline next 架构与状态机</h1>
<p class="lead">next/ 是一个 Rust workspace：<b>keeplined</b> 宿主持有全部终端和识别逻辑，<b>keepline-app</b> 是 GPUI 桌面工作台，两者只通过 <code>workbench.sock</code> 说话；手机经 <b>keepline-relay</b> 远程连同一个宿主。所有状态都存在宿主的 <code>Workspace</code> 里，界面上的分组是从它派生出来的，不另存。</p>
<nav><a href="#sys">系统总图</a><a href="#pipe">信号 → 状态</a><a href="#layers">状态分层</a><a href="#proc">终端进程</a><a href="#turn">回合</a><a href="#outcome">结局</a><a href="#inbox">分组</a><a href="#bind">绑定</a><a href="#recog">识别健康</a><a href="#remote">远程</a><a href="#handoff">宿主交接</a><a href="#code">代码索引</a></nav>

<h2 id="sys">1 · 系统总图</h2>
<p class="note">三个进程：Agent CLI 跑在宿主开的 PTY 里；宿主从四路读信号；工作台和手机都只是宿主的观看者和遥控器。</p>
<div class="fig">{fig_system()}</div>
<ol class="flows">
<li>Agent 跑在宿主的 PTY 里，键盘输入和屏幕字节都经过 <code>pty.rs</code>。PATH 带着 keeplined，终端里的 Agent 也能调 <code>keeplined list / split</code>（虚线）。</li>
<li>CLI 写对话日志，FSEvents 通知宿主增量读；中途丢事件就从检查点补扫。</li>
<li>CLI 自带的 Hook 运行 <code>"$KEEPLINE_HOOK" hook --agent …</code>，靠 <code>KEEPLINE_TERMINAL</code> 指明是哪个终端。</li>
<li>只在需要确认「还在等吗」时调一次 <code>claude agents --json</code>，从不定时轮询。</li>
<li>识别层产出的信号都汇到 <code>content/</code>，改的是一张 Content 卡片上的 <code>turn</code>，再写回 Workspace。</li>
<li>终端画面（VT 快照 + 增量帧）直接从 live 推给观看者，不经 SQLite。</li>
<li>工作台订阅 Workspace 的每次 revision；它的所有改动都以 <code>Edit</code> 请求发回宿主。</li>
<li>远程会话跑在 Noise 通道里，中继看不到内容。</li>
</ol>

<h2 id="pipe">2 · 一个信号怎么变成你看到的状态</h2>
<p class="note">六路信号，只有一张卡片。回合状态由信号改，结局只由你改；界面上的分组、圆点和通知都是 Workspace 的派生结果。</p>
<div class="fig">{fig_pipeline()}</div>

<h2 id="layers">3 · 状态分层</h2>
<table><tr><th>层</th><th>存在哪</th><th>谁改</th><th>状态</th></tr>
<tr><td>终端进程</td><td><code>Terminal.state</code></td><td>宿主（spawn、退出、重启）</td><td>Starting · Running · Draining · Exited · Interrupted · Failed</td></tr>
<tr><td>对话绑定</td><td><code>AgentSession.terminal / current / binding</code></td><td>recognition</td><td>只在日志里 · 未对上 · 当前 · 历史</td></tr>
<tr><td>回合</td><td><code>Content.turn</code></td><td>日志 / Hook / 画面 / 官方状态 / 子 Agent</td><td>Unknown · Running · Waiting · Completed · Interrupted · Error</td></tr>
<tr><td>结局</td><td><code>AgentSession.outcome</code></td><td><b>你</b>（新回合会自动重开）</td><td>Open · Completed · Deferred · Abandoned</td></tr>
<tr><td>分组（派生）</td><td>不存，现算</td><td><code>Content::group</code> · <code>sidebar_status</code></td><td>收件箱 8 组；窗格 Working / Waiting / Fresh / Background / Done / Ready …</td></tr>
<tr><td>系统健康</td><td>宿主内存，推给 app</td><td>识别订阅 · 远程连接</td><td>RecognitionPhase · RemoteState</td></tr>
</table>

<h2 id="proc">4 · 终端进程 <code>ProcessState</code></h2>
<p class="note">一个 Terminal 是一个 PTY，它的 shell 可以先后跑很多段对话。关掉窗格不等于结束：PTY 身份和摆放位置是分开的。</p>
<div class="fig">{fig_process()}{LEGEND}</div>

<h2 id="turn">5 · 回合 <code>Turn</code></h2>
<p class="note">核心状态机。每次变化由 <code>settle()</code> 记下是哪路信号定的（日志 / Hook / 画面 / 官方状态 / 子 Agent），进入 Waiting 时 <code>waiting_episode += 1</code>，同一个对话框被多路信号报到只算一次通知。任何状态收到新的用户发话都回到 Running。</p>
<div class="fig">{fig_turn()}{LEGEND}</div>

<h2 id="outcome">6 · 结局 <code>Outcome</code>：做没做完由你定</h2>
<p class="note">Agent 说「完成了」只记成 <code>completed_claim</code>，不改结局。一轮停了就是「等你验收」，只有你点「标记完成」才进 Completed。</p>
<div class="fig">{fig_outcome()}{LEGEND}</div>

<h2 id="inbox">7 · 分组：从状态派生，先命中先算</h2>
<p class="note">分组不存储，每次渲染从 Outcome + Turn + 终端状态现算。左边用于收件箱和看板列，右边用于 Keepline 终端里正活着的 Agent（侧栏圆点、窗格）。</p>
<div class="two"><div class="fig"><h3>收件箱 · <code>Content::group</code></h3>{INBOX}</div>
<div class="fig"><h3>活着的窗格 · <code>sidebar_status</code></h3>{PANE}
<p class="note">「没看着时停的」由 <code>track_unseen</code> 记：Phase 从非 Idle 变 Idle 且窗格没显示，就标为未看；显示出来或再动起来就清掉。</p></div></div>

<h2 id="bind">8 · 对话绑定</h2>
<p class="note">对话身份是 (runtime, cli_session_id)，和 PTY、摆放位置都无关。宿主尝试 CLI 提供的每种证据，有歧义宁可不绑。</p>
<div class="fig">{fig_binding()}{LEGEND}</div>

<div><div><h2 id="recog">9 · 识别健康 <code>RecognitionPhase</code></h2>
<p class="note">没有轮询兜底：日志全靠 FSEvents。丢事件时按目录子树补扫，补扫范围先持久化。</p>
<div class="fig">{fig_recognition()}</div></div>
<div><h2 id="remote">10 · 远程 <code>RemoteState</code></h2>
<p class="note"><code>keeplined remote --relay</code> 主动连出去，状态经 <code>RemoteReport</code> 报给宿主再给 app。</p>
<div class="fig">{fig_remote()}</div></div>
</div>

<h2 id="handoff">11 · 宿主升级交接</h2>
<p class="note">app 发现正在跑的宿主和自己带的不是同一份代码（digest 不同），就启动新宿主接管。终端里的 shell 和 Agent 不会察觉。</p>
<div class="fig">{fig_handoff()}</div>

<h2 id="code">12 · 代码索引</h2>
<table><tr><th>状态</th><th>定义</th><th>转移在哪</th></tr>{code_rows}</table>
<p class="note">本页由 <code>specs/architecture/gen.py</code> 生成，改图请改脚本里的数据后重跑。生成于 2026-11-02，对应 main 6dc82074。</p>
</main></body></html>
"""


if __name__ == "__main__":
    OUT.write_text(page(), encoding="utf-8")
    print(OUT)
