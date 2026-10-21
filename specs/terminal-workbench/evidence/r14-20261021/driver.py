"""R14 real Codex acceptance: official CLI, default shared-daemon mode, isolated HOME/CODEX_HOME.
Input goes through keeplined's own PTY protocol like typed keys. Never prints auth material."""
import json, os, pathlib, re, shutil, signal, socket, struct, subprocess, sys, tempfile, threading, time

WT = pathlib.Path('/Users/lifcc/Desktop/code/AI/tools/keepline-codex-daemon')
KEEPLINED = WT / 'next/target/debug/keeplined'
OUT = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path(tempfile.mkdtemp())
MODE = sys.argv[1] if len(sys.argv) > 1 else 'dry'
CONTROL, SNAPSHOT, READY, OUTPUT, ACK = 1, 2, 3, 4, 7
LOG = []

def log(*a):
    line = time.strftime('%H:%M:%S ') + ' '.join(str(x) for x in a)
    print(line, flush=True); LOG.append(line)

def read(s, n):
    b = b''
    while len(b) < n:
        c = s.recv(n - len(b))
        if not c: raise EOFError()
        b += c
    return b
def frame(s):
    n = struct.unpack('>I', read(s, 4))[0]; b = read(s, n); return b[0], b[1:]
def send(s, obj, kind=CONTROL):
    b = bytes([kind]) + json.dumps(obj).encode(); s.sendall(struct.pack('>I', len(b)) + b)

class Env:
    def __init__(self):
        self.root = pathlib.Path(tempfile.mkdtemp(prefix='kl-r14-', dir='/tmp')).resolve()
        self.root.chmod(0o700)
        self.home = self.root / 'home'; self.codex = self.home / '.codex'
        self.project = self.root / 'project'; self.runtime = self.root / 'runtime'
        (self.codex / 'sessions').mkdir(parents=True); self.project.mkdir()
        (self.home / '.claude/projects').mkdir(parents=True)
        self.home.chmod(0o700)
        shutil.copyfile(pathlib.Path.home() / '.codex/auth.json', self.codex / 'auth.json')
        (self.codex / 'auth.json').chmod(0o600)
        # Same model as the user's default; low effort and "reply ok" keep each round small.
        (self.codex / 'config.toml').write_text(
            'model = "gpt-6.1-sol"\nmodel_reasoning_effort = "low"\ncheck_for_update_on_startup = false\n'
            f'[projects.{json.dumps(str(self.project))}]\ntrust_level = "trusted"\n')
        self.env = {**os.environ, 'HOME': str(self.home), 'CODEX_HOME': str(self.codex),
                    'CLAUDE_CONFIG_DIR': str(self.home / '.claude')}
        self.host = subprocess.Popen([str(KEEPLINED), 'serve', '--runtime', str(self.runtime)],
                                     cwd=self.project, env=self.env, stdout=subprocess.DEVNULL,
                                     stderr=open(self.root / 'host.log', 'w'))
        until = time.monotonic() + 20
        while not (self.runtime / 'workbench.sock').exists():
            assert self.host.poll() is None and time.monotonic() < until; time.sleep(.05)
        self.edit({'AddSpace': {'id': 's', 'name': 'R14', 'cwd': str(self.project)}})
        log('root', self.root)

    def rpc(self, obj):
        with socket.socket(socket.AF_UNIX) as s:
            s.settimeout(10); s.connect(str(self.runtime / 'workbench.sock'))
            send(s, {'Hello': {'major': 3}}); frame(s); send(s, obj)
            while True:
                k, b = frame(s)
                if k == CONTROL: return json.loads(b)
    def edit(self, e):
        w = self.workspace(); r = self.rpc({'Edit': {'revision': w['revision'], 'edit': e}})
        assert 'Error' not in r, r
    def workspace(self): return self.rpc('Workspace')['Workspace']
    def hints(self): return self.rpc('RecognitionStatus')['Recognition']['terminal_hints']
    def current(self, pane):
        for s in self.workspace()['agent_sessions']:
            if s['terminal'] == pane and s['current']: return s
    def stop(self):
        try:
            for t in self.workspace()['terminals']: self.rpc({'Stop': {'id': t['id']}})
        except Exception as e: log('stop terminals', e)
        self.host.terminate()
        try: self.host.wait(timeout=10)
        except Exception: self.host.kill()
        # The isolated shared daemon lives under this CODEX_HOME only.
        ps = subprocess.run(['ps', '-axo', 'pid=,command='], capture_output=True, text=True).stdout
        for line in ps.splitlines():
            if str(self.codex) in line:
                pid = int(line.split()[0]); log('stop isolated daemon', pid)
                try: os.kill(pid, signal.SIGTERM)
                except ProcessLookupError: pass
        (self.codex / 'auth.json').unlink(missing_ok=True)

