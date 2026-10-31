"""Todo refresh visual run: the visual-20261021 fixture with todo-refresh sample data.

Private HOME and runtime, synthetic Codex logs, no model calls, credentials or user data.
The linked sessions' logs carry real-format test, patch and failed-test records, so the
evidence a criterion holds comes from the log parser, not from the fixture.

  python3 fixture.py prepare      # start an isolated host and seed it
  python3 fixture.py capture NAME # render KEEPLINE_PAGE_LIST into NAME/
  python3 fixture.py stop
"""
import json, os, pathlib, runpy, shlex, subprocess, tempfile, time, sys
OUT = pathlib.Path(__file__).resolve().parent
BASE = OUT.parents[3]
m = runpy.run_path(str(OUT.parent / 'r5-20261021/fixture.py')); rpc = m['rpc']; edit = m['edit']
STATE = pathlib.Path('/tmp/kl-todo-refresh.json')
names = ['工作空间', 'sys', '模型上线 work', 'keepline', 'codex-mall']
titles = ['修标签命名', '查 tab_name 从哪来', '核对标签测试', '审查反馈接口改动', '实现缓存结果补传', '核查指纹变更', '评估视频队列方案', '整理项目说明']
TASKS = [
    ('标签命名依旧有问题', '新开的终端标签还是叫 zsh。启动 Agent 时应该用待办标题命名标签；Agent 退出后保留这个名字，不要退回进程名。',
     ['新开 Codex 标签显示待办标题，不是 zsh', 'Agent 退出后标签名保留', 'cargo test -p keepline-app 全部通过']),
    ('健身', '', []),
    ('睡觉', '', []),
]


def call(cid, n, cmd, output, at):
    args = {'cmd': cmd} if cmd else {'input': output[1]}
    return [(at, 'response_item', {'type': 'function_call', 'name': 'exec_command' if cmd else 'apply_patch', 'call_id': f'{cid}-{n}', 'arguments': json.dumps(args)}),
            (at, 'response_item', {'type': 'function_call_output', 'call_id': f'{cid}-{n}', 'output': output if cmd else output[0]})]


def patch(path):
    return ('Success. Updated the following files:\nM ' + path, f'*** Begin Patch\n*** Update File: {path}\n@@\n-old\n+new\n*** End Patch')


