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
         Box("failed", 380, 300, 160, 56, "Failed", "err", sub="不再重试 · 等你改配置")]
    E = [
        {"from": "off", "to": "connecting", "label": "remote --relay 启动", "ldy": -16},
        {"from": "connecting", "to": "connected", "bend": -30, "label": "注册成功（退避重置为 1 秒）", "ldy": -14},
        {"from": "connected", "to": "connecting", "bend": -30, "label": "和中继断开 → 1s 起翻倍，最多 30s", "ldy": 14},
        {"from": "connecting", "to": "failed", "cls": "err", "label": "中继拒绝这台机器\n进程退出前上报", "ldx": -70},
        {"from": "connected", "to": "failed", "cls": "err", "label": "同一配对码的新连接顶替\n中继发关闭码 4000", "ldx": 80, "ldy": 10},
        {"from": "failed", "to": "connecting", "bend": 40, "cls": "thin", "label": "改配置\n（仍开着）", "ldx": 52},
        {"from": "failed", "to": "off", "cls": "thin", "label": "关掉远程", "ldx": -40, "ldy": 10},
    ]
    note = label_at(430, 405, "任何状态下改配置：开着 → Connecting，关掉 → Off（configure 换 generation、杀掉旧进程）\n连中继的进程被信号杀掉：回 Connecting，1s 起翻倍、最多 30s 后重启\n每台手机：offline → connecting → online / refused · Role 改角色 · Kick 断开 · Revoke 解除配对", "e-label")
    return svg(860, 460, B, E, extra_before=init_dot(20, 110, B[0], ""), extra_after=note, title="远程连接状态机")


def fig_handoff():
    W, H = 900, 470
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
    out.append(f'<line class="life" x1="40" y1="318" x2="{W-40}" y2="318" stroke-dasharray="6 4"/>')
    out.append(label_at(450, 395, "虚线以上（ACK 之前）失败：旧宿主回「handoff not confirmed」，继续服务\n虚线以下（ACK 之后）新宿主还要等 host.lock ≤10s、建 socket、开数据库、恢复终端；\n这时失败旧宿主已经退出，没有回退", "e-label err"))
    out.append("</svg>")
    return "\n".join(out)


def seq(W, H, parts, steps, title):
    """时序图。parts: [(key, x, 名字)]；steps: ("msg", a, b, 文本, y, cls) 或 ("self", a, 文本, y, cls)。"""
    xs = {k: x for k, x, _ in parts}
    out = [f'<svg viewBox="0 0 {W} {H}" role="img" aria-label="{esc(title)}" xmlns="http://www.w3.org/2000/svg">',
           '<defs><marker id="ah3" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" class="ah"/></marker></defs>']
    for k, x, t in parts:
        out.append(f'<g class="st mod"><rect x="{x-95}" y="16" width="190" height="42" rx="8"/></g>')
        out.append(text_block(x, 37, t, "t-main", 13))
        out.append(f'<line class="life" x1="{x}" y1="58" x2="{x}" y2="{H-12}"/>')
    for s in steps:
        if s[0] == "msg":
            _, a, b, t, y, cls = s
            x1, x2 = xs[a], xs[b]
            d = 1 if x2 > x1 else -1
            out.append(f'<path class="edge {cls}" d="M{x1},{y} L{x2 - d*4},{y}" marker-end="url(#ah3)"/>')
            out.append(label_at((x1 + x2) / 2, y - 12, t, "e-label " + cls))
        else:
            _, a, t, y, cls = s
            n = len(lines_of(t))
            h = 14 + n * 16
            w = 300
            out.append(f'<g class="st {cls} mod"><rect x="{xs[a]-w/2}" y="{y}" width="{w}" height="{h}" rx="7"/></g>')
            out.append(text_block(xs[a], y + h / 2, t, "t-mod", 12, 16))
    out.append("</svg>")
    return "\n".join(out)


def fig_task():
    B = [Box("not", 110, 110, 150, 56, "未开始", "muted", sub="NotStarted"),
         Box("working", 400, 110, 150, 56, "进行中", "run", sub="Working"),
         Box("proven", 690, 110, 160, 56, "都有证据了", "done", sub="Proven · 仍待你确认"),
         Box("done", 990, 110, 150, 56, "做完了", "done", sub="Completed"),
         Box("derive", 200, 330, 190, 64, "按证据重新派生", "", "mod", sub="→ 未开始 / 进行中 / 都有证据了"),
         Box("deferred", 560, 330, 160, 56, "稍后再看", "wait", sub="Deferred{until}"),
         Box("abandoned", 900, 330, 150, 56, "放弃", "muted", sub="Abandoned")]
    E = [
        {"from": "not", "to": "working", "label": "有会话挂上这个待办", "ldy": -16},
        {"from": "working", "to": "proven", "bend": -20, "label": "每条验收标准都挂上\n成功的测试 / 提交 / 改文件", "ldy": -30},
        {"from": "proven", "to": "working", "bend": -20, "label": "新增一条空标准 / 移除证据\n（只改文字会保留证据）", "ldy": 22},
        {"from": "proven", "to": "done", "label": "标记完成（主按钮）", "ldy": -16},
        {"from": "working", "to": "done", "via": [(400, 30), (990, 30)], "cls": "thin", "label": "未开始 / 进行中也可直接标记完成（不强制证据）", "seg": 2, "ldy": -10},
        {"from": "working", "to": "deferred", "label": "稍后再看\n（同样任何状态）", "ldx": 50},
        {"from": "proven", "to": "abandoned", "cls": "thin", "label": "放弃\n（未定结局的任何状态都可）", "ldx": 70},
        {"from": "deferred", "to": "derive", "cls": "thin", "label": "到期 / 撤销结局", "ldy": -16},
        {"from": "abandoned", "to": "derive", "via": [(900, 420), (200, 420)], "cls": "thin", "label": "撤销结局", "seg": 2, "ldy": 14},
        {"from": "done", "to": "derive", "via": [(1080, 110), (1080, 420), (200, 420)], "cls": "thin", "label": "撤销结局", "seg": 2, "ldx": -36},
    ]
    note = label_at(560, 470, "状态不存储：task_status 每次从结局 + 证据 + 挂着的会话现算；结局回到 Open 后落在哪一格，看那一刻的证据", "e-label")
    return svg(1110, 490, B, E, extra_before=init_dot(20, 110, B[0], ""), extra_after=note, title="待办状态")


def fig_criterion():
    B = [Box("none", 150, 70, 170, 56, "没有证据", "muted"),
         Box("tried", 480, 70, 170, 56, "只有尝试", "wait", sub="失败的测试留作记录"),
         Box("proven", 810, 70, 170, 56, "已证明", "done", sub="proves() = true")]
    E = [
        {"from": "none", "to": "tried", "label": "挂上一条失败的测试", "ldy": -16},
        {"from": "tried", "to": "proven", "label": "再挂上一条成功的记录", "ldy": -16},
        {"from": "none", "to": "proven", "bend": 55, "label": "直接挂上成功的 test / commit / file（退出码 0）", "ldx": -200, "ldy": 22},
        {"from": "proven", "to": "tried", "via": [(810, 200), (480, 200)], "cls": "thin", "label": "移除成功的那条证据：还剩失败的 → 只有尝试；什么都不剩 → 没有证据", "seg": 2, "ldy": 14},
    ]
    return svg(960, 235, B, E, title="验收标准证据")


def fig_notify():
    B = [Box("ws", 110, 190, 170, 70, "Workspace\n新 revision", "", "mod")]
    lane_a = [("a1", "entered()\n只看 等你回答 / 出错了\n跳过你正看着的终端"),
              ("a2", "去重 key\n分组:回合:开始时间:\n对话框序号 或 错误原文"),
              ("a3", "ClaimNotification\nSQLite 每会话一条回执\nkey 变了才算新"),
              ("a4", "系统通知 + 声音\n只开声音时只响一下")]
    lane_b = [("b1", "Hooks::changes()\nstarted · waiting\nerror · done"),
              ("b2", "每回合 / 每个对话框\n只触发一次\n宿主刚启动只记录"),
              ("b3", "每个 hook 一个线程\n超时 ≤ 30s · 同时最多 8 个\n满了直接丢弃，只记日志"),
              ("b4", "command：stdin JSON\n+ KEEPLINE_* 变量\n或 webhook POST")]
    for i, (id_, t) in enumerate(lane_a):
        B.append(Box(id_, 350 + i * 215, 95, 190, 74, t, "wait" if i == 3 else "", "mod"))
    for i, (id_, t) in enumerate(lane_b):
        B.append(Box(id_, 350 + i * 215, 285, 190, 74, t, "", "mod"))
    E = [{"from": "ws", "to": "a1"}, {"from": "ws", "to": "b1"}]
    for p in ("a", "b"):
        for i in range(1, 4):
            E.append({"from": f"{p}{i}", "to": f"{p}{i+1}"})
    g = (draw_group(245, 22, 880, 130, "App（desktop.rs）· 系统通知", "grp app")
         + draw_group(245, 212, 880, 130, "宿主（plugin_hooks）· hooks.json 插件，App 没开也会发", "grp host"))
    cmd = label_at(670, 375, "另：Shell 用 OSC 133 标出命令结束 → command_done / command_failed 也走 hooks.json", "e-label")
    return svg(1140, 390, B, E, extra_before=g, extra_after=cmd, title="通知链路")


