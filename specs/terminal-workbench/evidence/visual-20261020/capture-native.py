"""Native GPUI renderer capture with isolated, synthetic PTY/JSONL data. No user histories/hooks."""
import glob, json, os, pathlib, shlex, signal, socket, struct, subprocess, tempfile, time
BASE = pathlib.Path(__file__).resolve().parents[4]
OUT = pathlib.Path(os.environ.get('KEEPLINE_VISUAL_OUT', pathlib.Path(__file__).resolve().parent))
OUT.mkdir(parents=True, exist_ok=True)
root = pathlib.Path(tempfile.mkdtemp(prefix='kl-visual-', dir='/tmp'))
exe = max((pathlib.Path(p) for p in glob.glob(str(BASE/'next/target/debug/deps/managed_session-*')) if os.access(p, os.X_OK) and not p.endswith('.d')), key=lambda p:p.stat().st_mtime)
host = subprocess.Popen([str(exe), '--ignored', '--exact', 'daemon_fixture', '--nocapture'],env={**os.environ,'KEEPLINE_FIXTURE_RUNTIME':str(root)},stdout=subprocess.DEVNULL,stderr=open(root/'host.log','w'))
def rec(s):
    def read(n):
        b=b''
        while len(b)<n:
            part=s.recv(n-len(b))
            if not part: raise EOFError()
            b+=part
        return b
    b=read(struct.unpack('>I',read(4))[0]); return json.loads(b[1:])
def send(s,v):
    b=b'\x01'+json.dumps(v).encode();s.sendall(struct.pack('>I',len(b))+b)
def rpc(v):
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(10);s.connect(str(root/'workbench.sock'));send(s,{'Hello':{'major':1}});rec(s);send(s,v);r=rec(s)
        if isinstance(r,dict) and 'Error' in r:raise RuntimeError(r)
        return r

def workspace():return rpc('Workspace')['Workspace']
def edit(v):return rpc({'Edit':{'revision':workspace()['revision'],'edit':v}})
try:
    deadline=time.time()+10
    while not (root/'workbench.sock').exists():
        assert time.time()<deadline;time.sleep(.05)
    for i,name in enumerate(['lib','sys','模型上线 work','keepline','codex-mall']):
        edit({'AddSpace':{'id':str(i),'name':name,'cwd':str(root)}})
    for i,title in enumerate(['CLI 问题反馈','给 CLI 加离线模式','清理旧的扫描轮询代码']):
        edit({'AddTask':{'id':f'task{i}','space':'0','title':title,'description':'隔离视觉验收数据；不会启动真实 Agent','acceptance':[{'id':f'c{n}','description':text,'evidence':'fixture-test' if n==0 else None} for n,text in enumerate(['命令预填正确','身份关联正确','人工核对'])],'agent':'Codex'}})
    (root/'codex').symlink_to('/bin/sh')
    logs=root/'logs/codex';logs.mkdir(parents=True,exist_ok=True)
    for i in range(2):
        title=['实现反馈子命令','审查反馈接口改动'][i]
        transcript=logs/f'{i}.jsonl'
        transcript.write_text(json.dumps({'type':'session_meta','payload':{'id':f'visual-{i}','cwd':str(root)}})+'\n')
        lines=['› '+title,'','这是隔离的视觉验收终端，未调用真实 Agent。','','• 检查 CLI 命令预填','  codex --no-daemon','• 检查 Terminal / AgentSession 关联','  同一个终端可以保留多段对话','','当前展示的是实际 PTY 输出。','等待你输入…']
        script=f'echo $$ > {root}/pid{i}; exec 3<{transcript}; printf "%s\\n" '+ ' '.join(shlex.quote(t) for t in lines)+'; read -r line'
        command=f'({root}/codex -c {shlex.quote(script)}; read -r line) 2>/dev/null'
        rpc({'Launch':{'id':f'p{i}','space':'0','argv':['/bin/sh','-c',command],'cwd':str(root),'cols':90,'rows':30}})
    edit({'PutTab':{'tab':{'id':'feedback','space':'0','title':'CLI 问题反馈','default_task':'task0','closed':False},'terminals':['p0','p1']}})
    for name in ['tes','webs','首屏 1.5 秒以内','上线前安全检查']:
        edit({'PutTab':{'tab':{'id':name,'space':'0','title':name,'default_task':None,'closed':False},'terminals':[]}})
    for i in range(2):
        deadline=time.time()+5
        while not (root/f'pid{i}').exists():assert time.time()<deadline;time.sleep(.05)
        rpc({'Hook':{'runtime':'Codex','pid':int((root/f'pid{i}').read_text()),'cli_session_id':f'visual-{i}','transcript':str(logs/f'{i}.jsonl'),'ended':False}})
        session=next(s for s in workspace()['agent_sessions'] if s['cli_session_id']==f'visual-{i}')
        edit({'AssignTask':{'id':session['id'],'task':'task0'}})
    # Keep the second shell alive after its Agent exits, exercising the lifecycle correction.
    os.kill(int((root/'pid1').read_text()),signal.SIGTERM)
    if os.environ.get('KEEPLINE_VISUAL_RESTART') == '1':
        host.terminate(); host.wait(timeout=5)
        host = subprocess.Popen([str(exe), '--ignored', '--exact', 'daemon_fixture', '--nocapture'],env={**os.environ,'KEEPLINE_FIXTURE_RUNTIME':str(root)},stdout=subprocess.DEVNULL,stderr=open(root/'restart.log','w'))
        deadline = time.time()+10
        while True:
            try:
                terminals = workspace()['terminals']
                if len(terminals) == 2 and all(t['state'] == 'Interrupted' for t in terminals): break
            except (OSError, EOFError):
                pass
            assert time.time() < deadline, 'restarted host did not interrupt old terminals'
            time.sleep(.05)
    app = subprocess.run([str(BASE/'next/target/debug/keepline-app'),'--runtime',str(root),'--visual-evidence',str(OUT)],env={**os.environ,'GPUI_PAUSE_IDLE_DISPLAY_LINK':'1'},stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30)
    (OUT/'native-render.log').write_bytes(app.stdout+app.stderr)
    assert app.returncode==0,app.stderr.decode()
    print('Native renderer images saved:', OUT)
finally:
    try:
        for t in workspace()['terminals']:
            if t['state'] != 'Interrupted': rpc({'Stop':{'id':t['id']}})
    finally:
        host.terminate();host.wait(timeout=5)
        import shutil
        shutil.rmtree(root)
