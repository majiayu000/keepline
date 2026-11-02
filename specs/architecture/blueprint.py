#!/usr/bin/env python3
"""生成新仓库的目标设计：python3 specs/architecture/blueprint.py → specs/architecture/blueprint.html

这份设计是给新仓库从头搭建用的，不引用旧仓库的行号；旧仓库只在附录里作对照。
图和表都是这里的数据，改设计改这里再重跑，别手改 HTML。
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import gen  # noqa: E402
from gen import Box, svg, init_dot, label_at, draw_group, text_block, esc, seq, ladder, table, LEGEND, CSS  # noqa: E402

OUT = Path(__file__).with_name("blueprint.html")


# ───────── 图 ─────────

def fig_topology():
    B, g = [], []
    g.append(draw_group(20, 50, 230, 420, "终端里的程序"))
    for i, t in enumerate(["Agent CLI\nClaude · Codex · Grok …", "shell 与普通命令", "自报状态的程序\nOSC 7501", "keepline 命令行\nlist · send · run …"]):
        B.append(Box(f"p{i}", 35, 88 + i * 70, 200, 56, t, "", "mod", center=False))
    B.append(Box("logs", 35, 380, 200, 70, "磁盘上的对话日志\nCLI Hook", "", "mod", center=False))

    g.append(draw_group(290, 50, 560, 420, "keepline serve · 宿主（每用户一个，持有全部 PTY）", "grp host"))
    B.append(Box("pty", 305, 88, 160, 120, "终端层\nPTY · VT · pump\n每终端一个 Live\n不碰全局状态", "run", "mod", center=False))
    B.append(Box("sig", 305, 228, 160, 110, "观测层\n日志索引 · Hook\n画面规则 · 进程表\n官方状态", "", "mod", center=False))
    B.append(Box("actor", 490, 88, 170, 250, "状态线程（唯一）\n\n按顺序应用：\n· 你的操作 Op\n· 观测到的信号\n· 定时到期\n\n只改内存\n产出差异", "run", "mod", center=False))
    B.append(Box("writer", 685, 88, 150, 90, "写库线程\nSQLite WAL\n确认后才回执", "", "mod", center=False))
    B.append(Box("fx", 685, 198, 150, 140, "副作用工人\nspawn · ps\nhooks.json · webhook\n通知回执", "", "mod", center=False))
    B.append(Box("health", 305, 358, 530, 40, "宿主健康：Healthy / Degraded（原因 · 重试中），推给所有客户端", "wait", "mod", center=False))
    B.append(Box("ctl", 305, 412, 255, 44, "control：本机 socket（同一用户可信）", "", "mod", center=False))
    B.append(Box("rem", 580, 412, 255, 44, "remote：设备表 · 角色 · Noise 通道", "", "mod", center=False))

    g.append(draw_group(890, 50, 270, 200, "桌面", "grp app"))
    B.append(Box("app", 905, 88, 240, 70, "keepline-app（GPUI）\n侧栏 · 看板 · 窗格 · 设置", "", "mod", center=False))
    B.append(Box("menu", 905, 170, 240, 60, "菜单栏 · 系统通知\n同进程，共用一份状态", "", "mod", center=False))
    g.append(draw_group(890, 270, 270, 200, "远程", "grp"))
    B.append(Box("relay", 905, 308, 110, 70, "中继\n只转发密文", "", "mod", center=False))
    B.append(Box("phone", 1035, 308, 110, 70, "手机\n聊天 · 审批", "", "mod", center=False))
    core = ('<g class="grp core"><rect x="20" y="490" width="1140" height="40" rx="10"/>'
            '<text class="t-mod" x="590" y="510" text-anchor="middle" dominant-baseline="central">'
            'core（纯逻辑、无 I/O）：数据模型 · Op 与前提 · 状态派生 status · 回合规则 · 协议类型　｜　remote-proto：配对 · Noise · 帧 · 关闭码</text></g>')
    E = [
        {"from": (235, 150), "to": (305, 150), "both": True, "num": "1"},
        {"from": "logs", "to": (305, 300), "num": "2"},
        {"from": "pty", "to": "actor", "cls": "thin"},
        {"from": "sig", "to": "actor", "num": "3"},
        {"from": (660, 130), "to": (685, 130), "num": "4"},
        {"from": (660, 260), "to": (685, 260), "cls": "thin"},
        {"from": (432, 412), "to": (575, 338), "both": True, "num": "5"},
        {"from": (560, 434), "to": (905, 120), "via": [(870, 434), (870, 120)], "both": True, "cls": "thin"},
        {"from": (835, 434), "to": (960, 378), "via": [(960, 434)], "both": True, "num": "6"},
        {"from": "relay", "to": "phone", "both": True},
    ]
    return svg(1180, 545, B, E, extra_before="".join(g), extra_after=core, title="系统拓扑")


def fig_request():
    B = [Box("draft", 110, 100, 150, 56, "草稿", "muted", sub="表单 / 点击"),
         Box("pending", 400, 100, 170, 56, "挂起", "run", sub="带 request_id 发出"),
         Box("ok", 720, 100, 170, 56, "已确认", "done", "final", sub="才显示成功、关表单"),
         Box("rej", 720, 280, 170, 56, "被拒", "err", sub="原因 · 输入原样留着"),
         Box("unk", 400, 280, 170, 56, "结果未知", "wait", sub="超时 10s · 断线")]
    E = [
        {"from": "draft", "to": "pending", "label": "提交", "ldy": -14},
        {"from": "pending", "to": "ok", "label": "宿主回 Applied\n（已落库）", "ldy": -22},
        {"from": "pending", "to": "rej", "cls": "err", "label": "Rejected{原因, 对象现状}", "ldx": 40, "ldy": -12},
        {"from": "pending", "to": "unk", "cls": "err", "label": "没等到回执", "ldx": -48},
        {"from": "unk", "to": "pending", "bend": -50, "cls": "thin", "label": "「重试」\n同一个 id", "ldx": 50},
        {"from": "rej", "to": "draft", "via": [(720, 380), (110, 380)], "cls": "thin", "label": "改一改再提交（新 id）", "seg": 2, "ldy": 14},
    ]
    note = label_at(440, 440, "没有「忙」：可以同时挂多个请求。宿主按（客户端, request_id）记住最近的结果，\n同一个 id 再来直接回上次的结果，不会执行第二次；所以结果未知时重试，若其实已生效就直接进「已确认」", "e-label")
    return svg(880, 470, B, E, extra_before=init_dot(20, 100, B[0], ""), extra_after=note, title="一次请求的状态")


def fig_actor():
    B = [Box("c", 120, 80, 170, 56, "客户端 Op", "", "mod"),
         Box("s", 120, 170, 170, 56, "观测信号", "", "mod"),
         Box("t", 120, 260, 170, 56, "定时到期", "", "mod"),
         Box("q", 360, 170, 130, 150, "输入队列\n（有界）", "", "mod"),
         Box("a", 580, 170, 190, 150, "状态线程\n检查前提\n应用到内存\n算出差异", "run", "mod"),
         Box("w", 860, 100, 190, 64, "写库线程\n按序号提交事务", "", "mod"),
         Box("p", 860, 240, 190, 64, "推送\n差异 → 订阅者", "", "mod"),
         Box("r", 1100, 100, 150, 64, "回执给发起方", "done", "mod"),
         Box("d", 1100, 240, 150, 64, "Degraded\n+ 重试", "err", "mod")]
    E = [{"from": "c", "to": "q"}, {"from": "s", "to": "q"}, {"from": "t", "to": "q"}, {"from": "q", "to": "a"},
         {"from": "a", "to": "w", "label": "差异 + 序号", "ldy": -14}, {"from": "a", "to": "p"},
         {"from": "w", "to": "r", "label": "提交成功", "ldy": -14},
         {"from": "w", "to": "d", "cls": "err", "label": "失败", "ldx": 14}]
    note = label_at(640, 345, "状态线程从不做 I/O；PTY 读写从不等它。写库失败时内存不回滚，记 Degraded 并按序号重试，直到追上", "e-label")
    return svg(1200, 370, B, E, extra_after=note, title="宿主内部的数据流")


def fig_process2():
    B = [Box("starting", 130, 130, 150, 56, "Starting", "", sub="启动中"),
         Box("running", 400, 130, 150, 56, "Running", "run", sub="在跑"),
         Box("draining", 670, 130, 150, 56, "Draining", "wait", sub="已退出 · 读余下输出"),
         Box("exited", 940, 130, 150, 56, "Exited", "done", sub="结束（含退出码）"),
         Box("failed", 130, 320, 150, 56, "Failed", "err", "final", sub="没起来"),
         Box("interrupted", 540, 320, 170, 56, "Interrupted", "err", sub="宿主没交接就重启"),
         Box("reclaimed", 940, 320, 170, 56, "Reclaimed", "muted", "final", sub="fd · 缓冲已释放，只留记录")]
    E = [
        {"from": "starting", "to": "running", "label": "spawn 成功", "ldy": -16},
        {"from": "starting", "to": "failed", "label": "spawn 报错", "ldx": -40},
        {"from": "running", "to": "draining", "label": "子进程退出", "ldy": -16},
        {"from": "draining", "to": "exited", "label": "读完 · 或 5s 超时", "ldy": -16},
        {"from": "running", "to": "exited", "bend": -80, "label": "退出时已读完"},
        {"from": "running", "to": "interrupted", "cls": "err", "label": "宿主崩溃 / 被杀", "ldx": -70},
        {"from": "exited", "to": "reclaimed", "label": "没有窗格在看\n且超过保留时间", "ldx": 70},
        {"from": "interrupted", "to": "reclaimed", "cls": "thin", "label": "同上", "ldy": -12},
    ]
    return svg(1080, 400, B, E, extra_before=init_dot(20, 130, B[0], ""), title="终端进程（目标）")


def fig_approval2():
    B = [Box("none", 120, 120, 150, 56, "没有审批", "muted"),
         Box("shown", 420, 120, 190, 64, "Shown{id}", "wait", sub="id = 指纹(问句 + 命令行)"),
         Box("answering", 760, 120, 170, 56, "Answering", "run", sub="宿主重读画面"),
         Box("answered", 1050, 120, 150, 56, "Answered", "done", "final", sub="按了回车 / Esc"),
         Box("gone", 420, 320, 190, 56, "Gone", "muted", sub="画面上已经没有了"),
         Box("mismatch", 760, 320, 170, 56, "Mismatch", "err", "final", sub="拒绝：已经换了一个")]
    E = [
        {"from": "none", "to": "shown", "label": "画面规则认出\n截出问句和命令", "ldy": -26},
        {"from": "shown", "to": "answering", "label": "回答{id, 允许?}", "ldy": -16},
        {"from": "answering", "to": "answered", "label": "id 一致", "ldy": -16},
        {"from": "answering", "to": "mismatch", "cls": "err", "label": "id 不一致", "ldx": 44},
        {"from": "shown", "to": "gone", "label": "回合回到 Running\n或 Agent 不在了", "ldx": -70},
        {"from": "gone", "to": "none", "cls": "thin", "via": [(120, 320)]},
        {"from": "shown", "to": "shown", "label": "同一个 id 被多路信号报到：不变，不重复通知"},
    ]
    note = label_at(640, 420, "手机和桌面都只能回答带 id 的审批；不再有不核对画面的「选 1 / 2 / 3」「Esc」。卡片展示命令全文，过长折叠", "e-label")
    return svg(1130, 445, B, E, extra_before=init_dot(20, 120, B[0], ""), extra_after=note, title="审批实例")


def fig_health():
    B = [Box("boot", 130, 110, 150, 56, "Booting", "muted", sub="开库 · 恢复终端"),
         Box("healthy", 430, 110, 150, 56, "Healthy", "done"),
         Box("degraded", 760, 110, 190, 64, "Degraded{原因}", "wait", sub="重试中 · 状态栏小提示"),
         Box("fatal", 130, 300, 150, 56, "起不来", "err", "final", sub="只在启动时，告诉 App 原因")]
    E = [
        {"from": "boot", "to": "healthy", "label": "就绪", "ldy": -14},
        {"from": "boot", "to": "fatal", "cls": "err", "label": "库损坏\n拿不到锁", "ldx": -44},
        {"from": "healthy", "to": "degraded", "bend": -30, "cls": "err", "label": "写库失败 · 长期线程出错\n识别订阅中断 · hooks 被丢", "ldy": -26},
        {"from": "degraded", "to": "healthy", "bend": -30, "label": "所有原因都已恢复", "ldy": 16},
        {"from": "degraded", "to": "degraded", "label": "原因可以叠加，各自重试"},
    ]
    note = label_at(470, 360, "启动之后宿主不因为后台错误退出：它持有所有终端，退出等于全部挂断\n长期线程（定时、日志订阅、远程）由监督者重启，并记一条 Degraded 原因", "e-label")
    return svg(940, 400, B, E, extra_before=init_dot(20, 110, B[0], ""), extra_after=note, title="宿主健康")


def fig_handoff2():
    parts = [("new", 170, "新宿主"), ("old", 520, "旧宿主"), ("app", 860, "App / 终端")]
    steps = [
        ("msg", "new", "old", "Hello{major}", 90, ""),
        ("msg", "old", "new", "Prepare：终端列表 + 每个 VT 快照 + PTY fd", 130, ""),
        ("self", "new", "只读打开数据库\n恢复每个 VT\n全部成功才往下", 148, "run"),
        ("msg", "new", "old", "READY", 240, ""),
        ("self", "old", "停止接受写入\n写库线程刷完\n放开 host.lock", 258, "wait"),
        ("msg", "old", "new", "COMMIT", 350, ""),
        ("self", "new", "拿 host.lock · 开 socket\n开始服务", 368, "run"),
        ("msg", "new", "old", "SERVING", 445, ""),
        ("self", "old", "收到才退出\n（多等这一步，11-02 定）", 463, ""),
        ("msg", "new", "app", "App 重连，终端照跑", 548, ""),
    ]
    out = seq(1000, 620, parts, steps, "两阶段交接")
    note = ('<text class="e-label err" x="500" y="600" text-anchor="middle">READY 之前任何失败：旧宿主继续服务，新宿主退出；'
            'SERVING 之前失败：旧宿主重新拿锁、接着服务</text>')
    return out.replace("</svg>", note + "</svg>")


def fig_device():
    B = [Box("offer", 120, 120, 160, 56, "待确认", "user", sub="扫码 / 链接打开"),
         Box("paired", 450, 120, 170, 64, "已配对{角色}", "done", sub="独立 token"),
         Box("kicked", 450, 320, 170, 56, "踢出", "wait", sub="5 分钟内拒绝重连"),
         Box("revoked", 800, 120, 160, 56, "已撤销", "muted", "final", sub="token 作废")]
    E = [
        {"from": "offer", "to": "paired", "label": "你在手机上确认\n电脑上也看得到", "ldy": -26},
        {"from": "paired", "to": "paired", "label": "改角色：断开，重连即生效"},
        {"from": "paired", "to": "kicked", "bend": 30, "cls": "err", "label": "踢出", "ldx": -30},
        {"from": "kicked", "to": "paired", "bend": 30, "cls": "thin", "label": "冷却到期", "ldx": 40},
        {"from": "paired", "to": "revoked", "cls": "err", "label": "撤销", "ldy": -14},
        {"from": "kicked", "to": "revoked", "cls": "err", "label": "撤销", "ldx": 30},
    ]
    note = label_at(470, 400, "每台手机一个路由 token：中继上有人顶替，只影响那一台，本机不会整体下线", "e-label")
    return svg(940, 420, B, E, extra_before=init_dot(20, 120, B[0], ""), extra_after=note, title="设备（宿主侧）")


def fig_phone2():
    B = [Box("connecting", 150, 120, 170, 56, "connecting", "wait", sub="握手 → hello"),
         Box("online", 480, 120, 170, 56, "online", "done", sub="心跳 15s"),
         Box("offline", 150, 320, 170, 56, "offline", "err", sub="1s 起翻倍 ≤ 30s"),
         Box("cool", 480, 320, 170, 56, "cooldown", "wait", sub="被踢，显示剩余时间"),
         Box("repair", 820, 320, 170, 56, "要重新配对", "err", "final", sub="已撤销")]
    E = [
        {"from": "connecting", "to": "online", "label": "hello{角色, 能做的操作}", "ldy": -16},
        {"from": "connecting", "to": "offline", "cls": "err", "label": "连不上", "ldx": -36},
        {"from": "offline", "to": "connecting", "bend": 40, "cls": "thin", "label": "退避到期\n回到前台", "ldx": 50},
        {"from": "online", "to": "offline", "cls": "err", "label": "断开 · 心跳超时", "ldx": 70, "ldy": -24},
        {"from": "online", "to": "connecting", "bend": 60, "cls": "thin", "label": "4403 改了角色：立即重连", "ldy": -10},
        {"from": "online", "to": "cool", "cls": "err", "label": "4429 踢出", "ldx": 44},
        {"from": "cool", "to": "connecting", "via": [(330, 400), (150, 400)], "cls": "thin", "label": "冷却到期", "seg": 1, "ldy": 14},
        {"from": "online", "to": "repair", "cls": "err", "label": "4401 撤销", "ldx": 60},
    ]
    note = label_at(510, 455, "每个请求 10s 超时、带 request_id；重连后自动重订事件流；终端里换了对话，宿主推一条 session_switched", "e-label")
    return svg(1020, 475, B, E, extra_before=init_dot(30, 60, B[0], ""), extra_after=note, title="手机连接（手机侧）")


def fig_recog2():
    B = [Box("starting", 130, 110, 150, 56, "Starting", "muted"),
         Box("healthy", 420, 110, 150, 56, "Healthy", "done"),
         Box("recovering", 740, 110, 170, 56, "Recovering", "wait", sub="补扫中"),
         Box("partial", 740, 300, 190, 64, "Healthy + 隔离名单", "wait", sub="读不了的文件单独列出")]
    E = [
        {"from": "starting", "to": "healthy", "label": "订阅就绪", "ldy": -14},
        {"from": "healthy", "to": "recovering", "bend": -30, "cls": "err", "label": "丢事件 · 订阅出错", "ldy": -14},
        {"from": "recovering", "to": "healthy", "bend": -30, "label": "补扫成功，检查点前进", "ldy": 16},
        {"from": "recovering", "to": "partial", "cls": "err", "label": "个别文件读不了\n（权限、删了一半）", "ldx": 70},
        {"from": "partial", "to": "healthy", "cls": "thin", "label": "文件恢复或删掉", "ldx": -60, "ldy": 10},
    ]
    note = label_at(470, 395, "检查点只在一批事件全部处理成功后前进；一个坏文件不拖住其他文件，也不无限重试", "e-label")
    return svg(940, 420, B, E, extra_before=init_dot(20, 110, B[0], ""), extra_after=note, title="识别健康（目标）")


def fig_conn2():
    B = [Box("attach", 130, 140, 160, 60, "Attaching", "wait", sub="Attach{lease?} · 收快照"),
         Box("owner", 430, 70, 160, 56, "可写", "run", sub="持有写租约"),
         Box("viewer", 430, 230, 160, 56, "只读", "muted"),
         Box("ended", 760, 70, 160, 56, "Ended", "done", "final"),
         Box("dropped", 760, 230, 160, 56, "Dropped", "err", sub="画面还在")]
    E = [
        {"from": "attach", "to": "owner", "label": "没人写\n或带回自己的租约", "ldy": -26},
        {"from": "attach", "to": "viewer", "label": "别人在写", "ldy": 18},
        {"from": "owner", "to": "viewer", "bend": 25, "label": "被抢", "ldx": -40},
        {"from": "viewer", "to": "owner", "bend": 25, "label": "抢写权", "ldx": 44},
        {"from": "owner", "to": "ended", "label": "进程退出", "ldy": -14},
        {"from": "owner", "to": "dropped", "cls": "err", "label": "流断了\n输入结果未知，不重发", "ldx": 50, "ldy": -20},
        {"from": "viewer", "to": "dropped", "cls": "err"},
        {"from": "dropped", "to": "attach", "via": [(760, 320), (130, 320)], "cls": "thin", "label": "重连带上租约，30s 内仍是可写；首次连接失败也重试", "seg": 2, "ldy": 14},
    ]
    return svg(900, 355, B, E, extra_before=init_dot(20, 140, B[0], ""), title="窗格连接（目标）")


# ───────── 表 ─────────

STATUS = table(
    ["状态", "词", "符号", "颜色", "什么时候", "要你注意"],
    [
        ["Waiting", "等你回答", "◆", "橙 #e8823a", "Agent <b>还活着</b>，回合停在提问或审批", "是"],
        ["Error", "出错了", "✕", "红 #e5534b", "回合出错，或终端失败 / 非零退出", "是"],
        ["Running", "在跑", "◐", "黄 #e8c547", "回合进行中；或回合结束但子 Agent 还在为它工作", "—"],
        ["Background", "等后台", "◔", "黄 #e8c547", "回合结束，它起的 shell / 监视 / 定时唤醒还在", "—"],
        ["Review", "等你验收", "◇（看过画空心圈）", "绿 #4cc38a", "回合结束，做没做完由你说", "没看过时是"],
        ["Fresh", "新开的", "○", "灰蓝 #8a9bb0", "活着的 Agent 还没开始第一轮", "—"],
        ["Paused", "中断 *", "‖", "灰蓝 #8a9bb0", "回合被打断，或 <b>Agent 在等你时不在了</b>（11-02 定）", "—"],
        ["Stale", "很久没动", "·", "不上色", "超过 stale_days 没动静", "—"],
        ["Deferred", "稍后再看", "◷", "灰蓝 #8a9bb0", "你推迟到某个时间，还没到", "—"],
        ["Closed", "做完了", "✓（次要文字色）", "不上色", "<b>只有你</b>标记完成或放弃", "—"],
    ])

DERIVE2 = ladder([
    ("你定的结局：<code>Completed</code> / <code>Abandoned</code>（活着的 Agent 回合在跑时，先看回合）", "做完了", "done"),
    ("<code>Deferred</code> 且没到期", "稍后再看", "wait"),
    ("没在跑且超过 <code>stale_days</code>", "很久没动", "muted"),
    ("<code>turn = Waiting</code> <b>且 Agent 活着</b>（当前对话、终端 Running、进程还在）", "等你回答", "wait"),
    ("<code>turn = Waiting</code> 但 Agent 不在了", "中断", "muted"),
    ("<code>turn = Error</code>，或终端 Failed / 非零退出", "出错了", "err"),
    ("<code>turn = Running</code>（活着，或日志 15 分钟内有更新），或 Completed 但子 Agent 还在", "在跑", "run"),
    ("<code>turn = Completed</code>，后台还有 shell / 监视 / 定时唤醒", "等后台", "run"),
    ("活着的 Agent 还没开始第一轮", "新开的", "muted"),
    ("<code>turn = Completed</code>", "等你验收", "user"),
    ("其余（Interrupted、Unknown、日志里的 Running 过期）", "中断", "muted"),
])

OPS = table(
    ["分组", "Op", "前提（不满足就拒绝，并回对象现状）", "说明"],
    [
        ["布局", "<code>CreateTab · RenameTab · MoveTab</code>", "标签存在", "只改指名的那一个"],
        ["", "<code>SplitPane{target, side, terminal?}</code>", "目标窗格存在", "Agent 的 <code>keepline split</code> 也走它"],
        ["", "<code>ResizeSplit{split, ratio}</code>", "分割存在", "拖分隔线每帧都发，后一个覆盖前一个，不会冲掉别的操作"],
        ["", "<code>MovePane · ClosePane{expect_terminal}</code>", "窗格存在；要停的终端就是你确认过的", "关窗格会停掉它的终端"],
        ["", "<code>CloseTab{expect_terminals}</code>", "标签里的终端集合 = 你确认过的", "多出来的就拒绝，让你重新确认"],
        ["空间", "<code>AddSpace · RenameSpace · SetSpaceDir</code>", "空间存在", ""],
        ["", "<code>RemoveSpace{expect_terminals}</code>", "不是最后一个；没有没收尾的待办；终端集合一致", ""],
        ["待办", "<code>AddTask{task_id}</code>", "无（id 由客户端生成，重发即幂等）", "「保存并启动」用同一个请求带上启动"],
        ["", "<code>RenameTask · AddCriterion · EditCriterion · RemoveCriterion</code>", "待办 / 标准存在", "只改文字不动证据"],
        ["", "<code>AttachEvidence · DetachEvidence</code>", "标准存在", "挂的是证据的拷贝"],
        ["", "<code>SetTaskOutcome{expect}</code>", "当前结局 = 你看到的", ""],
        ["会话", "<code>SetOutcome{expect, turn_key}</code>", "当前结局 = 你看到的；还是同一轮", "Agent 已开新一轮时拒绝，免得把新一轮也标成做完"],
        ["", "<code>AssignTask · RenameSession</code>", "会话存在", ""],
        ["设置", "<code>SetSetting{key, value}</code>", "无（按键覆盖）", "每个键独立，不整份替换"],
    ])

REQ = table(
    ["字段", "含义"],
    [
        ["<code>client_id</code>", "每个 App 窗口、每台手机、每次 CLI 调用一个"],
        ["<code>request_id</code>", "客户端生成；重发用同一个"],
        ["<code>op</code>", "上表里的一种"],
        ["回执 <code>Applied{seq}</code>", "已经落库；<code>seq</code> 是这次改动的全局序号，客户端据此知道推送的差异是否已经包含它"],
        ["回执 <code>Rejected{reason, current}</code>", "前提不满足或参数非法；带上对象现状，界面直接更新"],
        ["回执 <code>Duplicate(上次结果)</code>", "同一个 id 已经处理过"],
    ])

AGGS = table(
    ["聚合", "包含", "谁改", "推送"],
    [
        ["Layout", "spaces、tabs（分割树）", "你的布局 Op；Agent 的 split", "差异"],
        ["Terminals", "PTY 身份、ProcessState、启动参数、cwd", "宿主（spawn、退出、回收）", "差异"],
        ["Sessions", "对话身份、绑定、结局、结局历史、挂的待办", "识别层（绑定）；你（结局）", "差异"],
        ["Content", "每个会话的回合卡片：turn、waiting、approval、background、evidence", "只有观测层", "<b>单独一条流</b>，高频，不和你的编辑抢"],
        ["Tasks", "待办、验收标准、证据拷贝、结局", "只有你", "差异"],
        ["Settings", "按键存", "只有你", "差异"],
        ["Health", "宿主健康、识别、远程、额度读数", "宿主", "差异"],
    ])

LOCKS2 = table(
    ["线程", "拥有", "可以阻塞吗"],
    [
        ["状态线程（1 个）", "全部聚合的内存状态", "<b>不能</b>：不做 I/O、不 fork、不等锁"],
        ["写库线程（1 个）", "SQLite 连接", "可以：fsync 只卡它自己"],
        ["每终端 pump", "这个终端的 Live（VT、PTY master、订阅者、写租约）", "只等自己的 PTY；画面信号投进状态线程的队列"],
        ["副作用工人（池）", "spawn、ps / lsof、hooks.json、webhook、通知回执", "可以；结果作为信号投回"],
        ["观测线程", "日志索引（自己的库）、FSEvents、Hook 接收", "可以；产出信号"],
        ["每客户端读 / 写", "一条 socket", "写用 try_send，慢客户端被断开，不拖别人"],
        ["监督者", "上面所有长期线程", "线程出错就重启，并记 Degraded 原因"],
    ])

TRUST2 = table(
    ["输入", "信任", "门控"],
    [
        ["本机同一用户的进程（App、CLI、<b>终端里的 Agent</b>）", "可信（11-02 定）", "runtime 目录 0700、socket 0600；不做额外确认"],
        ["插件目录", "可信（同上）", "只校验格式；wasm 左栏仍跑沙箱，因为它是第三方代码的渲染"],
        ["网页 / 别的 App 打开的配对链接", "<b>不可信</b>", "手机上先展示「哪台电脑、什么角色」，你点确认才保存"],
        ["手机经中继", "按设备和角色", "Noise 握手 → hello → 角色 → 操作白名单；请求带 id、有超时"],
        ["中继服务器", "<b>不可信</b>", "只见密文和每台手机的路由 token；握手前的连接 10s 超时，每个 token 限连接数"],
        ["仓库内容（OSC 7 给出的目录）", "<b>不可信</b>", "跑 git 时关掉 fsmonitor 和外部 hook"],
        ["拼进命令的文字（任务名、会话标题）", "<b>不可信</b>", "动作模板只做单引号转义，从不在双引号里展开"],
        ["磁盘上的 Agent 日志、PTY 画面", "数据，不是指令", "不拼进命令；只用来派生状态"],
    ])

PERSIST = table(
    ["表", "内容", "写入时机"],
    [
        ["<code>ops</code>", "每次改动的序号、聚合、差异", "写库线程按序号提交"],
        ["<code>layout · terminals · sessions · tasks · settings</code>", "各聚合当前值", "同一事务"],
        ["<code>content</code>", "回合卡片", "单独节流（最多每秒一次），丢了能从日志重建"],
        ["<code>requests</code>", "（client_id, request_id）→ 结果，保留 24 小时", "和改动同一事务，保证「执行了」和「记住了」一致"],
        ["<code>notifications</code>", "每会话一条回执：上次通知的 key", "通知前"],
        ["<code>devices</code>", "每台手机：token hash、角色、冷却到期时间", "配对 / 改角色 / 踢出"],
        ["日志索引（独立库）", "游标、检查点、隔离名单", "一批事件处理成功后"],
    ])

FAIL2 = table(
    ["故障", "用户看到", "系统做什么"],
    [
        ["写库失败（忙、磁盘满）", "状态栏小提示「后台没存上，重试中」", "内存照常，按序号重试；前台请求的回执要等写成功，所以你看到的是「结果未知」而不是假成功"],
        ["Agent 在等你时被杀", "卡片变「中断」", "派生规则看存活，不靠事后清理"],
        ["终端退出时写库失败", "仍然正确显示已退出", "退出事件进重试队列，不再只打日志"],
        ["一个日志文件读不了", "识别正常，诊断里列出这个文件", "隔离名单，不拖住别的文件"],
        ["长期线程出错", "状态栏小提示", "监督者重启；宿主不退出"],
        ["宿主崩溃", "终端断开，重启后标 Interrupted，可用原参数恢复对话", "PTY 无法跨崩溃保留，这是唯一会丢终端的情况"],
        ["交接任一步失败", "终端不断，App 提示升级没成功", "两阶段：SERVING 之前旧宿主都能接着服务"],
        ["手机请求超时", "「不确定是否成功」+ 重试", "同一 request_id 重试不会执行两次"],
        ["中继上有人顶替一台手机的 token", "只有那台手机掉线", "每台手机独立 token"],
        ["hooks.json 同时跑满", "诊断里计数", "丢最旧的并计数，记 Degraded 原因"],
    ])

TESTS = table(
    ["要守住的规则", "怎么验收"],
    [
        ["无关的后台提交不挡你的编辑", "Agent 持续输出时连续标记完成 100 次，0 次被拒"],
        ["每次操作要么生效要么明确失败，输入还在", "注入写库失败：表单不关、文字还在、按钮显示「结果未知 · 重试」"],
        ["重发不执行两次", "回执前断开连接再重发同一 id，待办只多一个"],
        ["布局操作互不冲掉", "Agent 执行 split 的同时改标签名、拖分隔线，三个改动都在"],
        ["你确认停的终端就是实际停的", "确认框打开后 Agent 新分屏，关标签被拒并要求重新确认"],
        ["批的就是你看到的那个审批", "两个问句相同、命令不同的审批先后出现，用旧卡片点允许被拒"],
        ["需要你注意的状态有活着的 Agent 支撑", "Ctrl-C 掉正在问审批的 Agent，1 秒内卡片变「中断」，手机推送撤回"],
        ["后台错误能被看见并自愈", "第一次写库返回 SQLITE_BUSY，状态栏出现提示，随后自动消失，数据正确"],
        ["PTY 读不等状态", "20 个终端持续输出 + 日志批量写 + 每秒 50 个 Hook：PTY 读最大停顿 &lt; 16ms（这就是旧仓库待压测的 I19，在新仓库里当验收指标）"],
        ["终端会被回收", "开关 200 个终端后，fd 数和内存回到基线"],
        ["交接不断终端", "在 Prepare / READY / COMMIT / SERVING 前后各注入一次失败，终端都不断"],
        ["一台手机出事不影响别的", "撤销手机 A，A 的 token 无法让本机掉线；手机 B 照常"],
        ["所有界面同一个状态", "固定一组会话快照，侧栏、看板、菜单栏、手机的状态逐个相等（单测）"],
    ])

MILESTONES = [
    ("M0 · core", "数据模型、7 个聚合、Op 与前提、状态派生（第 5 节）、回合规则，全是纯函数。", "派生和前提的单测全过；第 13 节「所有界面同一个状态」的单测。"),
    ("M1 · 宿主骨架", "PTY 与 pump、状态线程、写库线程、请求去重、健康状态、监督者。", "压测指标（PTY 读停顿 &lt; 16ms）；写库失败注入；终端回收。"),
    ("M2 · 本机协议与最小 App", "control socket、请求回执、差异推送、窗格连接与写租约；App 的请求状态机和侧栏。", "第 13 节前 5 条。"),
    ("M3 · 识别", "日志索引、Hook、画面规则、进程表、绑定；Content 流。审批实例 id。", "审批两条；被杀变「中断」；坏文件隔离。"),
    ("M4 · 看板、待办、通知、插件", "看板五列、待办与证据、系统通知与 hooks.json、插件七种挂点。", "通知去重；hooks 丢弃计数。"),
    ("M5 · 远程", "设备表、每机 token、关闭码、心跳、请求超时与幂等、会话跟随；手机只做聊天和审批。", "手机相关三条。"),
    ("M6 · 交接", "两阶段交接；从旧仓库宿主迁移数据的导入工具。", "四个注入点都不断终端。"),
]

OPEN = table(
    ["待定或待验证", "影响", "怎么定"],
    [
        ["看板列数：11-02 先记「保持五列」，同日又去掉「要你处理」列成四列（等你的卡留在「在跑」顶部）", "看板", "已定（11-02）：以最新的四列为准"],
    ["「中断」这个状态的界面词：沿用「停下了」，还是改成「中断」", "第 1 节状态表", "你定"],
        ["审批指纹的稳定性：重绘、折行会不会让同一个审批的 id 抖动", "第 7 节审批实例", "M3 开始前，用 Claude、Codex 各录 20 个真实审批画面测"],
        ["状态线程 + 写库线程这个结构，是我在新仓库直接采用的（旧仓库里 E1 定为「先压测再动」，那是因为改造成本）", "第 6 节", "已定（11-02）：采用"],
        ["新旧数据迁移：新仓库要不要导入旧库里的会话、待办", "M6", "你定"],
        ["多台电脑", "第 10 节", "按 11-02 的约定，手机只连一台电脑；以后再说"],
    ])

APPENDIX_SEC = {"A": "第 4、5 节", "B": "第 7 节审批", "C": "第 5 节 Op 表", "D": "第 5、7、12 节", "E": "第 6 节",
                "F": "第 8 节", "G": "第 9 节", "H": "第 10 节", "I": "第 5 节", "J": "—"}


def appendix():
    rows = []
    for i in gen.ISSUES:
        dec = gen.DECIDED.get(i[0], "")
        where = APPENDIX_SEC[i[1]]
        if i[0] in ("I27", "I31"):
            where = "不修（Agent 可信）"
        elif i[0] == "I38":
            where = "旧仓库已解决；第 5 节保持"
        elif i[0] == "I41":
            where = "旧文档，不搬"
        rows.append(f"<tr><td><b>{i[0]}</b></td><td>{esc(i[5])}</td><td>{esc(where)}</td></tr>")
    return '<div class="tbl"><table><tr><th>旧编号</th><th>问题</th><th>在本设计里怎么解决</th></tr>' + "".join(rows) + "</table></div>"


PRODUCT = [
    "<b>做完了只由你定。</b>一轮结束叫「等你验收」；Agent 说「完成了」只记下来，不改结局。看板列和状态同名（列数见第 15 节）。",
    "<b>宿主持有所有终端。</b>关掉窗口、App 崩溃，终端和 Agent 照跑；只有宿主崩溃才会丢终端。",
    "<b>任何 Agent 都能接。</b>各家差异只在适配层（日志位置、Hook、画面规则、官方状态）；没有对话的程序可以用 OSC 7501 自报状态。",
    "<b>Agent 可信。</b>本机同一用户的进程都可信，包括终端里的 Agent（11-02 定）；只防外来输入。",
    "<b>手机只做聊天和审批，不做终端。</b>自建中继，不用 Tailscale / iroh；中继只看到密文。",
    "<b>派活回报。</b><code>keepline run</code> 在旁屏启动 Agent 交任务，每结束一轮把结果在你闲下来时打进你的输入框，注明不是用户说的话。",
    "<b>不替用户改 Agent 的启动参数。</b>例如不给 Codex 加 <code>--no-daemon</code>。",
    "<b>仓库里不出现参考产品的名字。</b>",
]

PRINCIPLES = [
    ("P1", "每个请求都有自己的状态", "挂起 → 已确认 / 被拒 / 结果未知。成功提示等确认；被拒时输入还在；未知时可以用同一个 id 安全重试。", "第 5、10 节；请求表"),
    ("P2", "写入只带它在意的前提", "没有全局版本号。新建不带前提；改结局带「我看到的结局」；布局是操作，不是整块覆盖。", "第 5 节 Op 表"),
    ("P3", "状态只在一处派生，并看观测源还活着", "core 里一个 status 函数；需要你注意的状态必须有活着的 Agent 支撑。", "第 5 节派生"),
    ("P4", "状态线程不做 I/O；会失败的步骤放在承诺之前", "fsync、fork、跑命令、网络都在别的线程；交接先 Prepare 全部成功再 Commit。", "第 6、8 节"),
    ("P5", "宿主不能随便退出", "出错时降级、显示出来、重试；长期线程由监督者重启。", "第 7 节健康"),
    ("P6", "外来的信任要显式授予", "配对链接要确认；每台手机独立 token；外来文字不进命令。", "第 9 节"),
]


def page():
    pr = "".join(f'<tr><td><b>{a}</b></td><td><b>{esc(b)}</b><div class="sc">{esc(c)}</div></td><td>{esc(d)}</td></tr>' for a, b, c, d in PRINCIPLES)
    prod = "".join(f"<li>{x}</li>" for x in PRODUCT)
    ms = "".join(f"<tr><td><b>{esc(a)}</b></td><td>{b}</td><td>{c}</td></tr>" for a, b, c in MILESTONES)
    nav_items = [("goal", "0 目的"), ("product", "1 产品规则"), ("topo", "2 拓扑"), ("principles", "3 原则"), ("model", "4 数据模型"),
                 ("write", "5 写入与派生"), ("host", "6 宿主内部"), ("sm", "7 状态机"), ("handoff", "8 交接"), ("trust", "9 信任"),
                 ("remote", "10 远程"), ("persist", "11 持久化"), ("fail", "12 故障"), ("tests", "13 验收"), ("ms", "14 里程碑"),
                 ("open", "15 待定"), ("map", "附录 旧问题对照")]
    nav = "".join(f'<a href="#{a}">{esc(b)}</a>' for a, b in nav_items)
    return f"""<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Keepline 目标设计</title><style>{CSS}