def fig_approval():
    parts = [("agent", 150, "Agent 画面（PTY）"), ("host", 520, "keeplined 宿主"), ("ui", 890, "工作台 / 手机")]
    steps = [
        ("msg", "agent", "host", "输出停了 1 秒（SCREEN_QUIET）", 90, ""),
        ("self", "host", "screen_rules 在底部认出审批框\n回合 → Waiting，信号记为「画面」\ncontent.approval = 问题原文", 108, "wait"),
        ("self", "agent", "你 3 秒内刚按过键：先不算\n（多半是你自己打开的菜单）", 108, ""),
        ("msg", "host", "ui", "新 revision：卡片带上 approval", 200, ""),
        ("msg", "ui", "host", "AnswerApproval{终端, 问题, 允许?}", 245, ""),
        ("self", "ui", "手机要有 answer 或 full 角色\nview 只能看", 262, "user"),
        ("self", "host", "重读画面：还是同一个问题吗？\n允许时：光标在第一项吗？", 262, ""),
        ("msg", "host", "agent", "允许 → 回车 · 拒绝 → Esc", 345, ""),
        ("msg", "host", "ui", "对不上：「画面上已经没有这个审批了」", 385, "err"),
        ("self", "host", "清掉这块屏的 screen wait\n之后的工具结果把回合拉回 Running", 405, ""),
    ]
    return seq(1040, 470, parts, steps, "审批应答时序")


def fig_conn():
    B = [Box("attach", 130, 150, 160, 60, "Attaching", "wait", sub="发 Attach · 收快照"),
         Box("owner", 430, 70, 160, 56, "可写", "run", sub="这条连接是 owner"),
         Box("viewer", 430, 240, 160, 56, "只读", "muted", sub="别处在写 / 观看模式"),
         Box("ended", 760, 70, 160, 56, "Ended", "done", "final", sub="终端不再 Running"),
         Box("dropped", 760, 240, 160, 56, "Dropped", "err", sub="流断了，画面还在")]
    E = [
        {"from": "attach", "to": "owner", "label": "READY · takeover\n或还没人写", "ldy": -26},
        {"from": "attach", "to": "viewer", "label": "READY · 已有 owner", "ldy": 22},
        {"from": "owner", "to": "viewer", "bend": 25, "label": "别的窗口抢走写权", "ldx": 70},
        {"from": "viewer", "to": "owner", "bend": 25, "label": "WRITABLE 帧\n轮到你写", "ldx": -60},
        {"from": "owner", "to": "ended", "label": "STATUS 帧：进程退出", "ldy": -16},
        {"from": "viewer", "to": "ended", "cls": "thin"},
        {"from": "owner", "to": "dropped", "cls": "err", "label": "看得太慢被丢 · 写入失败\n（输入结果未知，绝不自动重发）", "ldx": 50, "ldy": -20},
        {"from": "viewer", "to": "dropped", "cls": "err"},
        {"from": "dropped", "to": "attach", "via": [(760, 330), (130, 330)], "cls": "thin", "label": "重新 attach，失败间隔翻倍；快照恢复画面", "seg": 2, "ldy": 14},
    ]
    return svg(900, 370, B, E, extra_before=init_dot(20, 150, B[0], ""), title="窗格连接")


def fig_hostlife():
    B = [Box("none", 120, 110, 150, 56, "没有宿主", "muted"),
         Box("same", 440, 110, 170, 56, "宿主在跑 · 同版本", "run"),
         Box("stale", 780, 110, 170, 56, "宿主在跑 · 旧版本", "wait", sub="App 顶部提示有新后台"),
         Box("take", 780, 300, 170, 56, "接管中", "wait", sub="serve --takeover ≤ 20s"),
         Box("fresh", 440, 300, 170, 56, "停旧宿主 · 起新的", "err", sub="终端变 Interrupted")]
    # 新宿主收到 fd 并回 ACK 后，旧宿主立即退出
    E = [
        {"from": "none", "to": "same", "label": "spawn_host：setsid 起 serve\n等 workbench.sock 应答 ≤ 5s", "ldy": -26},
        {"from": "same", "to": "stale", "label": "App 升级，digest 不同", "ldy": -16},
        {"from": "stale", "to": "take", "label": "你点重启\n旧宿主支持交接", "ldx": 62},
        {"from": "take", "to": "same", "label": "新 build 应答\n终端照跑", "ldx": -105, "ldy": -40},
        {"from": "take", "to": "stale", "bend": -60, "cls": "err", "label": "ACK 前失败\n旧宿主照常运行", "ldx": -60},
        {"from": "take", "to": "none", "via": [(780, 372), (120, 372)], "cls": "err", "label": "ACK 后新宿主起不来：旧宿主已退出，不能回退", "seg": 2, "ldy": 14},
        {"from": "stale", "to": "fresh", "cls": "thin", "label": "旧宿主不支持交接", "ldx": 25, "ldy": 45},
        {"from": "fresh", "to": "same", "label": "App 重开连上新宿主\n可用原参数恢复对话", "ldx": -80},
        {"from": "same", "to": "none", "bend": -40, "cls": "thin", "label": "宿主崩了 / 被杀", "ldy": 14},
    ]
    return svg(920, 410, B, E, extra_before=init_dot(20, 110, B[0], ""), title="宿主生命周期")


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

def table(head, rows):
    h = "".join(f"<th>{c}</th>" for c in head)
    b = "".join("<tr>" + "".join(f"<td>{c}</td>" for c in r) + "</tr>" for r in rows)
    return f'<div class="tbl"><table><tr>{h}</tr>{b}</table></div>'


Y, N = '<span class="yes">●</span>', '<span class="no">—</span>'
AGENTS = table(
    ["", "日志位置", "进程报出会话", "Hook", "官方状态", "子 Agent", "画面规则", "额度"],
    [
        ["<b>Claude Code</b>", "<code>~/.claude/projects</code>", f"{Y} <code>--session-id</code> / resume · <code>claude agents --json</code> 按 pid", f"{Y} 自动装进 settings.json", f"{Y} <code>claude agents --json</code>", "Stop Hook 带后台任务数", f"{Y} claude.json", f"{Y} statusline"],
        ["<b>Codex</b>", "<code>~/.codex/sessions</code>", f"{Y} resume UUID；<code>--no-daemon</code> 时可用 lsof", N, f"{Y} app-server <code>thread/read</code>", f"{Y} 读子线程日志", f"{Y} codex.json", f"{Y} 日志"],
        ["<b>Grok</b>", "<code>~/.grok/sessions</code>", f"{Y} <code>active_sessions.json</code>", N, N, N, "借用 Claude 的", f"{Y} 启动日志"],
        ["<b>Cursor</b>", "<code>~/.cursor/projects</code>", f"{Y} <code>--resume</code>", N, N, N, "借用 Claude 的", N],
        ["<b>Qoder</b>", "<code>~/.qoder/projects</code>", f"{Y} 同 Claude 参数", N, N, N, "借用 Claude 的", N],
        ["<b>声明式（插件）</b>", "agent.json 指定；SQLite 的经 mirror 转 JSONL", "按声明", N, N, N, "借用 Claude 的", N],
    ])

PLUGINS = table(
    ["种类", "文件", "挂在哪", "运行在", "仓库里的例子"],
    [
        ["Agent", "<code>agent.json</code>", "识别：教宿主认一个新 CLI 和它的日志", "宿主", "opencode"],
        ["动作", "<code>actions.json</code>", "窗格右键菜单、⌘P", "App", "common-actions"],
        ["验证规则", "<code>rules.json</code>", "哪些命令算「跑过测试」→ 证据", "宿主", "project-checks"],
        ["左栏", "<code>sidebar.json</code>（可带 wasm）", "左栏怎么分组排序", "App · wasmtime 沙箱，无文件无网络", "session-first · focus-sidebar · sidebar-status"],
        ["Hooks", "<code>hooks.json</code>", "会话进入 started / waiting / error / done，命令结束", "宿主 · 独立线程", "notify-macos"],
        ["窗格装饰", "<code>decorations.json</code>", "窗格标题栏的短信息", "App", "pane-facts"],
        ["动效", "effects", "标记完成时的动画", "App", "complete-stamp · complete-sweep · complete-dissolve"],
    ])

LAYOUT = table(
    ["操作（Edit）", "对布局", "对终端进程"],
    [
        ["<code>PutTab</code>", "新建或改一个标签及其分割树", "—"],
        ["<code>SplitPane</code>", "把还没摆出来的终端放到目标旁边（<code>keeplined split</code> 也走这个），基于宿主当时的布局，不覆盖期间的改动", "—"],
        ["<code>ClosePane</code>", "从标签里拿掉，最后一个窗格连标签一起关", "宿主 SIGHUP 停掉这个终端"],
        ["<code>CloseTab</code>", "删掉标签", "停掉它里面所有终端"],
        ["<code>RemoveSpace</code>", "关掉空间里所有标签；只剩一个空间或还有没收尾的待办时拒绝", "停掉这些终端"],
        ["<code>MoveTab</code>", "调整标签顺序", "—"],
    ])