ANSI = re.compile(r'\x1b\][^\x07\x1b]*(\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]|\x1b[()][0-9A-Za-z]|\x1b[=>78DEHM]')

class Pane:
    def __init__(self, env, id):
        self.env, self.id, self.raw = env, id, b''
        r = env.rpc({'Launch': {'id': id, 'space': 's', 'argv': ['/bin/zsh', '-f'],
                                'cwd': str(env.project), 'cols': 120, 'rows': 40}})
        assert 'Error' not in r, r
        self.s = socket.socket(socket.AF_UNIX); self.s.connect(str(env.runtime / 'workbench.sock'))
        send(self.s, {'Hello': {'major': 3}}); frame(self.s)
        send(self.s, {'Attach': {'id': id, 'takeover': True}})
        self.acks = 0; self.lock = threading.Lock()
        threading.Thread(target=self.pump, daemon=True).start()
    def pump(self):
        try:
            while True:
                k, b = frame(self.s)
                with self.lock:
                    if k == OUTPUT: self.raw = (self.raw + b)[-400_000:]
                    elif k == ACK: self.acks += 1
        except Exception: pass
    def text(self):
        with self.lock: return ANSI.sub('', self.raw.decode('utf-8', 'replace'))
    def type(self, s):
        send(self.s, {'Input': {'bytes': list(s.encode())}}); time.sleep(.05)
    def wait(self, pattern, secs=60):
        until = time.monotonic() + secs
        while time.monotonic() < until:
            if re.search(pattern, self.text()[-6000:]): return True
            time.sleep(.2)
        return False

def wait_for(what, ok, secs=90):
    until = time.monotonic() + secs
    while time.monotonic() < until:
        if ok(): log('ok:', what); return True
        time.sleep(.3)
    log('TIMEOUT:', what); return False

def submit(p, prompt):
    p.type(prompt); time.sleep(.6); p.type('\r')

def transcript_has(session, prompt):
    try: return prompt in pathlib.Path(session['transcript']).read_text()
    except Exception: return False