.sc{{color:var(--muted);font-size:13px;margin:2px 0}}ul.rules li{{margin:4px 0}}</style></head>
<body><main>
<button class="toggle" onclick="var r=document.documentElement;r.dataset.theme=(r.dataset.theme==='dark'||(!r.dataset.theme&&matchMedia('(prefers-color-scheme: dark)').matches))?'light':'dark'">明 / 暗</button>
<h1>Keepline 目标设计（新仓库用）</h1>
<p class="lead">新仓库从头搭建时照这一份做：产品规则、模块边界、数据模型、写入协议、每个状态机的目标版本、并发、持久化、远程、故障和验收。它吸收了对旧实现的六层审计（41 个问题）和 2026-11-02 定下的全部决定，不依赖旧代码的行号。</p>
<nav>{nav}</nav>

<h2 id="goal">0 · 这份设计支撑什么决定</h2>
<p class="note"><b>支撑的决定：</b>新仓库的骨架按这里搭，每个里程碑按第 13 节的验收条目判定完成。<b>放弃条件：</b>M1 的压测达不到指标，或 M3 前的审批指纹实验显示 id 不稳定，就回到对应章节改设计，不硬做下去。旧仓库 <code>next/</code> 是参考实现：可以看它怎么接各家 Agent、怎么画界面，不照搬它的状态管理。</p>