CODE = [
    ("终端进程", "next/crates/keepline-core/src/model.rs:256", "ProcessState；转移在 host/client.rs:459、host/live.rs:178、store.rs:100"),
    ("回合", "next/crates/keepline-core/src/content.rs:37", "Turn；转移在 keeplined/src/content/mod.rs（message / phase / tool / hook_event / official / children / settle）"),
    ("结局", "next/crates/keepline-core/src/model.rs:266", "Outcome；到期 model.rs:422；新回合重开 host/association.rs:72"),
    ("收件箱分组", "next/crates/keepline-core/src/content.rs:426", "Content::group，先命中先算"),
    ("窗格 / 侧栏", "next/crates/keepline-app/src/model/session_state.rs:262", "sidebar_status · idle_status · track_unseen"),
    ("绑定", "next/crates/keeplined/src/recognition.rs:446", "reconcile；Binding 定义在 model.rs:206"),
    ("识别健康", "next/crates/keeplined/src/host/association.rs:271", "recover_logs；RecognitionPhase 在 protocol.rs:364"),
    ("远程", "next/crates/keeplined/src/relay.rs:140", "Live::state；关闭码 4000 在 relay.rs:629；改配置 / 关闭在 host/remote.rs:99；RemoteState 在 protocol.rs:218；手机端 next/mobile/src/remote/hub.ts"),
    ("宿主交接", "next/crates/keeplined/src/host/mod.rs:650", "take_over；ACK 在 :687，旧宿主收 ACK 即退出在 host/client.rs:139；fd 传递在 handoff.rs"),
    ("待办状态", "next/crates/keepline-app/src/workbench/task_detail.rs:29", "task_status；证据 attachable 在同文件；Evidence::proves 在 keepline-core/src/content.rs:31"),
    ("待办挂到会话", "next/crates/keepline-core/src/model.rs:705", "首次绑定时消耗 pending_tasks 或标签 default_task"),
    ("系统通知", "next/crates/keepline-app/src/desktop.rs:78", "fact / entered；回执 keeplined/src/store.rs:128"),
    ("hooks.json", "next/crates/keeplined/src/plugin_hooks.rs:28", "event / changes；满 8 个丢弃在 :146"),
    ("额度", "next/crates/keepline-core/src/quota.rs:1", "latest 取每家最新读数；SHOWN_FOR 7 天；Window::reset"),
    ("审批", "next/crates/keeplined/src/host/client.rs:819", "answer_approval；画面检查 host/status.rs:182；角色 keepline-remote/src/role.rs:33"),
    ("窗格连接", "next/crates/keepline-app/src/connection.rs:80", "Attach / 帧处理；owner 判定 keeplined/src/host/live.rs:401"),
    ("宿主生命周期", "next/crates/keepline-app/src/lifecycle.rs:150", "spawn_host / restart_host"),
    ("各家 Adapter", "next/crates/keeplined/src/agents/mod.rs:85", "Adapter trait；画面规则 agents/screen/*.json"),
    ("插件种类", "next/crates/keepline-core/src/plugin.rs:17", "Kind"),
]


# ───────── 第二轮：数据一致性、并发、故障、信任边界、App、手机 ─────────

def fig_edit():
    B = [Box("local", 110, 90, 170, 60, "本地构造 Edit", "", sub="带上本地 revision"),
         Box("sent", 360, 90, 170, 60, "发出 RPC", "run", sub="读超时 10s · 不重放"),
         Box("check", 620, 90, 190, 60, "宿主拿全局锁", "run", sub="查 revision（布局类豁免）"),
         Box("applied", 880, 90, 170, 60, "副本上应用 + 落库", "run", sub="一个事务 · fsync"),
         Box("live", 1110, 90, 170, 60, "换内存 · 广播", "done", "final", sub="回 Workspace 快照"),
         Box("dropped", 110, 270, 170, 60, "静默丢弃", "err", "final", sub="忙时 · 非布局类"),
         Box("queued", 360, 270, 170, 60, "排队", "wait", sub="忙时 · 只有布局类"),
         Box("conflict", 620, 270, 190, 60, "「工作台已变化」", "err", sub="全局 revision 不符"),
         Box("invalid", 880, 270, 170, 60, "校验 / 落库失败", "err", sub="内存不动"),
         Box("result", 1110, 270, 170, 60, "客户端只写 error", "err", "final", sub="表单不放回 · 提示不撤回")]
    E = [
        {"from": "local", "to": "sent", "label": "不忙", "ldy": -14},
        {"from": "local", "to": "dropped", "cls": "err", "label": "忙 · 其余\n编辑", "ldx": -36},
        {"from": "local", "to": "queued", "cls": "thin", "label": "忙 · 布局类", "ldx": 30, "ldy": -20},
        {"from": "queued", "to": "sent", "cls": "thin", "label": "前一个回包后", "ldx": 50},
        {"from": "sent", "to": "check"},
        {"from": "check", "to": "applied", "label": "通过", "ldy": -14},
        {"from": "applied", "to": "live", "label": "成功", "ldy": -14},
        {"from": "check", "to": "conflict", "cls": "err", "label": "期间有别的提交\n（会话内容约每秒一次）", "ldx": 78},
        {"from": "applied", "to": "invalid", "cls": "err", "label": "出错", "ldx": 22},
        {"from": "invalid", "to": "result", "cls": "err"},
        {"from": "conflict", "to": "result", "via": [(620, 345), (1110, 345)], "cls": "err", "seg": 2, "label": "不重试", "ldy": 12},
    ]
    return svg(1220, 370, B, E, extra_before=init_dot(20, 90, B[0], ""), title="一次编辑的生命周期")


def fig_appreq():
    B = [Box("idle", 140, 100, 160, 56, "空闲", "muted"),
         Box("busy", 450, 100, 170, 56, "忙", "run", sub="300ms 后显示「正在更新…」"),
         Box("reply", 780, 100, 170, 56, "回包", "done", sub="Workspace / 错误"),
         Box("swallow", 450, 280, 330, 56, "忙时又点了一下", "err", sub="布局类排队 · 其余丢弃 · 新建 / 重开直接 return")]
    E = [
        {"from": "idle", "to": "busy", "label": "发出请求", "ldy": -14},
        {"from": "busy", "to": "reply", "label": "宿主回复", "ldy": -14},
        {"from": "reply", "to": "idle", "via": [(780, 30), (140, 30)], "seg": 2, "label": "end_busy · 取下一个排队请求 · 若有 start_after_save 就启动", "ldy": -10},
        {"from": "busy", "to": "swallow", "cls": "err"},
    ]
    return svg(940, 370, B, E, extra_before=init_dot(20, 100, B[0], ""), title="App 请求状态")


def fig_phone():
    B = [Box("unpaired", 110, 110, 150, 56, "未配对", "muted"),
         Box("connecting", 400, 110, 170, 56, "connecting", "wait", sub="Noise 握手 → hello"),
         Box("online", 740, 110, 160, 56, "online", "done", sub="角色定在连接时"),
         Box("offline", 400, 310, 170, 56, "offline", "err", sub="1s 起翻倍 ≤ 30s"),
         Box("refused", 960, 310, 160, 56, "refused", "err", sub="只能手动点重连")]
    E = [
        {"from": "unpaired", "to": "connecting", "label": "扫码 / 深链\n（先保存，不确认）", "ldy": -22},
        {"from": "connecting", "to": "online", "label": "hello 返回 role/ops", "ldy": -14},
        {"from": "connecting", "to": "offline", "bend": 40, "cls": "err", "label": "连不上\n握手失败", "ldx": -62},
        {"from": "offline", "to": "connecting", "bend": 40, "cls": "thin", "label": "退避到期\n回到前台", "ldx": 48},
        {"from": "online", "to": "offline", "cls": "err", "label": "断开（非 4401）", "ldx": 62, "ldy": 14},
        {"from": "online", "to": "refused", "cls": "err", "label": "4401：撤销 / 改角色 / 踢", "ldx": 92},
    ]
    note = label_at(540, 400, "宿主每 1 秒复查设备、角色和踢出时间，不符就用 4401 断开；改角色、踢出后其实点重连就能立刻连回\n所有请求都没有超时，半开连接会一直停在 online", "e-label")
    return svg(1100, 430, B, E, extra_before=init_dot(20, 110, B[0], ""), extra_after=note, title="手机连接状态")


LOCKS = table(
    ["锁", "保护什么", "谁拿", "持锁时做的阻塞操作"],
    [
        ["<code>Shared = Mutex&lt;Host&gt;</code>", "Workspace、Store、索引、pid 表、订阅者", "几乎所有线程：客户端、PTY pump 的副作用、回收、定时、订阅", "<b>fsync</b>（每次 commit，<code>synchronous=FULL</code>）· <code>pty::spawn</code> · <code>/bin/ps</code> · 读对话记录 · 等日志库 gate · 交接时写 socket"],
        ["<code>Mutex&lt;Live&gt;</code>（每终端一把）", "VT、PTY master、订阅者、owner、是否退出", "pump、attach、回收、<code>send_*</code>、关联", "<code>pty::write</code>（每次 poll 最多 5s）"],
        ["<code>replies</code>", "VT 的查询应答", "pump（在 Live 锁内）", "—（叶子锁）"],
        ["日志库 <code>gate</code>", "日志游标、SQLite 写入", "日志订阅线程、Hook 请求", "整批 <code>BEGIN IMMEDIATE</code> + 解析文件"],
        ["<code>Supervisor.inner</code> + Condvar", "远程配置、子进程 pid、generation", "Supervisor 线程、configure", "—"],
    ])

FAILURES = table(
    ["故障", "用户看到什么", "能否恢复"],
    [
        ["宿主被杀，终端在跑", "shell 和 Agent 收到 SIGHUP；窗格每 ≤5s 重连；显示「连接中断」", "要你点「重新连接后台」；重启后在跑的终端改成 Interrupted，记进 <code>lost_agents</code>，可用原 CLI 恢复"],
        ["App 崩溃 / 退出", "终端不受影响", "重新 attach 用快照恢复画面，识别状态自动重订"],
        ["FSEvents 丢事件", "先持久化补扫范围再补扫；失败显示「补扫未完成」", "能续扫；但补扫失败时同一批的其他事件被丢（问题 I15）"],
        ["SQLite 忙 / 磁盘满", "前台请求报错；<b>后台写失败只打日志</b>", "不自愈（I14）；库损坏时宿主起不来，只提示看 host.log"],
        ["日志被截断 / 轮转 / 删除", "读取位置重置；删除只删游标，卡片保留", "能自愈"],
        ["回合 Waiting 时 Agent 退出", "卡片一直「等你回答」", "<b>不自愈</b>，3 天后才转「过时」（I13）"],
        ["中继宕机 / 手机审批中途掉线", "中继子进程 1→30s 重连", "审批重试会得到「已经答过」；同问句的下一个审批会被答到（I8）"],
        ["交接 ACK 后新宿主失败", "全部终端挂断", "没有回退（I26）"],
        ["时钟跳变", "到期、过时都在渲染时现算", "基本无害"],
    ])

