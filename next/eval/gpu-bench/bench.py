"""GPU / CPU cost of the Keepline client under fixed terminal workloads, beside Ghostty.

Isolated fixture daemon, /bin/sh workloads only (no Agents, no real runtime). Per-process GPU
time is read from the AGX user clients' accumulatedGPUTime in ioreg; CPU from proc_pid_rusage.
  python3 bench.py [--app PATH] [--scenes idle,hidden4,vis1,vis4,flood] [--ghostty] [--secs 15]
The fixture window takes focus while it runs: keystrokes typed elsewhere land in it.
"""
import argparse, ctypes, glob, json, os, pathlib, re, shutil, socket, struct, subprocess, tempfile, time

HERE = pathlib.Path(__file__).resolve().parent
NEXT = HERE.parents[1]
ap = argparse.ArgumentParser()
ap.add_argument('--app', default=str(NEXT / 'target/release/keepline-app'))
ap.add_argument('--scenes', default='idle,hidden4,vis1,vis4,flood')
ap.add_argument('--ghostty', action='store_true')
ap.add_argument('--secs', type=float, default=15)
ap.add_argument('--label', default='')
ap.add_argument('--interact', action='store_true', help='move the pointer and type while measuring')
ap.add_argument('--size', default='', help='WxH points to resize the fixture window to')
args = ap.parse_args()

# Claude-Code-like status area: three lines redrawn in place every 80 ms.
SPIN = r'''i=0; printf '\n\n\n'; while :; do i=$((i+1)); printf '\033[3A\r\033[2K\033[38;5;174m* Thinking... (%ds  esc to interrupt)\033[0m\n\033[2K  > working on step %d\n\033[2K\033[2m  ? for shortcuts\033[0m\n' $((i/12)) $i; sleep 0.08; done'''
FLOOD = r'''i=0; while :; do i=$((i+1)); echo "line $i  the quick brown fox jumps over the lazy dog $i"; done'''
IDLE = r'''printf 'idle pane\n'; exec /bin/cat'''

lib = ctypes.CDLL('/usr/lib/libproc.dylib')
class Usage(ctypes.Structure):
    _fields_ = [('uuid', ctypes.c_ubyte * 16)] + [(x, ctypes.c_uint64) for x in ['user', 'system', 'idle', 'interrupt', 'pageins', 'wired', 'resident', 'footprint', 'start', 'exit', 'child_user', 'child_system', 'child_idle', 'child_interrupt', 'child_pageins', 'child_elapsed', 'read', 'write']]
class Timebase(ctypes.Structure): _fields_ = [('numer', ctypes.c_uint32), ('denom', ctypes.c_uint32)]
tb = Timebase(); ctypes.CDLL('/usr/lib/libSystem.B.dylib').mach_timebase_info(ctypes.byref(tb))
def cpu_ns(pid):
    u = Usage()
    if lib.proc_pid_rusage(pid, 2, ctypes.byref(u)) != 0: return 0
    return (u.user + u.system) * tb.numer / tb.denom

def gpu_ns():
    out = subprocess.run(['ioreg', '-r', '-c', 'AGXDeviceUserClient', '-w', '0', '-l'], capture_output=True, text=True).stdout
    d = {}
    for blk in out.split('+-o ')[1:]:
        m = re.search(r'"IOUserClientCreator" = "pid (\d+),', blk)
        if m:
            d[int(m[1])] = d.get(int(m[1]), 0) + sum(int(x) for x in re.findall(r'"accumulatedGPUTime"=(\d+)', blk))
    return d

def front(pid):
    subprocess.run(['osascript', '-e', f'tell application "System Events" to set frontmost of (first process whose unix id is {pid}) to true'], capture_output=True)
    time.sleep(1)

def measure(pids, secs):
    g0, c0 = gpu_ns(), {p: cpu_ns(p) for p in pids}
    t0 = time.monotonic(); time.sleep(secs); dt = time.monotonic() - t0
    g1, c1 = gpu_ns(), {p: cpu_ns(p) for p in pids}
    return {p: {'gpu_ms_per_s': round((g1.get(p, 0) - g0.get(p, 0)) / 1e6 / dt, 2),
                'cpu_pct': round((c1[p] - c0[p]) / 1e9 / dt * 100, 1)} for p in pids}