<h2 id="product">1 · 产品规则（不随实现变）</h2>
<ul class="rules">{prod}</ul>
<h3>会话状态：一个会话一个状态，所有界面同一个词、符号、颜色</h3>
{STATUS}
<p class="note">* 「中断」的界面词待你定（第 15 节）。状态色只用来画会话；表单报错、额度、连接这类用另一套「好 / 注意 / 失败」色调，不借状态色。</p>

<h2 id="topo">2 · 进程与模块</h2>
<p class="note">一个二进制兼任宿主和命令行（<code>keepline serve</code> / <code>keepline list</code> …），一个桌面 App，一个中继，一个手机 App。App、CLI、手机都只是宿主的客户端，所有状态在宿主。</p>
<div class="fig">{fig_topology()}</div>
<ol class="flows">
<li>Agent 跑在宿主开的 PTY 里，字节只经过终端层；终端层每个终端一个 Live，从不等全局状态。</li>
<li>Agent 写的日志、调的 Hook 进观测层。</li>
<li>观测层只产出「信号」，投进状态线程的队列，自己不改状态。</li>
<li>状态线程把差异交给写库线程；回执要等写成功才发。</li>
<li>客户端的 Op 和订阅都走 control，按 client_id / request_id 去重。</li>
<li>手机经中继连 remote，每台手机一个设备记录、一个 token。</li>
</ol>
<p class="note"><b>模块依赖规则：</b><code>core</code> 没有 I/O、不依赖任何别的模块，状态派生、Op 前提、回合规则都在这里，能单测；宿主和 App 都依赖 core；App 只经协议和宿主说话，不读宿主的库；手机只依赖 remote-proto 的 TypeScript 版本。</p>

