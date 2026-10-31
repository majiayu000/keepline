"""Pane decorations in the real app: a private HOME with the shipped pane-facts plugin and a
git repo with one uncommitted file. The native harness (KEEPLINE_DECORATION_EVIDENCE) opens
a shell there, checks the title and hover lines, leaves the repo and comes back after a
second file, saving what the renderer drew. No model, credentials or user data.

Run after `cargo build -p keepline-app -p keeplined --features keepline-app/visual-test`:
    python3 specs/plugins/evidence/decorations-e2e.py <repo root> <output dir>"""
import pathlib, runpy, shutil, subprocess, sys, tempfile, time

REPO = pathlib.Path(sys.argv[1]).resolve()
OUT = pathlib.Path(sys.argv[2]).resolve()
m = runpy.run_path(str(REPO / 'specs/terminal-workbench/evidence/r4-20261021/native-fixture.py'))
rpc, edit = m['rpc'], m['edit']

root = pathlib.Path(tempfile.mkdtemp(prefix='kldeco-', dir='/tmp')); root.chmod(0o700)
home = root / 'home'; (home / '.codex/sessions').mkdir(parents=True); (home / '.claude/projects').mkdir(parents=True)
shutil.copytree(REPO / 'next/plugins/pane-facts', home / '.config/keepline/plugins/pane-facts')
project = root / 'project'; project.mkdir()
git = lambda *a: subprocess.run(['git', '-C', str(project), *a], check=True, capture_output=True)
git('init', '-q'); git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-q', '--allow-empty', '-m', '初始提交')
(project / 'first.txt').write_text('')
env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'HOME': str(home), 'CODEX_HOME': str(home / '.codex'),
       'CLAUDE_CONFIG_DIR': str(home / '.claude'), 'LANG': 'en_US.UTF-8'}
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
        env={**env, 'GPUI_PAUSE_IDLE_DISPLAY_LINK': '1', 'KEEPLINE_DECORATION_EVIDENCE': str(project)},
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=90)
    log = result.stdout.decode(errors='replace')
    (OUT / 'native-render.log').write_text(log)
    print('\n'.join(l for l in log.splitlines() if 'repo' in l or '/tmp' in l or 'back' in l or 'PASS' in l or 'panicked' in l))
    assert result.returncode == 0, log[-3000:]
finally:
    try:
        for t in rpc(root, 'Workspace')['Workspace']['terminals']: rpc(root, {'Stop': {'id': t['id']}})
    except Exception as e: print('stop', e)
    host.terminate(); host.wait(5)
    shutil.rmtree(root, ignore_errors=True)
