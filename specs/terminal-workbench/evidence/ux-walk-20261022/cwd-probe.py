"""Launch an isolated Keepline bundle through LaunchServices (as Finder/Dock do) and read the
default space's cwd. Usage: cwd_probe.py <repo> <out.json>"""
import json, os, pathlib, plistlib, runpy, shlex, shutil, signal, subprocess, sys, tempfile, time
repo = pathlib.Path(sys.argv[1]); ev = repo / 'specs/terminal-workbench/evidence'
m = runpy.run_path(str(ev / 'r4-20261021/native-fixture.py')); rpc = m['rpc']
parent = pathlib.Path.home() / 'Library/Caches/KeeplineAcceptance'; parent.mkdir(exist_ok=True)
r = pathlib.Path(tempfile.mkdtemp(prefix='cwd-', dir=parent)); h = pathlib.Path(tempfile.mkdtemp(prefix='kl-cwd-home-', dir='/tmp'))
(h / '.codex/sessions').mkdir(parents=True); (h / '.claude/projects').mkdir(parents=True)
b = r / 'Keepline CWD Probe.app'; mac = b / 'Contents/MacOS'; mac.mkdir(parents=True)
shutil.copy2(repo / 'next/target/debug/keepline-app', mac / 'keepline-app'); shutil.copy2(repo / 'next/target/release/keeplined', mac / 'keeplined')
env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LANG': 'en_US.UTF-8', 'HOME': str(h), 'CODEX_HOME': str(h / '.codex'), 'CLAUDE_CONFIG_DIR': str(h / '.claude')}
f = mac / 'launch'
f.write_text('#!/bin/sh\nexec /usr/bin/env -i ' + ' '.join(k + '=' + shlex.quote(v) for k, v in env.items()) + ' ' + shlex.quote(str(mac / 'keepline-app')) + ' --runtime ' + shlex.quote(str(r / 'runtime')) + ' >>' + shlex.quote(str(r / 'app.log')) + ' 2>&1\n'); f.chmod(0o755)
(b / 'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleName': 'Keepline CWD Probe', 'CFBundleIdentifier': 'local.keepline.cwd-probe', 'CFBundleExecutable': 'launch', 'CFBundlePackageType': 'APPL', 'CFBundleVersion': '1', 'NSHighResolutionCapable': True}))
subprocess.run(['codesign', '--force', '--sign', '-', str(b)], check=True, capture_output=True)
subprocess.run(['open', '-n', str(b)], check=True)
end = time.monotonic() + 30; spaces = []
while time.monotonic() < end:
    try:
        spaces = rpc(r, 'Workspace')['Workspace']['spaces']
        if spaces: break
    except Exception: pass
    time.sleep(.3)
time.sleep(2)
w = rpc(r, 'Workspace')['Workspace']
out = {'launched_via': 'open -n (LaunchServices)', 'spaces': [{k: s[k] for k in ('id', 'name', 'cwd')} for s in w['spaces']],
       'terminals': [{'id': t['id'], 'cwd': t.get('cwd')} for t in w['terminals']], 'home': '<isolated HOME>'}
print(json.dumps(out, ensure_ascii=False))
pathlib.Path(sys.argv[2]).write_text(json.dumps(out, ensure_ascii=False, indent=2) + '\n')
for p in subprocess.run(['pgrep', '-f', str(mac)], capture_output=True, text=True).stdout.split(): os.kill(int(p), signal.SIGTERM)
time.sleep(1); shutil.rmtree(r, ignore_errors=True); shutil.rmtree(h, ignore_errors=True)