<h2 id="principles">3 · 六条原则</h2>
<p class="note">旧实现的 41 个问题背后是这六条没守住。新仓库在结构上保证它们，而不是靠每处小心。</p>
<div class="tbl"><table><tr><th></th><th>原则</th><th>落在哪</th></tr>{pr}</table></div>

<h2 id="model">4 · 数据模型：七个聚合</h2>
<p class="note">旧实现是一整个 Workspace 加一个全局 revision，会话内容约每秒改一次，就把你的编辑全挡掉了。新模型按「谁改」拆开：观测层改的 Content 单独成流，和你的编辑互不干扰；每次改动有一个全局序号 <code>seq</code>，只用来排序和对账，不当锁用。</p>
{AGGS}
<p class="note">身份：对话身份是 (runtime, cli_session_id)，和 PTY、窗格都无关；终端身份和摆放位置分开，关窗格不等于终端结束（但会停掉它）。待办挂到会话：第一次绑定时用这个终端预设的待办，没有就用标签的默认待办，之后不再改。</p>

<h2 id="write">5 · 写入协议与状态派生</h2>
<h3>请求信封</h3>
{REQ}
<h3>Op 与前提</h3>
<p class="note">每个 Op 只检查它在意的东西。没有「忙时丢弃」，客户端可以同时挂多个请求，宿主按到达顺序处理。</p>
{OPS}
<h3>一次请求的状态（App、CLI、手机共用）</h3>
<div class="fig">{fig_request()}{LEGEND}</div>
<p class="note">11-02 定：成功提示等确认再显示（本机通常几十毫秒）；超时显示「不确定是否成功」并给重试按钮。</p>
<h3>会话状态派生：<code>core::status::of</code>，先命中先算</h3>
<p class="note">唯一的判定入口。输入是结局、回合、终端状态、Agent 是否活着、后台任务、更新时间；所有界面只做展示映射。「活着」= 是这个终端的当前对话、终端 Running、Agent 进程还在。</p>
<div class="fig">{DERIVE2}</div>