TRUST = table(
    ["输入源", "能造成什么", "门控"],
    [
        ["本地 socket 客户端", "所有 Request，包括生成配对链接、改中继", "只看是不是同一个用户；runtime 目录 0700、socket 0600"],
        ["Agent hook", "绑定会话、改回合、触发插件 hook", "调用进程是受管终端的后代，<b>或</b>请求里写了在跑的终端 id（只防误报）"],
        ["磁盘上的 Agent 日志", "标题、等待、出错、回复，流进插件的环境变量和 webhook", "无；不拼进命令"],
        ["PTY 画面", "判断审批 / 等待；OSC 7 给出工作目录", "只有画面规则匹配"],
        ["插件目录", "<code>sh -c</code> 执行命令、发 webhook、菜单动作", "只校验格式，<b>默认全部启用</b>"],
        ["wasm 左栏", "只能返回渲染 JSON", "无 import、内存上限、epoch 超时"],
        ["手机经中继", "list / read / send / keys / answer / chat / events", "Noise psk 握手 → hello → <code>role.allows</code> → 操作白名单"],
        ["中继服务器", "看路由 token 和流量大小；能断开、重放握手、顶替注册", "看不到明文；机器注册只靠 token_hash，中继 key 可选"],
    ])

DERIVE = table(
    ["同一个会话", "侧栏 <code>sidebar_status</code>", "看板 <code>board_group</code>", "core <code>Content::group</code>（菜单栏、通知、状态栏）"],
    [
        ["活着、回合出错", "出错", "等你验收（无标记）", "出错"],
        ["Agent 停了、只剩后台 shell", "等后台", "—", "等你验收"],
        ["已完成的会话（待办详情里的列表）", "—", "—", "<code>session_group</code> 映射成「停下了」"],
        ["刚新开的会话", "新开的", "—", "peek 里显示「停下了」"],
    ])


# 问题清单：(编号, 族, 严重度, 核对程度, 层, 标题, 场景, 证据)
FAMILIES = [
    ("A", "编辑被拒或丢掉，界面却不知道",
     "全局一个 revision 当所有编辑的乐观锁；客户端没有「这次请求成功了吗」的状态，忙时直接丢。",
     "按对象版本或字段合并代替全局 revision；客户端给每个请求一个挂起态和回执，忙时排队不丢，失败把输入还给你。"),
    ("B", "审批和按键可能落到别的问题上",
     "审批的身份只是画面上那一行问句，不含命令，也没有实例编号；按键完全不核对画面。",
     "给每个审批一个实例身份（问句 + 证据行 + waiting_episode），回答和手机按键都带上它；手机展示命令本身。"),
    ("C", "布局整块替换",
     "PutTab 用客户端快照整块覆盖标签，又豁免了版本检查。",
     "布局编辑改成操作型（加窗格、改名、调比例），或者给每个标签单独的版本号。"),
    ("D", "状态卡住不自愈",
     "后台路径的写失败只打日志；Waiting 不要求会话还活着；补扫失败直接跳过。",
     "后台写失败进入可见的降级态并重试；派生 Waiting 时要求进程还在；补扫失败不推进检查点。"),
    ("E", "宿主并发与资源",
     "所有 I/O 都在一把全局锁下做；终端结束后不回收。",
     "锁内只改内存，fsync / spawn / ps 挪出锁；终端结束后移出表并释放 fd。"),
    ("F", "宿主交接", "ACK 之后没有回退。", "新宿主先完成能失败的步骤（拿锁、开库、恢复 VT）再发 ACK。"),
    ("G", "信任边界", "同用户即全权；插件默认启用；所有手机共用一个机器码。", "本机进程（含 Agent）可信（11-02 定）；只加固外来输入：每台手机独立 token；配对链接打开后先确认再保存。"),
    ("H", "手机连接", "请求没有超时，也没有幂等编号；视图跟着终端而不是会话。", "请求超时 + 幂等 id；会话切换时重新订阅；4401 区分「撤销」和「改角色」。"),
    ("I", "同一状态各处各算", "侧栏、看板、core 各有一套分组函数。", "已完成：c6da3508 统一到 core status::of。"),
    ("J", "文档", "旧实现的文档还在 specs 里。", "标注过时或删掉。"),
]

