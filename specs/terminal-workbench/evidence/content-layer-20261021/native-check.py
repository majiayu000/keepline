"""Private test host: real-log offline cards + simulated Agent PTYs, no real CLI/model calls.
Source CLI trees are never watched or changed by this fixture. Native actions use Computer Use.
"""
import ctypes,json,os,pathlib,socket,struct,subprocess,tempfile,time,plistlib,shutil,sqlite3
BASE=pathlib.Path(__file__).resolve().parents[4]
OUT=pathlib.Path(__file__).resolve().parent
root=pathlib.Path(tempfile.mkdtemp(prefix='kl-content-ui-',dir='/tmp'));os.chmod(root,0o700)
audit=json.loads(pathlib.Path('/tmp/kl-content-audit-reviewed.json').read_text())
# Pick 40 real-session projections. Skip health and empty metadata-only sessions in visual evidence.
skip_ids=['b79467ef-2992-474f-b1b8-919cad1c2d78']  # Exclude a private topic from visual evidence.
selected=[c for c in audit['cards'] if c['cli_id'] not in skip_ids][:40]
src=sqlite3.connect('/tmp/kl-content-audit-reviewed-20261021/workbench.sqlite');dst=sqlite3.connect(root/'workbench.sqlite');src.backup(dst);src.close()
paths=[c['path'] for c in selected]
for table in ['log_files','session_content','session_records']:
    dst.execute(f"DELETE FROM {table} WHERE path NOT IN ({','.join('?' for _ in paths)})",paths)
dst.commit();dst.close()
host=subprocess.Popen([str(BASE/'next/target/debug/examples/content_fixture'),str(root)],stdout=subprocess.DEVNULL,stderr=open(root/'host.log','w'))
def read(s,n):
    data=b''
    while len(data)<n:
        chunk=s.recv(n-len(data))
        if not chunk:raise EOFError()
        data+=chunk
    return data
def receive(s):return json.loads(read(s,struct.unpack('>I',read(s,4))[0])[1:])
def send(s,obj):
    data=b'\x01'+json.dumps(obj).encode();s.sendall(struct.pack('>I',len(data))+data)
def rpc(obj):
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(15);s.connect(str(root/'workbench.sock'));send(s,{'Hello':{'major':3}});receive(s);send(s,obj);r=receive(s)
        if isinstance(r,dict) and 'Error' in r:raise RuntimeError(r)
        return r
def w():return rpc('Workspace')['Workspace']
def edit(e):return rpc({'Edit':{'revision':w()['revision'],'edit':e}})
def leaf(i):return {'Leaf':f'p{i}'}
def split(a,b,axis='Right'):return {'Split':{'axis':axis,'ratio':5000,'first':a,'second':b}}
class Usage(ctypes.Structure):
    _fields_=[('uuid',ctypes.c_ubyte*16)]+[(x,ctypes.c_uint64) for x in ['user','system','idle','interrupt','pageins','wired','resident','footprint','start','exit','child_user','child_system','child_idle','child_interrupt','child_pageins','child_elapsed','read','write']]
class Timebase(ctypes.Structure):_fields_=[('numer',ctypes.c_uint32),('denom',ctypes.c_uint32)]
lib=ctypes.CDLL('/usr/lib/libproc.dylib');tb=Timebase();assert ctypes.CDLL('/usr/lib/libSystem.B.dylib').mach_timebase_info(ctypes.byref(tb))==0
def usage(pid):
    u=Usage();assert lib.proc_pid_rusage(pid,2,ctypes.byref(u))==0
    return {'cpu_ns':(u.user+u.system)*tb.numer/tb.denom,'footprint':u.footprint,'resident':u.resident,'read':u.read,'write':u.write}
