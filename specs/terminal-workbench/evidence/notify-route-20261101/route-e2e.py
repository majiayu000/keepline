"""A notification clicked before its session loaded, in the real app: a private HOME with one
synthetic Codex log. The native harness (KEEPLINE_ROUTE_EVIDENCE) takes that session out of
the window's copy, routes to it as a notification click does, and checks it opens once the
next workspace arrives, and that a page the user opens first wins. No model or user data.

Run after `cargo build -p keepline-app -p keeplined --features keepline-app/visual-test`:
    python3 specs/terminal-workbench/evidence/notify-route-20261101/route-e2e.py <repo root> <output dir>"""
import json, pathlib, runpy, shutil, subprocess, sys, tempfile, time

REPO = pathlib.Path(sys.argv[1]).resolve()
OUT = pathlib.Path(sys.argv[2]).resolve()
m = runpy.run_path(str(REPO / 'specs/terminal-workbench/evidence/r4-20261021/native-fixture.py'))
rpc, edit = m['rpc'], m['edit']

root = pathlib.Path(tempfile.mkdtemp(prefix='klroute-', dir='/tmp')); root.chmod(0o700)
home = root / 'home'; logs = home / '.codex/sessions'; logs.mkdir(parents=True); (home / '.claude/projects').mkdir(parents=True)
project = root / 'project'; project.mkdir()
now = int(time.time())
records = [(now - 600, 'session_meta', {'id': 'route-1', 'cwd': str(project), 'source': 'cli', 'originator': 'codex-tui', 'thread_source': 'user'}),
           (now - 600, 'event_msg', {'type': 'user_message', 'message': '通知跳转测试'}),
           (now - 60, 'event_msg', {'type': 'request_user_input', 'message': '要继续吗？'})]
(logs / 'route-1.jsonl').write_text(''.join(json.dumps({'timestamp': at, 'type': t, 'payload': p}, ensure_ascii=False) + '\n' for at, t, p in records))
env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'HOME': str(home), 'CODEX_HOME': str(home / '.codex'),
       'CLAUDE_CONFIG_DIR': str(home / '.claude'), 'LANG': 'en_US.UTF-8', 'SHELL': '/bin/zsh'}
host = subprocess.Popen([str(REPO / 'next/target/debug/keeplined'), 'serve', '--runtime', str(root / 'runtime')],
                        env=env, stdout=subprocess.DEVNULL, stderr=open(root / 'host.log', 'w'), start_new_session=True)
try:
    deadline = time.monotonic() + 30
    while not (root / 'runtime/workbench.sock').exists():
        assert host.poll() is None and time.monotonic() < deadline, (root / 'host.log').read_text()
        time.sleep(.05)
    edit(root, {'AddSpace': {'id': 's0', 'name': 'project', 'cwd': str(project)}})
    OUT.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(
        [str(REPO / 'next/target/debug/keepline-app'), '--runtime', str(root / 'runtime'), '--visual-evidence', str(OUT)],
        env={**env, 'GPUI_PAUSE_IDLE_DISPLAY_LINK': '1', 'KEEPLINE_ROUTE_EVIDENCE': '1'},
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=90)
    log = result.stdout.decode(errors='replace')
    (OUT / 'native-render.log').write_text(log)
    print('\n'.join(l for l in log.splitlines() if 'clicked' in l or 'loaded' in l or 'moved' in l or 'PASS' in l or 'panicked' in l))
    assert result.returncode == 0, log[-3000:]
finally:
    try:
        for t in rpc(root, 'Workspace')['Workspace']['terminals']: rpc(root, {'Stop': {'id': t['id']}})
    except Exception as e: print('stop', e)
    host.terminate(); host.wait(5)
    shutil.rmtree(root, ignore_errors=True)