ISSUES = [
    ("I1", "A", "高", "我核对", "数据", "待办、结局类编辑被无关的后台提交挡掉", "Agent 在输出时新建待办或标结局，宿主内容约每秒加一次 revision，请求报「工作台已变化」。代码注释自己写了布局类当初就是因此豁免", "core/model.rs:364 checks_revision；keeplined/host/client.rs:362"),
    ("I2", "A", "中", "我核对", "App", "忙时点的非布局编辑被静默丢掉", "前一个请求还没回来时点标记完成，什么都没发生，也没提示", "app/host/store.rs:101 hold"),
    ("I3", "A", "中", "我核对", "App", "保存失败，表单内容全丢", "存待办撞上 I1，表单已被取走，出错只写一条 error", "app/workbench/forms.rs:333；workbench/mod.rs:629"),
    ("I4", "A", "高", "子代理追踪", "App", "忙时「保存并启动」开出指向不存在待办的终端", "AddTask 被丢，但 start_after_save 还在，下一个回包时拿着不存在的 task id 启动", "forms.rs save_form；workbench/mod.rs:623"),
    ("I5", "A", "中", "子代理追踪", "App", "拖卡片、标记完成的成功提示不和结果对账", "编辑被丢或冲突，「已验收」提示和盖章动画照放，窗格也不关", "board_drawer.rs:205；complete.rs:255"),
    ("I6", "A", "中", "子代理追踪", "App", "忙时 ⌘T、通知恢复、运行 X 被吞", "launch_in_slot 忙时直接 return", "workbench/mod.rs:900"),
    ("I7", "A", "低", "推断", "数据", "超时后重点会建两份待办", "10s 超时但宿主已提交，用户再点生成第二个 id", "sessions.rs SaveSessionTask"),
    ("I8", "B", "高", "我核对", "安全", "审批只核对问句，相同问句的另一条命令会被批", "手机看到审批 A，网络延迟期间 Agent 换成同样是「Do you want to proceed?」的审批 B，点允许就批了 B", "keeplined/screen_rules.rs:312；agents/screen/claude.json anchor；host/client.rs:861"),
    ("I9", "B", "高", "我核对", "手机", "手机「选 1/2/3」「Esc」不校验画面", "电脑上已答完，手机还显示旧提问，点「选 1」把 1 打进 Agent 输入框", "mobile chat.tsx:242；keeplined/remote.rs:314"),
    ("I10", "C", "中", "我核对", "数据", "PutTab 整块替换冲掉并发分屏，进程成孤儿", "Agent 用 keeplined split 刚分屏，你同时改标签名，新窗格从布局消失但进程还在跑", "core/model.rs:862 *old = tab"),
    ("I11", "C", "中低", "子代理追踪", "数据", "关标签会停掉确认时没看到的终端", "确认框按本地快照列 Agent，宿主按当前状态停", "host/client.rs:399"),
    ("I12", "D", "高", "我核对", "故障", "Agent 退出后卡片一直「等你回答」", "Ctrl-C 掉正在问审批的 claude，看板「N 等你」和手机推送挂 3 天", "core/content.rs:448（Waiting 不看 current，:433 Running 看）"),
    ("I13", "D", "高", "我核对", "故障", "终端退出时写库失败，永远显示在跑", "日志批量写占锁超过 5s 撞上 SQLITE_BUSY，record_exit 失败只打日志，之后没人重算", "keeplined/host/live.rs:244,254"),
    ("I14", "D", "中", "子代理追踪", "故障", "补扫失败把整批事件丢掉，检查点越过", "丢的是回合最后一次写入时，卡片停在旧状态直到文件再被写", "host/association.rs:493"),
    ("I15", "D", "中", "子代理追踪", "故障", "一个读不了的日志让识别订阅无限重试", "sudo claude 留下 root 0600 文件，所有 CLI 的识别停在「识别中断」", "log_index.rs:653；association.rs:545"),
    ("I16", "D", "低中", "子代理追踪", "故障", "定时线程一次 commit 失败就永久退出", "之后到期、过时只靠渲染现算，hooks 和通知晚到", "host/mod.rs 定时线程 run()?"),
    ("I17", "D", "低", "推断", "故障", "Draining 没有超时，App 也不展示", "shell 退出但 disown 的后台进程占着 tty", "core/model.rs:259"),
    ("I18", "E", "中", "我核对", "并发", "已结束的终端从不移出内存", "宿主跑得越久，PTY fd 和滚动缓冲越多", "keeplined 全局无 terminals.remove / retain"),
    ("I19", "E", "中", "我核对一半", "并发", "全局锁下 fsync / fork / ps / 等日志库，拖停所有 PTY 读", "日志批量写占 gate 时来一个 Hook 请求，所有 pump 排队，Agent 输出卡住（卡顿后果待压测）", "store.rs:64 synchronous=FULL；host/client.rs:291 commit"),
    ("I20", "E", "中", "子代理追踪", "并发", "交接时持全部锁做网络 I/O，冻结 ≥ 20s", "新宿主读得慢时整个守护进程停住", "host/client.rs:102-145"),
    ("I21", "E", "中", "推断", "并发", "锁中毒后变成不退出的僵尸宿主", "任何线程持 Host 锁 panic 一次，之后所有请求失败但进程还在", "host/mod.rs:90 lock()"),
    ("I22", "E", "中低", "子代理追踪", "并发", "粘贴和回车之间 150ms 不持锁", "这期间窗格里的按键夹进来，回车把合在一起的内容提交", "host/client.rs:755"),
    ("I23", "E", "低", "子代理追踪", "并发", "交接接手的终端泄漏线程和 kqueue", "每接手一个终端泄漏一个", "host/live.rs:265"),
    ("I24", "F", "高", "我核对", "故障", "ACK 后新宿主失败挂断所有终端", "拿锁超时、开库失败、某个 VT 恢复失败都会挂断全部；App 提示仍说旧后台照常", "host/mod.rs:687 之后；lifecycle.rs:202"),
    ("I25", "G", "中", "我核对", "安全", "所有手机共用机器码，被顶替后永久下线", "拿过码的人在中继无 key 时注册同一 token，本机收 4000 进 Failed，不改设置不恢复", "keepline-relay/server.rs:257；relay.rs:632"),
    ("I26", "G", "中", "我核对", "安全", "配对链接打开就保存，不先确认", "任意网页打开 keepline://pair 链接即写入", "mobile app/pair.tsx:43"),
    ("I27", "G", "中", "子代理追踪", "安全", "插件不确认就加载执行", "能写插件目录的进程（包括 Agent）放一个目录就能每 10s 执行命令", "core/plugin.rs:194；plugin_hooks.rs:160"),
    ("I28", "G", "中低", "我核对", "安全", "动作模板的引号在双引号里失效，测试把它固定了", "\"看看 {task}\" 里 task 带 $(…) 会被执行", "core/action.rs:231 测试"),
    ("I29", "G", "中低", "推断", "安全", "pane-facts 在 OSC 7 给的目录里自动跑 git status", "进入恶意仓库时 core.fsmonitor 被执行", "actions.rs:12；terminal.rs:347"),
    ("I30", "G", "中低", "子代理追踪", "安全", "配对握手前的会话没有超时，占满 64 个槽", "拿着 token 的人开 64 个不握手的连接挤掉正常手机", "relay.rs:38,595"),
    ("I31", "G", "低", "子代理追踪", "安全", "同用户即全权：PTY 里的 Agent 能生成完全控制的配对链接", "受沙箱限制但能连 socket 的 Agent 借此提权", "host/client.rs:592"),
    ("I32", "H", "中", "子代理追踪", "手机", "断线后重发没有幂等 id，消息发两次", "宿主已执行，回复前断线，手机说没发出去，用户再发", "mobile client.ts:164；chat.tsx:147"),
    ("I33", "H", "中", "子代理追踪", "手机", "终端里换了会话，手机还看旧会话", "/new 之后新对话永远不出现", "chat.tsx:101"),
    ("I34", "H", "中", "子代理追踪", "手机", "重连竞态删掉活着的连接", "回前台和手动重连同时触发，旧连接的 onEnded 把新连接标 offline", "hub.ts:78,93"),
    ("I35", "H", "中", "子代理追踪", "手机", "改角色或踢出用 4401，手机提示重新扫码", "其实点重连就能连回，提示误导，踢也挡不住", "relay.rs:465；hub.ts:97"),
    ("I36", "H", "中", "推断", "手机", "请求没有超时，半开连接卡在 online", "换网络后所有请求挂住", "client.ts:85"),
    ("I37", "H", "中", "推断", "手机", "events 流结束后不重订", "状态还是 online，但卡片和审批不再更新", "hub.ts:111"),
    ("I38", "I", "中", "子代理追踪", "App", "侧栏、看板、core 对同一会话给出不同分组", "见第 24 节对照表", "board.rs:91；session_state.rs:286,312；content.rs:461"),
    ("I39", "I", "低中", "子代理追踪", "App", "菜单栏另存一份 Workspace，每次推送都全量拉", "两份副本可能不一致", "desktop.rs:325"),
    ("I40", "I", "中", "推断", "App", "窗格首次连接失败不重试；抢过写权的窗格重连后掉回只读", "", "terminal/pane.rs:529,542"),
    ("I41", "J", "低", "子代理追踪", "文档", "specs/restart-reconciliation.md 写的是旧 TS 版", "拿它当 Rust 版的契约会误导", "specs/restart-reconciliation.md"),
]

SEV = {"高": "err", "中": "wait", "中低": "wait", "低中": "muted", "低": "muted"}
VER = {"我核对": "done", "我核对一半": "done", "子代理追踪": "run", "推断": "muted"}


DECIDED = {
    "I12": "改：Agent 不在了就落到「中断」（11-02 定）。统一后的判定仍有这个问题：status.rs:156 和 :202 都先看 Waiting",
    "I27": "不修：本机进程（含 Agent）可信（11-02 定）",
    "I31": "不修：本机进程（含 Agent）可信，同用户即全权是有意的（11-02 定）",
    "I38": "已解决：c6da3508 已统一到 core status::of；本条是读了统一前的代码",
    "I39": "降为低：判定已统一，只剩两份数据可能差一拍",
}


def issues_page():
    rows = []
    for fid, name, cause, fix in FAMILIES:
        items = [i for i in ISSUES if i[1] == fid]
        rows.append(f'<h2 id="f{fid}">{fid} · {esc(name)} <span class="cnt">{len(items)} 条</span></h2>'
                    f'<p class="note"><b>根因：</b>{esc(cause)}<br><b>修法方向：</b>{esc(fix)}</p>')
        body = "".join(
            f'<tr><td><b>{i[0]}</b></td><td><span class="chip {SEV[i[2]]}">{i[2]}</span></td>'
            f'<td><span class="chip {VER[i[3]]}">{i[3]}</span></td><td>{esc(i[4])}</td>'
            f'<td><b>{esc(i[5])}</b><div class="sc">{esc(i[6])}</div><code>{esc(i[7])}</code></td><td class="dec">{esc(DECIDED.get(i[0], "待定"))}</td></tr>'
            for i in items)
        rows.append('<div class="tbl"><table><tr><th>编号</th><th>严重</th><th>核对</th><th>层</th><th>问题 · 场景 · 证据</th><th>决定</th></tr>'
                    + body + "</table></div>")
    cnt = {s: sum(1 for i in ISSUES if i[2] == s) for s in ["高", "中", "中低", "低中", "低"]}
    ver = {v: sum(1 for i in ISSUES if i[3] == v) for v in VER}
    nav = "".join(f'<a href="#f{f[0]}">{f[0]} {esc(f[1])}</a>' for f in FAMILIES)
    return f"""<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Keepline next 架构问题清单</title><style>{CSS}
.cnt{{font-size:13px;color:var(--muted);font-weight:400}}.sc{{color:var(--muted);font-size:13px;margin:2px 0 4px}}.dec{{color:var(--muted);white-space:nowrap}}
.tbl td:nth-child(5){{min-width:420px}}</style></head>
<body><main>
<button class="toggle" onclick="var r=document.documentElement;r.dataset.theme=(r.dataset.theme==='dark'||(!r.dataset.theme&&matchMedia('(prefers-color-scheme: dark)').matches))?'light':'dark'">明 / 暗</button>
<h1>架构问题清单</h1>
<p class="lead">2026-11-02 按六层（数据一致性、宿主并发、故障恢复、信任边界、App 内部状态、手机端）只读审计 <code>next/</code> 得到。共 {len(ISSUES)} 条，按根因归成 {len(FAMILIES)} 族；同一族用同一种修法，比逐条修划算。状态机和结构图见 <a href="next.html">架构页</a>；每族该怎么设计见 <a href="design.html">设计稿</a>。</p>
<p class="note">严重度：高 {cnt['高']} · 中 {cnt['中']} · 中低 / 低中 {cnt['中低'] + cnt['低中']} · 低 {cnt['低']}。核对程度：<span class="chip done">我核对</span> 我亲自回代码看过分支 {ver['我核对'] + ver['我核对一半']} 条；<span class="chip run">子代理追踪</span> 子代理读代码追过调用链、我没逐条复核 {ver['子代理追踪']} 条；<span class="chip muted">推断</span> 依赖运行时条件、需要复现 {ver['推断']} 条。全部没有实际运行复现。行号是 2026-11-02 的工作区，别的会话在改，可能已偏移。</p>
<p class="note"><b>建议先做的三族：</b>A（每天都会碰到：Agent 在输出时存待办、标完成会失败或丢东西）、B（安全：审批可能批到另一条命令）、D（状态卡住不自愈，看板和推送会说谎）。E、G、H 次之；C、I、J 可以随相关改动顺手做。</p>
<nav>{nav}</nav>
{''.join(rows)}
</main></body></html>
"""


# ───────── 设计稿：先定设计，再改 ─────────

