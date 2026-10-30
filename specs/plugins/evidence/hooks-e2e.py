"""Hooks end to end: a private HOME with a hooks plugin, one synthetic Codex session that
waits, then finishes. No model, no credentials, no network."""
import json, os, pathlib, runpy, shlex, subprocess, sys, tempfile, time

REPO = pathlib.Path(sys.argv[1])
m = runpy.run_path(str(REPO / 'specs/terminal-workbench/evidence/r4-20261021/native-fixture.py'))
rpc, edit = m['rpc'], m['edit']

root = pathlib.Path(tempfile.mkdtemp(prefix='klhk-', dir='/tmp')); root.chmod(0o700)
home = root / 'home'; logs = home / '.codex/sessions'; logs.mkdir(parents=True)
(home / '.claude/projects').mkdir(parents=True)
project = root / 'project'; project.mkdir()
out = root / 'hook-out.jsonl'
plugin = home / '.config/keepline/plugins/record'; plugin.mkdir(parents=True)
(plugin / 'plugin.json').write_text(json.dumps({'api': 1, 'id': 'record', 'name': '记录', 'provides': ['hooks']}))
(plugin / 'hooks.json').write_text(json.dumps({'api': 1, 'hooks': [
    {'id': 'record', 'on': ['started', 'waiting', 'error', 'done'],
     'command': f'(cat; echo) >> {shlex.quote(str(out))}'},
    {'id': 'slow', 'on': ['waiting'], 'command': 'sleep 30', 'timeout_ms': 300},
]}))
env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'HOME': str(home), 'CODEX_HOME': str(home / '.codex'),
       'CLAUDE_CONFIG_DIR': str(home / '.claude'), 'LANG': 'en_US.UTF-8'}
host = subprocess.Popen([str(REPO / 'next/target/debug/keeplined'), 'serve', '--runtime', str(root / 'runtime')],
                        env=env, stdout=subprocess.DEVNULL, stderr=open(root / 'host.log', 'w'), start_new_session=True)
try:
    deadline = time.monotonic() + 30
    while not (root / 'runtime/workbench.sock').exists():
        assert host.poll() is None and time.monotonic() < deadline, (root / 'host.log').read_text()
        time.sleep(.05)
    edit(root, {'AddSpace': {'id': 's0', 'name': 'proj', 'cwd': str(project)}})
    log = logs / 'hk-1.jsonl'
    now = int(time.time())
    def rec(t, p): return json.dumps({'timestamp': now, 'type': t, 'payload': p}, ensure_ascii=False) + '\n'
    log.write_text(rec('session_meta', {'id': 'hk-1', 'cwd': str(project), 'source': 'cli', 'originator': 'codex-tui'})
                   + rec('event_msg', {'type': 'user_message', 'message': '修复登录超时'})
                   + rec('event_msg', {'type': 'task_started', 'turn_id': 't1'}))
    codex = root / 'codex'; codex.symlink_to('/bin/sh')
    r = rpc(root, {'Launch': {'id': 'p0', 'space': 's0', 'argv': [str(codex), '-c', 'exec 3<' + shlex.quote(str(log)) + '; sleep 600'],
                              'cwd': str(project), 'cols': 80, 'rows': 24}})
    assert 'Error' not in r, r
    w = rpc(root, 'Workspace')['Workspace']
    t = next(t for t in w['terminals'] if t['id'] == 'p0')
    r = rpc(root, {'Hook': {'runtime': 'Codex', 'pid': t['pid'], 'cli_session_id': 'hk-1', 'transcript': str(log), 'ended': False, 'event': None}})
    assert 'Error' not in r, r
    def events(n, timeout=15):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            lines = [json.loads(l) for l in out.read_text().splitlines() if l.strip()] if out.exists() else []
            if len(lines) >= n: return lines
            time.sleep(.1)
        return [json.loads(l) for l in out.read_text().splitlines() if l.strip()] if out.exists() else []
    print('after bind:', [e['event'] for e in events(1)])
    with log.open('a') as f:
        f.write(rec('event_msg', {'type': 'request_user_input', 'message': '是否允许推送分支？'}))
    print('after wait:', [(e['event'], e['detail']) for e in events(2)])
    with log.open('a') as f:
        f.write(rec('event_msg', {'type': 'user_message', 'message': '可以推送'})
                + rec('event_msg', {'type': 'task_started', 'turn_id': 't2'}))
    print('after answer:', [e['event'] for e in events(3)])
    with log.open('a') as f:
        f.write(rec('event_msg', {'type': 'task_complete', 'last_agent_message': '已完成，登录超时改成 30 秒。'}))
    got = events(4)
    print('after done:', [(e['event'], e['title'], e['detail'], e['space']) for e in got])
    time.sleep(2)
    w = rpc(root, 'Workspace')['Workspace']
    for sid, c in w['content'].items(): print('content', sid, c['turn'], repr(c['turn_key']), c['completed_claim'], [ (x['id'],x['outcome'],x['current']) for x in w['agent_sessions']])
    print('terminals', [(t['id'], t['state'], t.get('exit_code')) for t in w['terminals']])
    print('total lines', len(events(99, 0.1)))
    print('host log:', [l for l in (root / 'host.log').read_text().splitlines() if 'hook' in l])
finally:
    try:
        for t in rpc(root, 'Workspace')['Workspace']['terminals']: rpc(root, {'Stop': {'id': t['id']}})
    except Exception as e: print('stop', e)
    host.terminate(); host.wait(5)
    print('root', root)