<h2 id="host">6 · 宿主内部：一个状态线程 + 一个写库线程</h2>
<p class="note">旧实现一把全局锁下做 fsync、fork、跑 ps、写 socket，PTY 读的副作用也要等这把锁。新结构里状态线程只改内存，所有会阻塞的事都在别的线程。数据库用 WAL + <code>synchronous=NORMAL</code>：库不会坏，宿主崩溃时最多丢最后一次提交（11-02 定可以接受）；但回执在提交之后才发，所以你看到「成功」的一定已经落盘。</p>
<div class="fig">{fig_actor()}</div>
{LOCKS2}

<h2 id="sm">7 · 状态机（目标版本）</h2>
<h3>终端进程</h3>
<p class="note">比旧版多了 Draining 超时和回收：结束的终端最终释放 fd 和缓冲，只留记录。</p>
<div class="fig">{fig_process2()}{LEGEND}</div>
<h3>回合 <code>Turn</code>（沿用旧版规则）</h3>
<p class="note">由日志、Hook、画面、官方状态、子 Agent 五路信号驱动，每次变化记下是哪路信号定的。这一层旧版是对的，搬过来。</p>
<div class="fig">{gen.fig_turn()}{LEGEND}</div>
<h3>结局 <code>Outcome</code>（沿用）</h3>
<div class="fig">{gen.fig_outcome()}{LEGEND}</div>
<h3>对话绑定（沿用）</h3>
<div class="fig">{gen.fig_binding()}{LEGEND}</div>
<h3>待办与验收证据（沿用）</h3>
<div class="fig">{gen.fig_task()}{LEGEND}</div>
<div class="fig">{gen.fig_criterion()}</div>
<h3>审批实例（新）</h3>
<p class="note">旧版审批的身份只是那一行问句，问句相同的两个审批分不出来。新版用问句加命令行的指纹当 id，回答必须带 id。</p>
<div class="fig">{fig_approval2()}{LEGEND}</div>
<h3>宿主健康（新）</h3>
<div class="fig">{fig_health()}{LEGEND}</div>
<h3>识别健康（目标）</h3>
<div class="fig">{fig_recog2()}{LEGEND}</div>
<h3>窗格连接（目标）</h3>
<p class="note">写租约让抢过写权的窗格重连后还是可写的；首次连接失败也会重试。这把锁只管窗格；手机的发送走请求，受角色限制。</p>
<div class="fig">{fig_conn2()}{LEGEND}</div>