PRINCIPLES = [
    ("P1", "每个请求都有自己的状态机", "挂起 → 已确认 / 被拒（原因）/ 结果未知（超时）。界面以回执为准：成功提示等确认，失败时把输入还给你，结果未知时明说「不确定有没有生效」。", "A H B"),
    ("P2", "写入只带它在意的前提", "不拿全局版本号当锁。新建不需要前提；改结局带「我看到的是 Open」；改对象带这个对象自己的版本。宿主上一次无关的提交不该挡住你。", "A C"),
    ("P3", "派生状态只有一个来源，且要算上「观测源还活着吗」", "「等你回答」「在跑」这类需要你注意的状态，必须有活着的进程或终端支撑；观测源没了就降级。所有界面调同一个函数，只做展示映射。", "D I"),
    ("P4", "锁内只改内存；会失败的步骤放在「承诺」之前", "fsync、fork、跑外部命令、网络 I/O 不进全局锁；交接时先把能失败的都做完，再发 ACK。", "E F"),
    ("P5", "宿主不能随便退出", "宿主持有所有 PTY，宿主退出等于挂断全部终端。所以出错时的默认策略是「降级 + 显示出来 + 重试」，不是「崩溃让人重启」。这条约束决定了 D、E 的很多选择。", "D E F"),
    ("P6", "外来的信任要显式授予", "本机同一用户的进程（含 Agent）可信；外来的配对链接、新设备第一次都要你确认；每台手机的凭据彼此独立，撤销一台不影响别的。", "G H"),
]

DESIGNS = [
    {"id": "A", "name": "编辑的前提与请求回执", "issues": "I1–I7",
     "now": "9 种编辑查全局 revision：AddSpace、UpdateSpace、AddTask、UpdateTask、SetTaskOutcome、SetPendingTask、SetOutcome、AssignTask、SaveSessionTask。宿主每次提交（包括会话内容约每秒一次）都加 revision。客户端只有一个「忙」位，忙时非布局编辑直接丢；表单发出前就取走，失败不放回。",
     "inv": ["一个编辑只会因为它要改的那个对象被别人改过而被拒", "你的每次操作要么生效，要么明确失败且输入还在；不存在静默消失", "成功提示只在宿主确认后出现"],
     "opts": [("A1 对象版本", "Task、Space、AgentSession 各带 rev；Update 类编辑带期望 rev", "模型和存储加字段；每处写入都要记得加 rev"),
              ("A2 前提条件（推荐）", "每种编辑自带它在意的前提：Add 类不带；SetOutcome 带「我看到的结局」；UpdateTask 拆成字段级修改（改标题、改标准、挂证据）", "要逐个编辑设计前提；UpdateTask 要拆；但和 C 族用同一套机制"),
              ("A3 过渡：拆计数器", "会话内容另起一个计数，不再加 Workspace revision", "改动最小，立刻消掉 I1 的大部分；但两个用户操作之间仍是粗锁")],
     "client": "客户端改成请求队列：每个请求一个 id 和状态（挂起 / 确认 / 被拒 / 未知），不再有丢弃；表单在确认后才关；乐观提示要么等确认，要么失败时撤回。宿主记住最近处理过的请求 id，重发同一个 id 直接返回上次结果（同时解决 I7 和手机的 I32）。",
     "ask": ["成功提示等确认（本机 socket 通常几十毫秒，但碰上 fsync 会慢）还是乐观显示、失败撤回？", "结果未知（超时）时界面怎么说、给不给「重试」按钮？", "A3 先做过渡，还是直接上 A2？"],
     "check": "用例：Agent 持续输出时连续标记完成 100 次，0 次「工作台已变化」；忙时的每次点击都生效或明确报错；保存失败后表单内容还在。"},
    {"id": "B", "name": "审批与按键的实例身份", "issues": "I8–I9",
     "now": "审批的身份只是画面上那一行问句（如 \"Do you want to proceed?\"），回答时宿主只核对这一行还在。手机的「选 1/2/3」「Esc」直接写进终端，不核对画面。",
     "inv": ["你批准的必须是你看到的那一个审批", "任何代替你按键的远程操作，都要指明它针对的是哪个画面"],
     "opts": [("B1 证据指纹（推荐）", "审批 id = 问句 + 证据行（命令、文件名）的指纹；卡片带上证据行给手机展示；回答带 id，宿主重读画面重算，不符就拒", "画面规则要能稳定截出证据行；重绘、折行可能让指纹抖动，需要规范化"),
              ("B2 用 waiting_episode", "每次进入 Waiting 加 1，回答带 episode", "前提是连续两个审批之间一定经过 Running；这一点没核实，可能不成立"),
              ("B3 按键也带画面身份", "没认出提问时手机不给数字按钮，只给「打开画面」；有提问时按键带它的 id", "手机少了一点「盲按」的灵活性")],
     "client": "", "ask": ["手机还要不要保留不带校验的原始按键（选 1/2/3、Esc）？", "手机上审批必须展示命令全文吗（可能很长）？"],
     "check": "构造两个问句相同、命令不同的审批先后出现，用旧卡片点允许 → 被拒；画面已经变了时按「选 1」→ 被拒。先核实 B2 的前提。"},
    {"id": "C", "name": "布局编辑改成操作", "issues": "I10–I11",
     "now": "PutTab 用客户端快照整块覆盖标签，并且豁免版本检查。改名、拖分隔线、移动窗格都走它，所以会冲掉并发的分屏。关标签时，确认框按本地快照列终端，宿主按当前状态停。",
     "inv": ["一个布局操作只改它指名的那部分，并发的其他布局操作不丢", "你确认要停的终端，就是宿主实际停的终端"],
     "opts": [("C1 操作型编辑（推荐）", "RenameTab、ResizeSplit、MovePane、AddPane……宿主在当前布局上应用", "编辑种类变多；客户端的布局代码要从「改完整树再发」改成「发操作」"),
              ("C2 每个标签一个版本", "PutTab 带期望版本，冲突时客户端基于新布局重做", "改动小；拖分隔线这种高频操作冲突后要重做"),
              ("C3 宿主合并", "PutTab 时宿主把客户端不知道的窗格补回去", "规则难说清，容易出新的边界情况")],
     "client": "关标签：确认请求带上「我确认停这些终端」的列表，宿主发现多出来的就拒绝，让你重新确认。",
     "ask": ["拖分隔线这种每秒几十次的操作，走 C1 还是 C2？"], "check": "Agent 执行 keeplined split 的同时改标签名，两个改动都保留。"},
    {"id": "D", "name": "不自愈的状态：观测源与宿主健康", "issues": "I12–I17",
     "now": "「等你回答」不要求会话还活着；后台写库失败只打日志；补扫失败直接跳过、检查点照常前进；定时线程出错一次就永久退出。",
     "inv": ["任何「需要你」的状态都必须有活着的观测源支撑，观测源没了就降级成「停了」或「中断」", "宿主内存和外显状态的偏差一定会被看见，并且会被重试修正", "检查点只在事件被成功处理之后前进"],
     "opts": [("派生规则", "Waiting 要求 Agent 还活着（session.current 且终端在跑）；否则落到「中断」。改在 core status::of 里（live() 和 recorded() 两条路径都要改）", "已定：显示「中断」"),
              ("宿主健康状态机", "Healthy → Degraded（原因、重试中）→ Healthy。后台写失败进 Degraded，失败的写入（如 record_exit）进重试队列；App 显示一条提示", "要新增一个推给 App 的状态和一条提示"),
              ("长期线程监督", "定时、订阅这类线程出错后由监督者重启，并进 Degraded；不让宿主退出（见 P5）", "要给每个长期线程定重启策略")],
     "client": "", "dec": "已定（11-02）：Agent 在等审批时被杀，显示「中断」。", "ask": ["Degraded 的提示放在哪里（状态栏？顶部条？）"],
     "check": "故障注入：让 record_exit 第一次写库返回 SQLITE_BUSY，终端状态最终仍变成 Exited；Ctrl-C 掉等审批的 Agent，卡片 1 分钟内离开「等你回答」。"},
    {"id": "E", "name": "锁与 I/O 分离、终端回收", "issues": "I18–I23",
     "now": "一把全局 Host 锁下做 fsync（synchronous=FULL）、fork、/bin/ps、读对话记录、等日志库锁、交接时写 socket。PTY pump 的副作用也要拿这把锁。结束的终端从不移出内存。",
     "inv": ["锁内只改内存、发通知，不做可能阻塞超过毫秒级的 I/O", "读 PTY 不依赖全局锁", "终端结束后，它的 fd 和缓冲会被释放"],
     "opts": [("E2 先降 fsync 级别", "WAL 下改成 synchronous=NORMAL：库不会坏，崩溃时最多丢最后一次提交", "一行改动；要接受崩溃丢最后一次编辑"),
              ("E3 pump 不直接拿 Host 锁", "标题、命令完成这类副作用改成发消息给一个处理线程", "多一个队列；顺序要保证"),
              ("E1 单写线程", "锁内生成新状态和差异，交给写线程落库", "失去「落库成功才换内存」，落库失败要走 D 的 Degraded；改动最大")],
     "client": "", "ask": ["能否接受宿主崩溃时丢最后一次编辑（E2）？", "要先压测确认 I19 真的会卡（目前是推断）再动 E1/E3 吗？"],
     "check": "压测：20 个终端持续输出 + 日志批量写入 + 频繁 Hook，测 PTY 读的最大停顿；改前改后对比。开关 200 个终端后，fd 数回到基线。"},
    {"id": "F", "name": "交接两阶段提交", "issues": "I24",
     "now": "新宿主收到 fd 就发 ACK，旧宿主收到即退出；之后的拿锁、开库、恢复 VT 任何一步失败，全部终端被挂断。",
     "inv": ["旧宿主退出之前，新宿主已经完成所有会失败的步骤"],
     "opts": [("两阶段（推荐）", "PREPARE：新宿主收 fd、开库、恢复 VT，都成功才发 READY；COMMIT：旧宿主释放 host.lock 并退出，新宿主拿锁开始服务", "要处理「两个进程同时开着库」（WAL 允许，但写入要只有一方）；拿锁仍在 COMMIT 后，要保证旧宿主退出够快")],
     "client": "", "ask": ["COMMIT 后新宿主仍然失败（极少见）时，接受挂断，还是让旧宿主先不退出、多等一个确认？"],
     "check": "在 PREPARE 的每一步注入失败，旧宿主都继续服务、终端不断。"},
    {"id": "G", "name": "本机信任模型", "issues": "I25–I31",
     "now": "同一用户的任何进程（包括终端里的 Agent）都有全部权限，能生成完全控制的配对链接；插件默认启用、直接执行命令；所有手机共用一个机器码；配对链接打开就保存。",
     "inv": ["外来的配对链接、新设备第一次都要你确认", "撤销一台手机只影响这一台，别人顶替也只影响那一台"],
     "opts": [("每台手机独立路由 token", "撤销即失效；中继上的顶替只影响一台", "配对和中继注册流程要改"),
              ("零散加固", "配对链接先确认再保存；握手前的会话 10s 超时；动作模板不在双引号里展开；不可信目录里的 git 关掉 fsmonitor", "各自都小")],
     "client": "", "ask": [], "dec": "已定（11-02）：本机同一用户的进程（包括终端里的 Agent）可信，I27、I31 不修。不可信的只有外来输入：网页打开的配对链接（I26）、中继和手机（I25、I30）、仓库内容（I29）、拼进命令的任务文字（I28）。这一族只做这些的加固，插件首次确认去掉。",
     "check": "撤销手机 A 后，A 的凭据无法让本机掉线；从网页打开配对链接，不确认就不保存。"},
    {"id": "H", "name": "手机请求与订阅", "issues": "I32–I37",
     "now": "请求没有超时、没有幂等 id；会话视图按终端取会话，换会话后不跟；4401 同时表示撤销、改角色、踢出；events 流结束后不重订；重连有竞态。",
     "inv": ["手机的每个请求都会在有限时间内得到结果（成功 / 失败 / 未知）", "重发不会让同一件事做两次", "视图跟着会话走，不跟着终端走"],
     "opts": [("复用 A 的请求状态机", "超时 + 幂等 id，宿主按（设备，请求 id）去重", "宿主要存最近的请求结果"),
              ("关闭码拆开", "4401 撤销（要重新配对）、改角色（重连即可）、踢出（冷却一段时间）各用一个码", "手机和宿主一起改"),
              ("心跳 + 重订", "心跳检测半开连接；events 结束自动重订；会话切换推一条事件", "多一点流量")],
     "client": "", "ask": ["踢出之后要不要冷却时间，比如 5 分钟内拒绝重连？"],
     "check": "发送时断网再恢复，消息只出现一次；终端里 /new 之后，手机 5 秒内切到新会话。"},
    {"id": "I", "name": "唯一的状态派生", "issues": "I38–I40",
     "now": "已统一（c6da3508，11-02 21:48）：状态只在 core status::of 判定一次，侧栏、看板、菜单栏、手机都调它。问题清单的 I38 是读了统一前的代码。剩下的只有菜单栏另存一份 Workspace（I39，差一拍）和窗格重连（I40，与分组无关）。",
     "inv": ["同一个会话，在任何界面上只有一个分组结果"],
     "opts": [("core 里一个 status 函数（推荐）", "输入：结局、回合、终端状态、活跃程度、后台 shell、是否看过；输出一个 Status。侧栏、看板、菜单栏、通知只做展示映射", "要先把分歧处的口径定下来；三处调用都要改")],
     "client": "菜单栏和工作台在同一进程里，共用一份 Workspace。",
     "ask": [], "dec": "已解决：分组已统一，原来三个口径问题由 status::of 回答（活着出错 → 出错了；只剩后台 shell → 等后台）。只剩 D 族「被杀 → 中断」要改进这同一个函数。",
     "check": "对一组固定的会话快照，侧栏、看板、菜单栏的分组结果完全一致（可做成单测）。"},
]

