"""Fresh isolated Keepline resource fixture: `fixture.py <release-dir> <1|4> <label>`.
Same loads as R8 (1 pane: load-one.py 80x20) and R5 (4 panes: load.py 60x20, 2x2 split).
Prints a measure.py spec with explicit test PIDs; never touches the user's backend."""
import json, os, pathlib, plistlib, runpy, shlex, shutil, subprocess, sys, tempfile, time
out = pathlib.Path(__file__).resolve().parent
ev = out.parent
m = runpy.run_path(str(ev / 'r4-20261021/native-fixture.py')); rpc = m['rpc']; edit = m['edit']
bins, panes, label = pathlib.Path(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
r = pathlib.Path(tempfile.mkdtemp(prefix=f'kl14-{label}-', dir='/tmp')); h = r / 'home'
(h / '.codex/sessions').mkdir(parents=True); (h / '.claude/projects').mkdir(parents=True)
b = r / f'Keepline A14 {label}.app'; mac = b / 'Contents/MacOS'; mac.mkdir(parents=True)
for n in ('keepline-app', 'keeplined'): shutil.copy2(bins / n, mac / n)
env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LANG': 'en_US.UTF-8', 'HOME': str(h),
       'CODEX_HOME': str(h / '.codex'), 'CLAUDE_CONFIG_DIR': str(h / '.claude')}
host = subprocess.Popen([str(mac / 'keeplined'), 'serve', '--runtime', str(r / 'runtime')], env=env,
                        stdout=subprocess.DEVNULL, stderr=open(r / 'host.log', 'w'), start_new_session=True)
end = time.monotonic() + 15
while not (r / 'runtime/workbench.sock').exists():
    assert time.monotonic() < end; time.sleep(.1)
edit(r, {'AddSpace': {'id': 's', 'name': '资源对照', 'cwd': str(r)}})
load = ev / ('r8-20261021/load-one.py' if panes == 1 else 'r5-20261021/load.py')
for i in range(panes):
    rpc(r, {'Launch': {'id': f'load{i+1}', 'space': 's', 'argv': ['/usr/bin/python3', str(load)],
                       'cwd': str(r), 'cols': 160 if panes == 1 else 80, 'rows': 48 if panes == 1 else 24}})
leaf = lambda i: {'Leaf': f'load{i}'}
split = lambda a, c, axis: {'Split': {'axis': axis, 'ratio': 5000, 'first': a, 'second': c}}
layout = leaf(1) if panes == 1 else split(split(leaf(1), leaf(2), 'Down'), split(leaf(3), leaf(4), 'Down'), 'Right')
edit(r, {'PutTab': {'tab': {'id': 'load', 'space': 's', 'title': '资源对照', 'default_task': None,
                            'closed': False, 'layout': layout}}})
f = mac / 'launch'
f.write_text('#!/bin/sh\nexec /usr/bin/env -i ' + ' '.join(k + '=' + shlex.quote(v) for k, v in env.items()) + ' '
             + shlex.quote(str(mac / 'keepline-app')) + ' --runtime ' + shlex.quote(str(r / 'runtime'))
             + ' >>' + shlex.quote(str(r / 'app.log')) + ' 2>&1\n'); f.chmod(0o755)
(b / 'Contents/Info.plist').write_bytes(plistlib.dumps({
    'CFBundleName': f'Keepline A14 {label}', 'CFBundleDisplayName': f'Keepline A14 {label}',
    'CFBundleIdentifier': f'local.keepline.a14.{label}', 'CFBundleExecutable': 'launch',
    'CFBundlePackageType': 'APPL', 'CFBundleVersion': '1', 'NSHighResolutionCapable': True}))
subprocess.run(['codesign', '--force', '--sign', '-', str(b)], check=True, capture_output=True)
subprocess.run(['open', '-n', str(b)], check=True)
client = None
end = time.monotonic() + 20
while client is None:
    assert time.monotonic() < end
    time.sleep(.3)
    ps = subprocess.run(['pgrep', '-f', str(mac / 'keepline-app')], capture_output=True, text=True).stdout.split()
    client = int(ps[0]) if ps else None
producers = [int(p) for p in subprocess.run(['pgrep', '-P', str(host.pid)], capture_output=True, text=True).stdout.split()]
for p in producers:
    cmd = subprocess.run(['ps', '-o', 'command=', '-p', str(p)], capture_output=True, text=True).stdout
    assert str(load) in cmd, cmd
assert len(producers) == panes, producers
pids = {'client': client, 'daemon': host.pid, **{f'producer{i+1}': p for i, p in enumerate(sorted(producers))}}
spec = {'label': label, 'root': str(r), 'binaries': str(bins), 'pids': pids, 'producers': sorted(producers)}
(out / f'{label}-spec.json').write_text(json.dumps(spec, indent=2) + '\n'); print(json.dumps(spec))