app=None
try:
    deadline=time.monotonic()+15
    while not (root/'workbench.sock').exists():
        assert time.monotonic()<deadline;time.sleep(.1)
    while len(w()['content'])<40:
        assert time.monotonic()<deadline;time.sleep(.1)
    edit({'AddSpace':{'id':'s','name':'内容层验收 · 隔离环境','cwd':str(root)}})
    os.symlink('/bin/sh',root/'codex')
    for i in range(8):
        log=root/'logs'/f'managed-{i}.jsonl'
        # These are deliberately marked fixtures, not claimed as real CLI acceptance.
        content=[{'type':'session_meta','payload':{'id':f'fixture-{i}','cwd':str(root)}},
                 {'type':'event_msg','payload':{'type':'task_started','turn_id':'fixture-turn'}},
                 {'type':'event_msg','payload':{'type':'user_message','message':f'验证第 {i+1} 个托管画面的增量内容，不启动真实 Agent。'}},
                 {'type':'response_item','payload':{'type':'function_call','call_id':'test','name':'functions.exec_command','arguments':json.dumps({'cmd':'cargo test --lib'})}},
                 {'type':'response_item','payload':{'type':'function_call_output','call_id':'test','output':'Process exited with code 0\ntest result: ok. 12 passed; 0 failed'}},
                 {'type':'event_msg','payload':{'type':'task_complete','last_agent_message':'模拟回合已完成，请核对证据。'}}]
        for row in content:row['timestamp']=int(time.time())
        staged=root/f'seed-{i}';staged.write_text(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in content))
        script=f"exec 3>'{log}'; /bin/cat '{staged}' >&3; printf '内容层验收 · 模拟 Agent {i+1}\\n真实日志卡片在鸟瞰；本画面没有调用模型。\\n'; while IFS= read -r line; do printf '%s\\n' \"$line\"; done"
        rpc({'Launch':{'id':f'p{i}','space':'s','argv':[str(root/'codex'),'-c',script],'cwd':str(root),'cols':80,'rows':24}})
    deadline=time.monotonic()+15
    while sum(s['current'] for s in w()['agent_sessions'])<8:
        assert time.monotonic()<deadline;time.sleep(.1)
    tree=split(split(split(leaf(0),leaf(1),'Down'),split(leaf(2),leaf(3),'Down')),split(split(leaf(4),leaf(5),'Down'),split(leaf(6),leaf(7),'Down')))
    edit({'PutTab':{'tab':{'id':'eight','space':'s','title':'八画面 · 资源验收','default_task':None,'closed':False,'layout':tree}}})
    bundle=root/'Keepline Content Check.app';mac=bundle/'Contents/MacOS';mac.mkdir(parents=True)
    shutil.copy2(BASE/'next/target/debug/keepline-app',mac/'keepline-app')
    (bundle/'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleName':'Keepline Content Check','CFBundleDisplayName':'Keepline Content Check','CFBundleExecutable':'keepline-app','CFBundleIdentifier':'local.keepline.content-check','CFBundlePackageType':'APPL','CFBundleVersion':'1','NSHighResolutionCapable':True}))
    home=root/'home';home.mkdir()
    app=subprocess.Popen([str(mac/'keepline-app'),'--runtime',str(root)],env={**os.environ,'HOME':str(home),'GPUI_PAUSE_IDLE_DISPLAY_LINK':'1','SHELL':'/bin/sh'},stdout=subprocess.DEVNULL,stderr=open(root/'app.log','w'))
    state={'app':app.pid,'host':host.pid,'bundle':str(bundle),'runtime':str(root)}
    (OUT/'native-client.json').write_text(json.dumps(state,indent=2));print(json.dumps(state),flush=True)
    # Driver waits are test orchestration only, not a product timer.
    while not (root/'measure-ready').exists():time.sleep(.5)
    time.sleep(3)
    pids={'client':app.pid,'daemon':host.pid,**{t['id']:t['pid'] for t in w()['terminals']}}
    metrics0=rpc('RecognitionStatus')['Recognition']['metrics'];before={k:usage(pid) for k,pid in pids.items()};start=time.monotonic();time.sleep(30)
    elapsed=time.monotonic()-start;after={k:usage(pid) for k,pid in pids.items()};metrics1=rpc('RecognitionStatus')['Recognition']['metrics']
    report={'seconds':elapsed,'external':sum(s['terminal'] is None for s in w()['agent_sessions']),'managed':sum(s['current'] for s in w()['agent_sessions']),'metrics_before':metrics0,'metrics_after':metrics1,'processes':{k:{'cpu_percent':(after[k]['cpu_ns']-before[k]['cpu_ns'])/1e9/elapsed*100,'footprint_MiB':after[k]['footprint']/2**20,'disk_read':after[k]['read']-before[k]['read'],'disk_write':after[k]['write']-before[k]['write']} for k in pids}}
    (OUT/'idle-48.json').write_text(json.dumps(report,indent=2));print('MEASURED',flush=True)
    while not (root/'finish').exists():time.sleep(.5)
finally:
    if app is not None:app.terminate();app.wait(timeout=5)
    try:
        for t in w()['terminals']:rpc({'Stop':{'id':t['id']}})
    finally:host.terminate();host.wait(timeout=5)