ORDER = [
    ("第 0 步 · 先定产品口径（11-02 已定）", "分组已统一（c6da3508）；Agent 可信；被杀时的审批显示「中断」。"),
    ("第 1 步 · 先验证推断", "I19（锁下 fsync 会不会真卡）压测；B2 的前提（两个审批之间是否一定经过 Running）。推断不成立的问题就降级或删掉。"),
    ("第 2 步 · A + C 一起设计", "同一套「编辑带前提」机制和客户端请求状态机；H 族直接复用。"),
    ("第 3 步 · B 和 F 可以并行", "都比较独立。"),
    ("第 4 步 · D 再 E", "E1 依赖 D 的 Degraded 状态；E2 可以随时先做。"),
]


def design_page():
    pr = "".join(f'<tr><td><b>{a}</b></td><td><b>{esc(b)}</b><div class="sc">{esc(c)}</div></td><td>{esc(d)}</td></tr>' for a, b, c, d in PRINCIPLES)
    cards = []
    for d in DESIGNS:
        opts = "".join(f'<tr><td><b>{esc(a)}</b></td><td>{esc(b)}</td><td>{esc(c)}</td></tr>' for a, b, c in d["opts"])
        inv = "".join(f"<li>{esc(x)}</li>" for x in d["inv"])
        ask = "".join(f"<li>{esc(x)}</li>" for x in d["ask"])
        client = f'<p><b>配套：</b>{esc(d["client"])}</p>' if d["client"] else ""
        cards.append(f'''<h2 id="d{d["id"]}">{d["id"]} · {esc(d["name"])} <span class="cnt">问题 {d["issues"]}</span></h2>
<div class="card"><p><b>现状：</b>{esc(d["now"])}</p>
<p><b>要守住的规则：</b></p><ul>{inv}</ul>
<div class="tbl"><table><tr><th>方案</th><th>做法</th><th>代价</th></tr>{opts}</table></div>
{client}{f'<p><b>需要你定：</b></p><ul class="ask">{ask}</ul>' if ask else ""}
<p><b>怎么验证设计对了：</b>{esc(d["check"])}</p>
<p class="dec">决定：{esc(d.get("dec", "待定"))}</p></div>''')
    order = "".join(f"<li><b>{esc(a)}</b>：{esc(b)}</li>" for a, b in ORDER)
    nav = "".join(f'<a href="#d{d["id"]}">{d["id"]} {esc(d["name"])}</a>' for d in DESIGNS)
    asks = sum(len(d["ask"]) for d in DESIGNS)
    return f"""<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Keepline next 设计稿</title><style>{CSS}
.cnt{{font-size:13px;color:var(--muted);font-weight:400}}.sc{{color:var(--muted);font-size:13px;margin:2px 0}}
.card{{background:var(--card);border:1px solid var(--line);border-radius:16px;padding:6px 20px 12px}}
.card .tbl{{border:none;padding:0;background:none}}ul{{margin:4px 0 10px;padding-left:22px}}ul.ask li{{color:var(--user)}}
.dec{{color:var(--muted);border-top:1px dashed var(--line);padding-top:8px}}</style></head>
<body><main>
<button class="toggle" onclick="var r=document.documentElement;r.dataset.theme=(r.dataset.theme==='dark'||(!r.dataset.theme&&matchMedia('(prefers-color-scheme: dark)').matches))?'light':'dark'">明 / 暗</button>
<h1>设计稿：先定设计，再改代码</h1>
<p class="lead">这一页回答「<a href="issues.html">问题清单</a>里的问题，系统应该怎么设计才不会再出」。<b>本页还没有任何代码改动。</b>它要支撑的决定是：每一族选定一个目标设计（采用 / 否定 / 待验证），全部定下来之后才开始改。放弃条件：某一族的设计成本远高于修补，而它的问题都只是「推断」级，就先复现，复现不了就不做。</p>
<p class="note">共 6 条设计原则、9 族设计、{asks} 个需要你定的问题（紫色）。状态机现状见 <a href="next.html">架构页</a>。</p>

<h2>跨族的设计原则</h2>
<p class="note">41 个问题背后其实只有几条原则没守住。先定原则，各族的设计就是它们的展开。</p>
<div class="tbl"><table><tr><th></th><th>原则</th><th>管哪几族</th></tr>{pr}</table></div>

<h2>建议的推进顺序</h2>
<ol class="flows">{order}</ol>
<nav>{nav}</nav>
{''.join(cards)}
<h2>J · 文档</h2>
<div class="card"><p><code>specs/restart-reconciliation.md</code> 写的是旧 TS 版，给它标上「已过时，next 见 architecture/next.html 第 22 节」。不需要设计。</p></div>
</main></body></html>
"""

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
.tbl{overflow-x:auto;background:var(--card);border:1px solid var(--line);border-radius:16px;padding:6px 14px;margin:12px 0}
.tbl table td:first-child{white-space:nowrap}.yes{color:var(--done)}.no{color:var(--muted)}
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
<nav><a href="#sys">系统总图</a><a href="#pipe">信号 → 状态</a><a href="#layers">状态分层</a><a href="#proc">终端进程</a><a href="#turn">回合</a><a href="#outcome">结局</a><a href="#inbox">分组</a><a href="#bind">绑定</a><a href="#recog">识别健康</a><a href="#remote">远程</a><a href="#handoff">宿主交接</a><a href="#task">待办</a><a href="#notify">通知</a><a href="#approval">审批</a><a href="#conn">窗格连接</a><a href="#hostlife">宿主生命周期</a><a href="#layout">布局</a><a href="#agents">各家信号</a><a href="#plugins">插件</a><a href="#edit">编辑生命周期</a><a href="#locks">线程与锁</a><a href="#failures">故障恢复</a><a href="#trust">信任边界</a><a href="#appstate">App 内部状态</a><a href="#phone">手机端</a><a href="#code">代码索引</a><a href="issues.html"><b>问题清单</b></a><a href="design.html"><b>设计稿</b></a></nav>

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
<p class="note">核心状态机。每次变化由 <code>settle()</code> 记下是哪路信号定的（日志 / Hook / 画面 / 官方状态 / 子 Agent），进入 Waiting 时 <code>waiting_episode += 1</code>，同一个对话框被多路信号报到，每条通知路只发一次（见第 13 节）。任何状态收到新的用户发话都回到 Running。</p>
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
<p class="note">app 发现正在跑的宿主和自己带的不是同一份代码（digest 不同）只会提示；你点重启后，旧宿主支持交接才由新宿主接管（见第 16 节）。交接成功时终端里的 shell 和 Agent 不会察觉。分界在 ACK：ACK 前失败旧宿主继续服务，ACK 后旧宿主已退出，新宿主再失败就没有回退。</p>
<div class="fig">{fig_handoff()}</div>