if sys.argv[1] == 'prepare':
    parent = pathlib.Path.home() / 'Library/Caches/KeeplineAcceptance'; parent.mkdir(exist_ok=True)
    root = pathlib.Path(tempfile.mkdtemp(prefix='todo-', dir=parent)); home = pathlib.Path(tempfile.mkdtemp(prefix='kltodo-home-', dir='/tmp'))
    logs = home / '.codex/sessions'; logs.mkdir(parents=True); (home / '.claude/projects').mkdir(parents=True)
    now = int(time.time())
    for n in range(40):
        project = root / 'projects' / names[n // 8]; project.mkdir(parents=True, exist_ok=True)
        stamp = now - (6 * 86400 if n % 8 == 5 else 60 * (n + 1)); cid = f'visual-{n}'
        records = [(stamp - 3600, 'session_meta', {'id': cid, 'cwd': str(project), 'source': 'cli', 'originator': 'codex-tui', 'thread_source': 'user'}),
                   (stamp - 3600, 'event_msg', {'type': 'user_message', 'message': titles[n % 8] + ('' if n < 8 else ' · ' + names[n // 8])}),
                   (stamp - 3600, 'event_msg', {'type': 'task_started', 'turn_id': cid + '-turn'})]
        if n == 0:
            records += call(cid, 1, None, patch('next/crates/keepline-app/src/workbench/tabs.rs'), stamp - 1500)
            records += call(cid, 2, 'cargo test -p keepline-app tab_title', 'Process exited with code 0\ntest result: ok. 4 passed; 0 failed', stamp - 1260)
            records += call(cid, 3, 'cargo test -p keepline-app', 'Process exited with code 101\ntest result: FAILED. 211 passed; 2 failed', stamp - 720)
        if n == 1:
            records += call(cid, 1, None, patch('next/crates/keepline-core/src/model.rs'), stamp - 400)
            records += call(cid, 2, 'cargo test -p keepline-app tab_title_survives_exit', 'Process exited with code 0\ntest result: ok. 1 passed; 0 failed', stamp - 300)
            records += call(cid, 3, 'cargo test -p keepline-app', 'Process exited with code 0\ntest result: ok. 213 passed; 0 failed', stamp - 200)
        group = n % 8
        if group == 0: records.append((stamp, 'event_msg', {'type': 'request_user_input', 'message': '标签名在退出后要不要保留？我倾向保留，等你确认。'}))
        elif group == 1: records.append((stamp, 'event_msg', {'type': 'task_complete', 'last_agent_message': '已找到 tab_name 的来源。'}))
        elif group == 2: records.extend([(stamp, 'response_item', {'type': 'function_call', 'name': 'exec_command', 'call_id': 'test', 'arguments': json.dumps({'cmd': 'bun test'})}), (stamp, 'response_item', {'type': 'function_call_output', 'call_id': 'test', 'output': 'Process exited with code 0\n31 pass 0 fail'}), (stamp, 'event_msg', {'type': 'task_complete', 'last_agent_message': '已完成，请核对。'})])
        elif group != 4: records.append((stamp, 'event_msg', {'type': 'turn_aborted'}))
        (logs / (cid + '.jsonl')).write_text(''.join(json.dumps({'timestamp': at, 'type': t, 'payload': p}, ensure_ascii=False) + '\n' for at, t, p in records))
    env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'HOME': str(home), 'CODEX_HOME': str(home / '.codex'), 'CLAUDE_CONFIG_DIR': str(home / '.claude'), 'LANG': 'en_US.UTF-8'}
    host = subprocess.Popen([str(BASE / 'next/target/release/keeplined'), 'serve', '--runtime', str(root / 'runtime')], env=env, stdout=subprocess.DEVNULL, stderr=open(root / 'host.log', 'w'), start_new_session=True)
    deadline = time.monotonic() + 30
    while not (root / 'runtime/workbench.sock').exists():
        assert host.poll() is None and time.monotonic() < deadline; time.sleep(.05)
    for n, name in enumerate(names): edit(root, {'AddSpace': {'id': f's{n}', 'name': name, 'cwd': str(root / 'projects' / name)}})
    # A space whose folder is gone, as after deleting a worktree.
    edit(root, {'AddSpace': {'id': 's5', 'name': '新空间', 'cwd': str(root / 'projects/gone')}})
    for n, (title, description, criteria) in enumerate(TASKS):
        edit(root, {'AddTask': {'id': f'task{n}', 'space': 's0', 'title': title, 'description': description, 'acceptance': [{'id': f't{n}-c{i}', 'description': text, 'evidence': []} for i, text in enumerate(criteria)], 'agent': 'Codex'}})
    (root / 'codex').symlink_to('/bin/sh')
    for n in range(3):
        lines = ['› ' + titles[n], '', '• 已读取项目上下文', '• 修改标签命名', '', '此处为隔离视觉测试输出，未调用模型。']
        script = 'exec 3<' + shlex.quote(str(logs / f'visual-{n}.jsonl')) + '; printf "%s\\n" ' + ' '.join(shlex.quote(s) for s in lines) + '; while IFS= read -r line; do printf "%s\\n" "$line"; done'
        r = rpc(root, {'Launch': {'id': f'p{n}', 'space': 's0', 'argv': [str(root / 'codex'), '-c', script], 'cwd': str(root / 'projects/工作空间'), 'cols': 90, 'rows': 35}}); assert 'Error' not in r, r
    rpc(root, {'Launch': {'id': 'pstart', 'space': 's0', 'argv': ['/bin/zsh', '-f'], 'cwd': str(root / 'projects/工作空间'), 'cols': 100, 'rows': 35}})
    def leaf(p): return {'Leaf': p}
    def split(a, b, axis): return {'Split': {'axis': axis, 'ratio': 5000, 'first': a, 'second': b}}
    for id, title, task, layout in [('feedback', '标签命名依旧有问题', 'task0', split(leaf('p0'), split(leaf('p1'), leaf('p2'), 'Down'), 'Right')), ('start', '健身', 'task1', leaf('pstart'))]:
        edit(root, {'PutTab': {'tab': {'id': id, 'space': 's0', 'title': title, 'default_task': task, 'closed': False, 'layout': layout}}})
    while True:
        w = rpc(root, 'Workspace')['Workspace']
        if len(w['agent_sessions']) >= 40: break
        assert time.monotonic() < deadline, 'index timeout'; time.sleep(.1)
    for n in range(3):
        t = next(t for t in w['terminals'] if t['id'] == f'p{n}')
        r = rpc(root, {'Hook': {'runtime': 'Codex', 'pid': t['pid'], 'cli_session_id': f'visual-{n}', 'transcript': str(logs / f'visual-{n}.jsonl'), 'ended': False, 'event': None}}); assert 'Error' not in r, r
        session = next(s for s in rpc(root, 'Workspace')['Workspace']['agent_sessions'] if s['cli_session_id'] == f'visual-{n}')
        edit(root, {'AssignTask': {'id': session['id'], 'task': 'task0'}})
    state = {'root': str(root), 'home': str(home), 'host': host.pid}; STATE.write_text(json.dumps(state)); print(state)
else:
    s = json.loads(STATE.read_text()); root = pathlib.Path(s['root']); home = pathlib.Path(s['home'])
    if sys.argv[1] == 'capture':
        directory = OUT / sys.argv[2]; directory.mkdir(exist_ok=True)
        env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'HOME': str(home), 'CODEX_HOME': str(home / '.codex'), 'CLAUDE_CONFIG_DIR': str(home / '.claude'), 'LANG': 'en_US.UTF-8', 'GPUI_PAUSE_IDLE_DISPLAY_LINK': '1', 'KEEPLINE_PAGE_EVIDENCE': '1',
               'KEEPLINE_PAGE_LIST': os.environ.get('KEEPLINE_PAGE_LIST', 'TodoNew,TodoNewBare,TodoNewChecks,TodoDetail,TodoDetailAttach,TodoDetailFull,TodoDetailDone')}
        result = subprocess.run([str(BASE / 'next/target/debug/keepline-app'), '--runtime', str(root / 'runtime'), '--visual-evidence', str(directory)], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120)
        (directory / 'native-render.log').write_bytes(result.stdout); assert result.returncode == 0, result.stdout.decode()[-4000:]
        print('native captures', sorted(p.name for p in directory.glob('*-native.png')))
    elif sys.argv[1] == 'readback':
        w = rpc(root, 'Workspace')['Workspace']
        print(json.dumps([t for t in w['tasks'] if t['id'] == 'task0'], ensure_ascii=False, indent=1)[:3000])
        print(json.dumps({k: v['evidence'] for k, v in w['content'].items()}, ensure_ascii=False)[:3000])
    elif sys.argv[1] == 'stop':
        for t in rpc(root, 'Workspace')['Workspace']['terminals']: rpc(root, {'Stop': {'id': t['id']}})
        os.kill(s['host'], 15)