def read(s, n):
    data = b''
    while len(data) < n:
        part = s.recv(n - len(data))
        if not part: raise EOFError()
        data += part
    return data
def receive(s): return json.loads(read(s, struct.unpack('>I', read(s, 4))[0])[1:])
def send(s, obj):
    data = b'\x01' + json.dumps(obj).encode(); s.sendall(struct.pack('>I', len(data)) + data)

class Fixture:
    def __init__(self):
        self.root = pathlib.Path(tempfile.mkdtemp(prefix='kl-gpu-', dir='/tmp'))
        exe = max((pathlib.Path(p) for p in glob.glob(str(NEXT / 'target/debug/deps/managed_session-*')) if os.access(p, os.X_OK) and not p.endswith('.d')), key=lambda p: p.stat().st_mtime)
        self.host = subprocess.Popen([str(exe), '--ignored', '--exact', 'daemon_fixture', '--nocapture'], env={**os.environ, 'KEEPLINE_FIXTURE_RUNTIME': str(self.root)}, stdout=subprocess.DEVNULL, stderr=open(self.root / 'host.log', 'w'))
        deadline = time.time() + 10
        while not (self.root / 'workbench.sock').exists():
            assert time.time() < deadline; time.sleep(.05)
        self.edit({'AddSpace': {'id': 's', 'name': 'GPU bench', 'cwd': str(self.root)}})
    def rpc(self, obj):
        with socket.socket(socket.AF_UNIX) as s:
            s.settimeout(10); s.connect(str(self.root / 'workbench.sock')); send(s, {'Hello': {'major': 3}}); receive(s); send(s, obj); r = receive(s)
            if isinstance(r, dict) and 'Error' in r: raise RuntimeError(r)
            return r
    def edit(self, obj):
        rev = self.rpc('Workspace')['Workspace']['revision']
        return self.rpc({'Edit': {'revision': rev, 'edit': obj}})
    def launch(self, id, script):
        self.rpc({'Launch': {'id': id, 'space': 's', 'argv': ['/bin/sh', '-c', script], 'cwd': str(self.root), 'cols': 100, 'rows': 30}})
    def tab(self, id, tree):
        self.edit({'PutTab': {'tab': {'id': id, 'space': 's', 'title': id, 'default_task': None, 'closed': False, 'layout': tree}}})
    def close(self):
        self.host.terminate(); self.host.wait(5)
        # The app takes the terminals over into its own detached host, which outlives both
        # the app and the fixture and keeps every workload loop running.
        pattern = f'keeplined serve --runtime {self.root}( |$)'
        for sig, wait in ((15, 50), (9, 20)):
            pids = [int(p) for p in subprocess.run(['pgrep', '-f', pattern], capture_output=True, text=True).stdout.split()]
            if not pids: return
            for p in pids:
                try: os.kill(p, sig)
                except ProcessLookupError: pass
            for _ in range(wait):
                if not subprocess.run(['pgrep', '-f', pattern], capture_output=True).stdout: return
                time.sleep(.1)
        raise RuntimeError(f'host for {self.root} did not exit')

def leaf(i): return {'Leaf': i}
def split(a, b, axis='Right'): return {'Split': {'axis': axis, 'ratio': 5000, 'first': a, 'second': b}}

SCENES = {
    # (visible tab tree builder, hidden tab tree, scripts)
    'idle':    (lambda: leaf('v0'), None, {'v0': IDLE}),
    'hidden4': (lambda: leaf('v0'), split(split(leaf('h0'), leaf('h1'), 'Down'), split(leaf('h2'), leaf('h3'), 'Down')), {'v0': IDLE, 'h0': SPIN, 'h1': SPIN, 'h2': SPIN, 'h3': SPIN}),
    'vis1':    (lambda: leaf('v0'), None, {'v0': SPIN}),
    'vis4':    (lambda: split(split(leaf('v0'), leaf('v1'), 'Down'), split(leaf('v2'), leaf('v3'), 'Down')), None, {f'v{i}': SPIN for i in range(4)}),
    'flood':   (lambda: leaf('v0'), None, {'v0': FLOOD}),
    # One shell to type into beside three spinners; --interact also sweeps the pointer.
    'mixed':   (lambda: split(split(leaf('v0'), leaf('v1'), 'Down'), split(leaf('v2'), leaf('v3'), 'Down')), None, {'v0': IDLE, 'v1': SPIN, 'v2': SPIN, 'v3': SPIN}),
}

