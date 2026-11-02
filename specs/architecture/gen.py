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
        {"from": "working", "to": "done", "via": [(400, 30), (990, 30)], "cls": "thin", "label": "也可直接标记完成（不强制证据）", "seg": 2, "ldy": -10},
        {"from": "working", "to": "deferred", "label": "稍后再看", "ldx": 40},
        {"from": "proven", "to": "abandoned", "cls": "thin", "label": "放弃", "ldx": 30},
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
<nav><a href="#sys">系统总图</a><a href="#pipe">信号 → 状态</a><a href="#layers">状态分层</a><a href="#proc">终端进程</a><a href="#turn">回合</a><a href="#outcome">结局</a><a href="#inbox">分组</a><a href="#bind">绑定</a><a href="#recog">识别健康</a><a href="#remote">远程</a><a href="#handoff">宿主交接</a><a href="#task">待办</a><a href="#notify">通知</a><a href="#approval">审批</a><a href="#conn">窗格连接</a><a href="#hostlife">宿主生命周期</a><a href="#layout">布局</a><a href="#agents">各家信号</a><a href="#plugins">插件</a><a href="#code">代码索引</a></nav>

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
<p class="note">只有画面规则认出来的审批框能远程回答，回答前宿主一定重读画面，防止答到别的问题上。</p>
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

<h2 id="code">20 · 代码索引</h2>
<table><tr><th>状态</th><th>定义</th><th>转移在哪</th></tr>{code_rows}</table>
<p class="note">本页由 <code>specs/architecture/gen.py</code> 生成，改图请改脚本里的数据后重跑。生成于 2026-11-02，对应 main 270f49fe。</p>
</main></body></html>
"""


if __name__ == "__main__":
    OUT.write_text(page(), encoding="utf-8")
    print(OUT)