def main():
    OUT.mkdir(parents=True, exist_ok=True); env = Env(); result = {'root': str(env.root), 'scenarios': {}}
    panes = {}
    try:
        for id in ['p0', 'p1', 'p2', 'p3', 'p4', 'p5']:
            panes[id] = Pane(env, id)
        time.sleep(1.5)
        for p in panes.values(): p.type('codex\r')
        ready = {id: p.wait(r'›|for shortcuts|context left', 60) for id, p in panes.items()}
        log('tui ready', ready); time.sleep(3)
        log('hints before prompts', env.hints())
        if MODE == 'dry':
            for id, p in panes.items(): log(f'--- {id} tail ---\n' + p.text()[-1500:])
            return
        Q = '只回复 ok，不要调用任何工具。'
        if MODE == 'new':
            a = '场景三旧会话：' + Q
            submit(panes['p0'], a)
            wait_for('S3 first bind', lambda: env.current('p0') is not None)
            old = env.current('p0')
            assert panes['p0'].wait(r'Worked for', 120), 'first turn never finished'
            time.sleep(2)
            panes['p0'].type('/new'); time.sleep(.6); panes['p0'].type('\r'); time.sleep(3)
            e = '场景三新会话：' + Q
            submit(panes['p0'], e)
            ok = wait_for('S3 takeover', lambda: (env.current('p0') or {}).get('cli_session_id') not in (None, old['cli_session_id']))
            n = env.current('p0')
            olds = [s for s in env.workspace()['agent_sessions'] if s['cli_session_id'] == old['cli_session_id']]
            result['scenarios']['new'] = {
                'old': old['cli_session_id'], 'new': n and n['cli_session_id'],
                'old_correct': transcript_has(old, a), 'took_over': ok,
                'correct': bool(ok and transcript_has(n, e) and not transcript_has(n, a)),
                'old_not_current': bool(olds and not olds[0]['current']),
                'idle_panes_unbound': all(env.current(x) is None for x in ['p1', 'p2', 'p3', 'p4', 'p5'])}
            result['user_prompts'] = 2
            for id, p in panes.items(): (OUT / f'{id}.txt').write_text(p.text()[-20000:])
            return
        # S1 same cwd, two panes, typed seconds apart.
        a, b = '场景一甲：' + Q, '场景一乙：' + Q
        submit(panes['p0'], a); time.sleep(4); submit(panes['p1'], b)
        ok = wait_for('S1 bind', lambda: all(env.current(x) for x in ['p0', 'p1']))
        s0, s1 = env.current('p0'), env.current('p1')
        result['scenarios']['same_cwd'] = {
            'bound': ok, 'p0': s0 and s0['cli_session_id'], 'p1': s1 and s1['cli_session_id'],
            'correct': bool(ok and transcript_has(s0, a) and transcript_has(s1, b))}
        # S2 Enter on both panes back to back.
        c, d = '场景二甲：' + Q, '场景二乙：' + Q
        panes['p2'].type(c); panes['p3'].type(d); time.sleep(.8)
        panes['p2'].type('\r'); panes['p3'].type('\r')
        ok = wait_for('S2 bind', lambda: all(env.current(x) for x in ['p2', 'p3']))
        s2, s3 = env.current('p2'), env.current('p3')
        result['scenarios']['same_moment'] = {
            'bound': ok, 'correct': bool(ok and transcript_has(s2, c) and transcript_has(s3, d))}
        # S3 /new in p0 hands the pane to the newer thread.
        old = env.current('p0')
        panes['p0'].wait(r'ok', 60); time.sleep(3)
        panes['p0'].type('/new'); time.sleep(.6); panes['p0'].type('\r'); time.sleep(3)
        e = '场景三新会话：' + Q
        submit(panes['p0'], e)
        ok = wait_for('S3 takeover', lambda: (env.current('p0') or {}).get('cli_session_id') not in (None, old and old['cli_session_id']))
        n = env.current('p0')
        olds = [s for s in env.workspace()['agent_sessions'] if old and s['cli_session_id'] == old['cli_session_id']]
        result['scenarios']['new'] = {
            'took_over': ok, 'correct': bool(ok and transcript_has(n, e)),
            'old_not_current': bool(olds and not olds[0]['current'])}
        # S4 identical prompts, then distinct follow-ups.
        same = '场景四相同提示：' + Q
        panes['p4'].type(same); panes['p5'].type(same); time.sleep(.8)
        panes['p4'].type('\r'); panes['p5'].type('\r')
        time.sleep(25)
        hint = env.hints()
        unbound = env.current('p4') is None and env.current('p5') is None
        log('S4 identical unbound:', unbound, 'hints', hint)
        f, g = '场景四甲追问：' + Q, '场景四乙追问：' + Q
        submit(panes['p4'], f); time.sleep(4); submit(panes['p5'], g)
        ok = wait_for('S4 follow-up bind', lambda: all(env.current(x) for x in ['p4', 'p5']))
        s4, s5 = env.current('p4'), env.current('p5')
        result['scenarios']['identical'] = {
            'unbound_while_identical': unbound, 'hints_while_identical': hint,
            'bound_after_follow_up': ok,
            'correct': bool(ok and transcript_has(s4, f) and transcript_has(s5, g))}
        # Exit clears current: Ctrl-C twice in p1.
        panes['p1'].type('\x03'); time.sleep(.5); panes['p1'].type('\x03')
        result['exit_clears_current'] = wait_for('p1 exit', lambda: env.current('p1') is None, 20)
        result['user_prompts'] = 9
        status = env.rpc('RecognitionStatus')['Recognition']['metrics']
        result['metrics'] = status
        for id, p in panes.items(): (OUT / f'{id}.txt').write_text(p.text()[-20000:])
    finally:
        log('result', json.dumps(result, ensure_ascii=False))
        OUT.mkdir(parents=True, exist_ok=True)
        (OUT / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
        (OUT / 'driver.log').write_text('\n'.join(LOG) + '\n')
        env.stop()

if __name__ == '__main__':
    main()