def run_keepline(scene):
    shown, hidden, scripts = SCENES[scene]
    fx = Fixture()
    home = fx.root / 'home'; home.mkdir()
    app = None
    try:
        for id, script in scripts.items(): fx.launch(id, script)
        fx.tab('shown', shown())
        if hidden: fx.tab('hidden', hidden)
        app = subprocess.Popen([args.app, '--runtime', str(fx.root)], env={**os.environ, 'HOME': str(home), 'SHELL': '/bin/sh', 'KEEPLINE_FRAME_STATS': '1'}, stdout=subprocess.DEVNULL, stderr=open(fx.root / 'app.log', 'w'))
        time.sleep(3); front(app.pid)
        if args.size:
            w, h = args.size.split('x')
            subprocess.run(['osascript', '-e', f'tell application "System Events" to tell (first process whose unix id is {app.pid}) to set size of window 1 to {{{w}, {h}}}'], capture_output=True)
            time.sleep(1)
        subprocess.run(['screencapture', '-x', '-o', str(HERE / f'out/{scene}{args.label}.png')])
        log = fx.root / 'app.log'
        mark = log.stat().st_size
        actor = None
        if args.interact:
            actor = subprocess.Popen(['/bin/sh', '-c', f'''swift {HERE}/mouse.swift 40 40 1470 910; for i in 1 2 3 4 5 6; do osascript -e 'tell application "System Events" to keystroke "echo damage $i"' -e 'tell application "System Events" to key code 36'; sleep 0.3; done; swift {HERE}/mouse.swift 40 40 400 910'''])
        r = measure([app.pid], args.secs)[app.pid]
        if actor: actor.wait()
        stats = [l for l in log.read_text(errors='replace')[mark:].splitlines() if l.startswith('frame-stats')]
        if stats: r['frames'] = stats[len(stats) // 2].removeprefix('frame-stats ')
        checks = [l for l in log.read_text(errors='replace').splitlines() if l.startswith('damage-verify')]
        if checks:
            bad = [l for l in checks if 'bad_pixels=0 ' not in l]
            r['verify'] = f'{len(checks)} frames checked, {len(bad)} differ' + (f': {bad[:3]}' if bad else '')
        return r
    finally:
        if app: app.terminate(); app.wait(5)
        fx.close()

def run_ghostty(scene):
    script = {'vis1': SPIN, 'flood': FLOOD, 'idle': IDLE}.get(scene)
    if not script: return None
    path = pathlib.Path(tempfile.mkdtemp(prefix='kl-gpu-gh-', dir='/tmp')) / 'w.sh'
    path.write_text(script)
    def pids(): return set(map(int, subprocess.run(['pgrep', '-x', 'ghostty'], capture_output=True, text=True).stdout.split()))
    before = pids()
    # A separate instance (-n): the CLI binary alone forwards to the running Ghostty.
    subprocess.run(['open', '-na', '/Applications/Ghostty.app', '--args', '--config-default-files=false', '--window-save-state=never', '--quit-after-last-window-closed=true', '--font-family=JetBrains Mono', '--font-size=13', f'--command=/bin/sh {path}'])
    time.sleep(4)
    new = pids() - before
    assert len(new) == 1, f'ghostty instance not found: {new}'
    pid = new.pop()
    front(pid)
    try:
        subprocess.run(['screencapture', '-x', '-o', str(HERE / f'out/ghostty-{scene}{args.label}.png')])
        return measure([pid], args.secs)[pid]
    finally:
        os.kill(pid, 15)

(HERE / 'out').mkdir(exist_ok=True)
results = {}
for scene in args.scenes.split(','):
    results[f'keepline/{scene}'] = run_keepline(scene)
    print(f'keepline {scene:8} {results[f"keepline/{scene}"]}', flush=True)
    if args.ghostty:
        g = run_ghostty(scene)
        if g:
            results[f'ghostty/{scene}'] = g
            print(f'ghostty  {scene:8} {g}', flush=True)
(HERE / f'out/results{args.label}.json').write_text(json.dumps(results, indent=2))
