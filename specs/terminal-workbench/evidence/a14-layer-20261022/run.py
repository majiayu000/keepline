"""Bounded, unattended A14 foreground run: `run.py <opt-bin-dir> <base-bin-dir> <deadline-epoch>`.
Waits for an unlocked screen and 20 s without keyboard/mouse input, then for each case builds a
fresh isolated fixture, samples with the unchanged R5 measure.py under frontmost.swift (one
activation at start, any focus loss voids the sample, at most 3 attempts), and tears the fixture
down by its explicit PIDs only. Every attempt and receipt is kept."""
import json, os, pathlib, signal, subprocess, sys, time
out = pathlib.Path(__file__).resolve().parent; ev = out.parent
opt, base, deadline = sys.argv[1], sys.argv[2], float(sys.argv[3])
guard, lock = '/tmp/kl14-idle-guard', '/tmp/kl14-lock'
cases = [('keepline-opt-one', 'k', opt, 1), ('ghostty-one', 'g', None, 1),
         ('keepline-opt-four', 'k', opt, 4), ('ghostty-four', 'g', None, 4),
         ('keepline-base-one', 'k', base, 1), ('keepline-base-four', 'k', base, 4)]
log = []
def note(**kw):
    kw['utc'] = time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()); log.append(kw); print(json.dumps(kw), flush=True)
    (out / 'run-log.json').write_text(json.dumps(log, indent=1, ensure_ascii=False) + '\n')
def idle():
    o = subprocess.run(['ioreg', '-c', 'IOHIDSystem'], capture_output=True, text=True).stdout
    return int(next(l for l in o.splitlines() if 'HIDIdleTime' in l).split()[-1]) / 1e9
def ready():
    # Unlocked sessions have no CGSSessionScreenIsLocked key, printed as locked=false.
    return b'locked=1' not in subprocess.run([lock], capture_output=True).stdout and idle() >= 20
def wait_ready():
    while not ready():
        if time.time() > deadline: return False
        time.sleep(15)
    return True
def teardown(spec):
    for p in [spec['pids']['client'], spec['pids'].get('daemon'), *spec['producers']]:
        if p:
            try: os.kill(p, signal.SIGTERM)
            except ProcessLookupError: pass
for name, kind, bins, panes in cases:
    done = False
    attempt = 0
    while not done and attempt < 3:
        if not wait_ready():
            note(case=name, event='deadline', detail='no unlocked, input-idle window before deadline'); sys.exit(0)
        cmd = ['/usr/bin/python3', str(out / 'fixture.py'), bins, str(panes), name] if kind == 'k' else \
              ['/usr/bin/python3', str(out / 'ghostty-fixture.py'), str(panes), name]
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode:
            note(case=name, event='fixture-failed', stderr=r.stderr[-600:]); break
        spec = json.loads((out / f'{name}-spec.json').read_text())
        time.sleep(8)  # first frames, atlas warm-up
        attempt += 1
        prefix = out / f'{name}-{attempt}'
        g = subprocess.run([guard, str(spec['pids']['client']), str(prefix), '/usr/bin/python3',
                            str(ev / 'r5-20261021/measure.py'), str(out / f'{name}-spec.json'), str(prefix) + '.json'],
                           capture_output=True, text=True, timeout=400)
        focus = pathlib.Path(str(prefix) + '-focus-1.json')
        valid = g.returncode == 0 and focus.exists() and json.loads(focus.read_text())['continuous_frontmost']
        note(case=name, attempt=attempt, exit=g.returncode, valid=valid, root=spec['root'])
        if g.returncode == 2: attempt -= 1  # idle gate expired: not a sample attempt
        if valid:
            wid = subprocess.run(['/tmp/kl14-wins', str(spec['pids']['client'])], capture_output=True, text=True).stdout.split()
            if wid: subprocess.run(['screencapture', '-x', '-o', f'-l{wid[0]}', str(out / f'{name}.png')])
            done = True
        teardown(spec)
        time.sleep(3)
note(event='complete')