<h2 id="handoff">8 · 宿主升级：两阶段交接</h2>
<p class="note">旧版新宿主收到 fd 就确认，旧宿主马上退出，之后新宿主任何一步失败都会挂断全部终端。新版先把会失败的步骤全做完再承诺；承诺之后旧宿主还多等一个 SERVING（11-02 定）。</p>
<div class="fig">{fig_handoff2()}</div>

<h2 id="trust">9 · 信任模型</h2>
{TRUST2}

<h2 id="remote">10 · 远程</h2>
<p class="note">手机能做的操作：会话列表、读会话、发消息、回答审批（带 id）、事件流。不再有原始按键（11-02 定）。每个请求带 request_id、10 秒超时；宿主按（设备, request_id）去重。关闭码拆开：4401 撤销（要重新配对）、4403 改了角色（重连即可）、4429 踢出（5 分钟冷却，11-02 定）。会话视图跟着对话走：终端里 <code>/new</code> 之后宿主推 <code>session_switched</code>。</p>
<div class="fig">{fig_device()}{LEGEND}</div>
<div class="fig">{fig_phone2()}{LEGEND}</div>
<p class="note">宿主连中继：Off → Connecting → Connected；中继拒绝时进 Failed，改配置或重开就回 Connecting，不会卡死。协议版本：配对链接版本号必须相等；JSON 协议用「未知字段忽略」兼容，并在 hello 里报版本。</p>

<h2 id="persist">11 · 持久化</h2>
{PERSIST}

<h2 id="fail">12 · 故障与恢复</h2>
{FAIL2}

<h2 id="tests">13 · 验收：每条规则怎么证明</h2>
{TESTS}

<h2 id="ms">14 · 里程碑</h2>
<p class="note">按依赖排：先纯逻辑，再宿主骨架，再界面和识别，远程和交接最后。每个里程碑以对应的验收条目通过为完成。</p>
<div class="tbl"><table><tr><th>里程碑</th><th>内容</th><th>完成标准</th></tr>{ms}</table></div>

<h2 id="open">15 · 还没定或要先验证的</h2>
{OPEN}

<h2 id="map">附录 · 旧实现的 41 个问题在这里怎么解决</h2>
<p class="note">旧仓库的问题清单和设计稿：<a href="issues.html">issues.html</a> · <a href="design.html">design.html</a>；旧实现的状态机：<a href="next.html">next.html</a>。</p>
{appendix()}
<p class="note">本页由 <code>specs/architecture/blueprint.py</code> 生成，2026-11-02。</p>
</main></body></html>
"""


if __name__ == "__main__":
    OUT.write_text(page(), encoding="utf-8")
    print(OUT)