<h2 id="task">12 · 待办与验收证据</h2>
<p class="note">待办状态先看你定的结局（<code>task_outcomes</code>），没定结局时按证据派生。证据只来自会话卡片里能挂的记录（测试、提交、改文件），Agent 说的话永远不算；挂上去的是一份拷贝，会话往后滚也不会丢。</p>
<div class="fig">{fig_task()}{LEGEND}</div>
<h3>每条验收标准</h3>
<div class="fig">{fig_criterion()}</div>
<p class="note">新会话挂上哪个待办：第一次绑定到终端时，先用 <code>pending_tasks[终端]</code>（用一次就删掉；显式设为空则表示「这次不挂」），没有就用标签的 <code>default_task</code>。之后换终端、resume 都不再改。</p>

<h2 id="notify">13 · 通知链路</h2>
<p class="note">两条独立的路，都从 Workspace 的变化出发、都只看 Keepline 终端里的会话，<b>各自去重、互不知情</b>：同一个对话框会弹一次系统通知，也会跑一次 hooks.json。每条路内部去重的单位是「一个回合的一个对话框」：日志、Hook、画面先后报到，<code>waiting_episode</code> 不变，这条路只发一次。hooks.json 同时在跑的满 8 个时，新触发直接丢弃，不排队、不补发。</p>
<div class="fig">{fig_notify()}</div>

<h2 id="approval">14 · 审批应答</h2>
<p class="note">只有画面规则认出来的审批框能远程回答，回答前宿主重读画面核对问句。<b>但核对的「问句」只是那一行</b>（如 "Do you want to proceed?"），不含命令，前后两个问句相同的审批分不出来（问题 I8）。</p>
<div class="fig">{fig_approval()}</div>

<h2 id="conn">15 · 窗格连接</h2>
<p class="note">每个窗格一条到宿主的流：先收 VT 快照，再收增量。一个终端同一时刻只有一个 Attach 流能写（owner），其余只读。<b>这把锁只管窗格的 Attach 流</b>：手机的发送文本、按键、回答审批走独立 RPC（<code>SendText</code> / <code>AnswerApproval</code>），不看 owner，只受手机角色限制，所以手机和正在写的窗格可以同时往一个终端输入。</p>
<div class="fig">{fig_conn()}{LEGEND}</div>

<h2 id="hostlife">16 · 宿主生命周期</h2>
<p class="note">App 启动时连 <code>workbench.sock</code>；对比宿主报的 build digest 和自己带的 keeplined，不同就提示，由你决定何时重启。交接细节见第 11 节。</p>
<div class="fig">{fig_hostlife()}{LEGEND}</div>

<h2 id="layout">17 · 布局与终端</h2>
<p class="note">摆放只归 <code>Tab.layout</code> 管，终端身份不随窗格消失；但关窗格、关标签、删空间都会让宿主停掉对应终端。布局类改动按 id 寻址，不要求基于宿主最新 revision。</p>
{LAYOUT}

<h2 id="agents">18 · 各家 Agent 能给的信号</h2>
<p class="note">宿主其余部分与 CLI 无关，差异都收在 <code>keeplined/src/agents/</code> 的 Adapter 里。信号越少的 CLI 越依赖画面规则和日志，「等你回答」也越可能晚一点。</p>
{AGENTS}
<p class="note"><b>额度怎么来、怎么过期</b>（<code>keepline-core/src/quota.rs</code>）：Keepline 从不向供应商请求额度，只读各 CLI 在本机留下的记录：Codex 写在对话日志里，Claude Code 经 Keepline 包装的 statusline 记下，Grok 在启动日志里。Cursor、Qoder 本地不记额度（Qoder 只记每次回复花的点数）。展示时按 Agent 类型取最新一条读数，超过 7 天（<code>SHOWN_FOR</code>）的不显示；读数里某个窗口的重置时间已过（<code>Window::reset</code>），旧百分比就不再算数，要等下次用这个 CLI 才会更新。所以额度永远只和最后一条记录一样新，每个读数都带着记录时间。</p>

<h2 id="plugins">19 · 插件的七种挂点</h2>
<p class="note">插件是一个目录，<code>plugin.json</code> 声明 <code>provides</code>，每种对应一个文件。只有左栏插件跑 wasm，其余都是声明式数据或外部命令。</p>
{PLUGINS}

<h2 id="edit">20 · 一次编辑的生命周期</h2>
<p class="note">宿主这一侧是原子的：先在副本上应用、落库成功才换内存，宿主自己不会内存和库分叉。问题都在两头：前面客户端忙时直接丢，中间用一个全局 revision 挡住所有「查版本号」的编辑，后面客户端失败时不还原输入、不撤回乐观提示。豁免版本检查的：PutTab、CloseTab、ClosePane、SplitPane、RenameSession、MoveTab、Settings、RemoveSpace。见问题族 <a href="issues.html#fA">A</a>、<a href="issues.html#fC">C</a>。</p>
<div class="fig">{fig_edit()}{LEGEND}</div>

<h2 id="locks">21 · 宿主线程与锁</h2>
<p class="note">长期线程：接收连接、每客户端一个、attach / 状态 / 事件各自的写线程、每终端的 PTY pump 和回收、定时、日志订阅、注册表监视、Vnodes、远程 Supervisor、mirror。两把锁同时持有的地方顺序一致：<b>Host → Live → replies</b>、Host → Supervisor.inner、Host → gate；没找到反方向，所以没有锁序死锁。推给订阅者都走 <code>try_send</code>，慢客户端不会拖住别人。问题在于持锁时做阻塞 I/O（问题族 <a href="issues.html#fE">E</a>）。</p>
{LOCKS}

<h2 id="failures">22 · 故障与恢复</h2>
<p class="note">宿主整体是「故障即停」：SQLite 写在事务里，前台请求失败会报给你。漏洞在后台路径：后台写失败只打日志，状态从此错下去（问题族 <a href="issues.html#fD">D</a>）。<code>specs/restart-reconciliation.md</code> 描述的是旧 TS 版，不适用于 next。</p>
{FAILURES}

<h2 id="trust">23 · 信任边界</h2>
<p class="note">手机这条路是严的：只能触发固定 7 种操作，每个请求都过角色检查。松的是本机这一侧：同一用户的任何进程都有全部权限，插件默认启用（问题族 <a href="issues.html#fG">G</a>）。</p>
{TRUST}
<h3>手机连接（手机侧 <code>hub.ts</code>，宿主侧 <code>relay.rs</code>）</h3>
<div class="fig">{fig_phone()}{LEGEND}</div>

<h2 id="appstate">24 · App 内部状态</h2>
<p class="note">App 只持有一份 <code>Workbench.workspace</code>，新 revision 来了整份替换，视图渲染时现算状态。导航（鸟瞰 / 标签 / 待办详情）、表单、拖拽、未看标记都只在内存里；偏好存在 <code>~/.config/keepline/*</code>。<code>apply_workspace</code> 不检查导航和表单指向的东西还在不在。所有请求共用一个「忙」位：</p>
<div class="fig">{fig_appreq()}</div>
<h3>同一状态各处各算（问题 I38）</h3>
{DERIVE}

<h2 id="phone">25 · 手机端</h2>
<p class="note">发消息：发出后显示「发送中」，失败撤回并把文字放回输入框；成功<b>没有确认态</b>，要等日志里出现同样文字或 30 秒超时才去掉。答审批：成功后 300ms 刷新一次列表。会话视图按终端取会话，之后每 2s 轮询。协议三层版本：配对链接 <code>v=2</code> 必须相等；JSON 协议 <code>VERSION=1</code> 不协商，靠未知字段忽略兼容；中继帧 proto≥2 走二进制。问题见族 <a href="issues.html#fH">H</a>、<a href="issues.html#fB">B</a>。</p>

<h2 id="code">26 · 代码索引</h2>
<table><tr><th>状态</th><th>定义</th><th>转移在哪</th></tr>{code_rows}</table>
<p class="note">本页由 <code>specs/architecture/gen.py</code> 生成，改图请改脚本里的数据后重跑。生成于 2026-11-02，对应 main 270f49fe。</p>
</main></body></html>
"""


if __name__ == "__main__":
    OUT.write_text(page(), encoding="utf-8")
    OUT.with_name("issues.html").write_text(issues_page(), encoding="utf-8")
    OUT.with_name("design.html").write_text(design_page(), encoding="utf-8")
    print(OUT)
