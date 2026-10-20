"""Release resource soak; synthetic 40 external + 8 visible managed sessions, no model calls.
Only private runtime/config; sampling belongs to the harness, never the product.
"""
import ctypes,json,os,pathlib,socket,struct,subprocess,tempfile,time,plistlib,shutil
BASE=pathlib.Path(__file__).resolve().parents[4]
OUT=pathlib.Path(os.environ.get('KL_EVIDENCE_DIR',str(pathlib.Path(__file__).resolve().parent)))
RELEASE=pathlib.Path(os.environ.get('KL_RELEASE_DIR',str(BASE/'next/target/release')))
root=pathlib.Path(tempfile.mkdtemp(prefix='kl-soak-',dir='/tmp'));os.chmod(root,0o700)
(root/'logs').mkdir()
def records(identity):
    return [{'timestamp':int(time.time()),'type':'session_meta','payload':{'id':identity,'cwd':str(BASE),'source':'cli','originator':'codex-tui','thread_source':'user'}},
            {'timestamp':int(time.time()),'type':'event_msg','payload':{'type':'user_message','message':'资源测试：保持空闲，不调用模型。'}},
            {'timestamp':int(time.time()),'type':'event_msg','payload':{'type':'task_complete','last_agent_message':'模拟结束，等待核对。'}}]
def encoded(identity):return ''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in records(identity))
for i in range(40):(root/'logs'/f'external-{i}.jsonl').write_text(encoded(f'external-{i}'))
host=subprocess.Popen([str(RELEASE/'examples/content_fixture'),str(root)],stdout=subprocess.DEVNULL,stderr=open(root/'host.log','w'))
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
    deadline=time.monotonic()+60
    while not (root/'workbench.sock').exists():
        assert time.monotonic()<deadline;time.sleep(.1)
    while len(w()['content'])<40:
        assert time.monotonic()<deadline;time.sleep(.1)
    edit({'AddSpace':{'id':'s','name':'资源验收 · 40 外部 + 8 托管','cwd':str(BASE)}})
    os.symlink('/bin/sh',root/'codex')
    for i in range(8):
        log=root/'logs'/f'managed-{i}.jsonl';staged=root/f'seed-{i}';staged.write_text(encoded(f'managed-{i}'))
        script=f"exec 3>'{log}'; /bin/cat '{staged}' >&3; printf 'Keepline release resource fixture {i+1}\\n'; while IFS= read -r line; do printf '%s\\n' \"$line\"; done"
        rpc({'Launch':{'id':f'p{i}','space':'s','argv':[str(root/'codex'),'-c',script],'cwd':str(BASE),'cols':80,'rows':24}})
    deadline=time.monotonic()+60
    while sum(s['current'] for s in w()['agent_sessions'])<8:
        assert time.monotonic()<deadline;time.sleep(.1)
    tree=split(split(split(leaf(0),leaf(1),'Down'),split(leaf(2),leaf(3),'Down')),split(split(leaf(4),leaf(5),'Down'),split(leaf(6),leaf(7),'Down')))
    edit({'PutTab':{'tab':{'id':'eight','space':'s','title':'8 画面 · 两小时空闲','default_task':None,'closed':False,'layout':tree}}})
    bundle=root/'Keepline Soak Test.app';mac=bundle/'Contents/MacOS';mac.mkdir(parents=True)
    shutil.copy2(RELEASE/'keepline-app',mac/'keepline-app')
    (bundle/'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleName':'Keepline Soak Test','CFBundleDisplayName':'Keepline Soak Test','CFBundleExecutable':'keepline-app','CFBundleIdentifier':'local.keepline.soak-test','CFBundlePackageType':'APPL','CFBundleVersion':'1','NSHighResolutionCapable':True}))
    home=root/'home';home.mkdir()
    app=subprocess.Popen([str(mac/'keepline-app'),'--runtime',str(root)],env={**os.environ,'HOME':str(home),'GPUI_PAUSE_IDLE_DISPLAY_LINK':'1','SHELL':'/bin/sh'},stdout=subprocess.DEVNULL,stderr=open(root/'app.log','w'))
    state={'app':app.pid,'host':host.pid,'bundle':str(bundle),'runtime':str(root),'started':time.time()}
    (OUT/'soak-native.json').write_text(json.dumps(state,indent=2));print(json.dumps(state),flush=True)
    time.sleep(10)
    pids={'client':app.pid,'daemon':host.pid,**{t['id']:t['pid'] for t in w()['terminals']}}
    counts={'external':sum(s['terminal'] is None for s in w()['agent_sessions']),'managed':sum(s['current'] for s in w()['agent_sessions'])}
    assert counts=={'external':40,'managed':8},counts
    metrics0=rpc('RecognitionStatus')['Recognition']['metrics'];start=time.monotonic();samples=[]
    for index in range(121):
        if index:time.sleep(max(0,start+index*60-time.monotonic()))
        sample={'seconds':time.monotonic()-start,'processes':{k:usage(pid) for k,pid in pids.items()}};samples.append(sample)
        with (OUT/'soak-trend.jsonl').open('a') as f:f.write(json.dumps(sample)+'\n')
    metrics1=rpc('RecognitionStatus')['Recognition']['metrics']
    seconds=samples[-1]['seconds'];first=samples[0]['processes'];last=samples[-1]['processes']
    report={**counts,'seconds':seconds,'metrics_before':metrics0,'metrics_after':metrics1,'processes':{k:{'cpu_seconds':(last[k]['cpu_ns']-first[k]['cpu_ns'])/1e9,'cpu_percent':(last[k]['cpu_ns']-first[k]['cpu_ns'])/1e9/seconds*100,'footprint_start_MiB':first[k]['footprint']/2**20,'footprint_end_MiB':last[k]['footprint']/2**20,'footprint_peak_MiB':max(s['processes'][k]['footprint'] for s in samples)/2**20,'disk_read':last[k]['read']-first[k]['read'],'disk_write':last[k]['write']-first[k]['write']} for k in pids}}
    (OUT/'soak-result.json').write_text(json.dumps(report,indent=2));print('SOAK COMPLETE',flush=True)
finally:
    if app is not None:app.terminate();app.wait(timeout=10)
    try:
        for t in w()['terminals']:rpc({'Stop':{'id':t['id']}})
    finally:host.terminate();host.wait(timeout=10)
