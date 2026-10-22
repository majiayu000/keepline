"""A04 native check without a model: a pane runs `claude --session-id <uuid>` (a /bin/sh
stand-in), its JSONL ends on a pending Write tool call with no permission record, and after
binding the screen shows the R9 Claude Code 2.1.302 approval verbatim. Isolated HOME/runtime."""
import json, os, pathlib, runpy, shlex, subprocess, sys, tempfile, time, uuid
OUT = pathlib.Path(__file__).resolve().parent; BASE = OUT.parents[3]
m = runpy.run_path(str(OUT.parent / 'r4-20261021/native-fixture.py')); rpc = m['rpc']; edit = m['edit']
# Transcribed from evidence/r9-20261021 (real Claude Code 2.1.302 approval screen).
R9 = ['● Write(approval-proof.txt)', '', '─' * 40, ' Create file', ' approval-proof.txt', '╌' * 40, '  1 ok',
      '╌' * 40, ' Do you want to create approval-proof.txt?', ' ❯ 1. Yes',
      '   2. Yes, and switch to accept edits (auto-approve file edits and common',
      '      file commands) for this session (shift+tab)', '   3. No', '', ' Esc to cancel · Tab to amend']
parent = pathlib.Path.home() / 'Library/Caches/KeeplineAcceptance'; parent.mkdir(exist_ok=True)
# Not under /tmp: Claude sessions in temporary directories are hidden as automation.
root = pathlib.Path(tempfile.mkdtemp(prefix='a04-', dir=parent))
# HOME stays short: the client's single-instance socket lives under it (SUN_LEN).
home = pathlib.Path(tempfile.mkdtemp(prefix='kl-a04-home-', dir='/tmp')); project = root / 'project'
project.mkdir(parents=True); (home / '.codex/sessions').mkdir(parents=True)
sid = str(uuid.uuid4())
logs = home / '.claude/projects' / str(project).replace('/', '-').replace('.', '-'); logs.mkdir(parents=True)
log = logs / f'{sid}.jsonl'; now = time.strftime('%Y-%m-%dT%H:%M:%S.000Z', time.gmtime())
rows = [{'type': 'user', 'sessionId': sid, 'uuid': 'u0', 'cwd': str(project), 'entrypoint': 'cli', 'userType': 'external',
         'timestamp': now, 'message': {'role': 'user', 'content': '写一个 approval-proof.txt，内容 ok'}},
        {'type': 'assistant', 'sessionId': sid, 'uuid': 'a0', 'parentUuid': 'u0', 'cwd': str(project), 'timestamp': now,
         'message': {'role': 'assistant', 'model': 'claude', 'stop_reason': 'tool_use', 'content': [
             {'type': 'tool_use', 'id': 'toolu_1', 'name': 'Write',
              'input': {'file_path': str(project / 'approval-proof.txt'), 'content': 'ok'}}]}}]
seed = root / 'seed.jsonl'; seed.write_text(''.join(json.dumps(r, ensure_ascii=False) + '\n' for r in rows))
env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'HOME': str(home), 'CODEX_HOME': str(home / '.codex'),
       'CLAUDE_CONFIG_DIR': str(home / '.claude'), 'LANG': 'en_US.UTF-8'}
host = subprocess.Popen([str(BASE / 'next/target/release/keeplined'), 'serve', '--runtime', str(root / 'runtime')], env=env,
                        stdout=subprocess.DEVNULL, stderr=open(root / 'host.log', 'w'), start_new_session=True)
deadline = time.monotonic() + 30
while not (root / 'runtime/workbench.sock').exists():
    assert host.poll() is None and time.monotonic() < deadline; time.sleep(.05)
edit(root, {'AddSpace': {'id': 's0', 'name': 'approval', 'cwd': str(project)}})
(root / 'claude').symlink_to('/bin/sh')
script = (f'cat {shlex.quote(str(seed))} > {shlex.quote(str(log))}; '
          f'while [ ! -e {shlex.quote(str(root / "show"))} ]; do sleep 0.2; done; '
          'printf "%s\\n" ' + ' '.join(shlex.quote(s) for s in R9) + '; while IFS= read -r line; do :; done')
r = rpc(root, {'Launch': {'id': 'p1', 'space': 's0', 'argv': [str(root / 'claude'), '-c', script, 'claude', '--session-id', sid],
                          'cwd': str(project), 'cols': 100, 'rows': 30}}); assert 'Error' not in r, r
edit(root, {'PutTab': {'tab': {'id': 'feedback', 'space': 's0', 'title': '审批复验', 'default_task': None, 'closed': False, 'layout': {'Leaf': 'p1'}}}})
receipt = {'session_id': sid}
while (s := rpc(root, 'Workspace')['Workspace']) and not any(a['cli_session_id'] == sid for a in s['agent_sessions']):
    assert time.monotonic() < deadline + 30, 'explicit --session-id did not bind'; time.sleep(.1)
a = next(a for a in s['agent_sessions'] if a['cli_session_id'] == sid)
receipt['bound'] = {'turn': s['content'][a['id']]['turn'], 'waiting': s['content'][a['id']]['waiting']}
(root / 'show').touch(); shown = time.monotonic()
while True:
    s = rpc(root, 'Workspace')['Workspace']; c = s['content'][a['id']]
    if c['turn'] == 'Waiting': break
    assert time.monotonic() < shown + 15, 'screen approval was not recognized'; time.sleep(.05)
receipt['recognized'] = {'turn': c['turn'], 'waiting': c['waiting'], 'seconds_after_screen': round(time.monotonic() - shown, 2)}
receipt['tabs'] = [{k: t[k] for k in ('id', 'layout', 'closed')} for t in s['tabs']]
receipt['terminals'] = [{k: t.get(k) for k in ('id', 'space', 'state')} for t in s['terminals']]
print(json.dumps(receipt, ensure_ascii=False))
if len(sys.argv) > 1:
    d = OUT / 'native'; d.mkdir(exist_ok=True)
    cenv = {**env, 'GPUI_PAUSE_IDLE_DISPLAY_LINK': '1', 'KEEPLINE_PAGE_EVIDENCE': '1', 'KEEPLINE_PAGE_LIST': 'ScreenWait,Overview'}
    res = subprocess.run([str(BASE / 'next/target/debug/keepline-app'), '--runtime', str(root / 'runtime'), '--visual-evidence', str(d)],
                         env=cenv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=90)
    (d / 'native-render.log').write_bytes(res.stdout); receipt['native_exit'] = res.returncode
(OUT / 'receipt.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n')
for t in rpc(root, 'Workspace')['Workspace']['terminals']: rpc(root, {'Stop': {'id': t['id']}})
os.kill(host.pid, 15)
