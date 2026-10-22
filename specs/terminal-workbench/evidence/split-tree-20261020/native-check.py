"""Isolated real PTYs; native GPUI screenshots and eight-visible-pane resource samples."""
import ctypes, glob, json, os, pathlib, socket, struct, subprocess, tempfile, time, plistlib, shutil
BASE=pathlib.Path(__file__).resolve().parents[4]
OUT=pathlib.Path(__file__).resolve().parent
root=pathlib.Path(tempfile.mkdtemp(prefix='kl-split-',dir='/tmp'))
exe=max((pathlib.Path(p) for p in glob.glob(str(BASE/'next/target/debug/deps/managed_session-*')) if os.access(p,os.X_OK) and not p.endswith('.d')),key=lambda p:p.stat().st_mtime)
host=subprocess.Popen([str(exe),'--ignored','--exact','daemon_fixture','--nocapture'],env={**os.environ,'KEEPLINE_FIXTURE_RUNTIME':str(root)},stdout=subprocess.DEVNULL,stderr=open(root/'host.log','w'))
def read(s,n):
    data=b''
    while len(data)<n:
        part=s.recv(n-len(data))
        if not part: raise EOFError()
        data+=part
    return data
def receive(s): return json.loads(read(s,struct.unpack('>I',read(s,4))[0])[1:])
def send(s,obj):
    data=b'\x01'+json.dumps(obj).encode();s.sendall(struct.pack('>I',len(data))+data)
def rpc(obj):
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(10);s.connect(str(root/'workbench.sock'));send(s,{'Hello':{'major':2}});receive(s);send(s,obj);r=receive(s)
        if isinstance(r,dict) and 'Error' in r: raise RuntimeError(r)
        return r
def workspace():return rpc('Workspace')['Workspace']
def edit(obj):return rpc({'Edit':{'revision':workspace()['revision'],'edit':obj}})
def leaf(id):return {'Leaf':id}
def split(a,b,axis='Right',ratio=5000):return {'Split':{'axis':axis,'ratio':ratio,'first':a,'second':b}}
def tab(name,tree):return {'id':name,'space':'s','title':{'single':'单画面','nested':'嵌套四分屏','eight':'八画面资源测试'}[name],'default_task':None,'closed':False,'layout':tree}
class Usage(ctypes.Structure):
    _fields_=[('uuid',ctypes.c_ubyte*16)]+[(x,ctypes.c_uint64) for x in ['user','system','idle','interrupt','pageins','wired','resident','footprint','start','exit','child_user','child_system','child_idle','child_interrupt','child_pageins','child_elapsed','read','write']]
lib=ctypes.CDLL('/usr/lib/libproc.dylib')
class Timebase(ctypes.Structure): _fields_=[('numer',ctypes.c_uint32),('denom',ctypes.c_uint32)]
timebase=Timebase()
assert ctypes.CDLL('/usr/lib/libSystem.B.dylib').mach_timebase_info(ctypes.byref(timebase))==0
# rusage times are Mach ticks; the local process_time calibration is archived beside this script.
ns_per_tick=timebase.numer/timebase.denom
def usage(pid):
    u=Usage()
    if lib.proc_pid_rusage(pid,2,ctypes.byref(u))!=0:raise RuntimeError(f'usage {pid}')
    return {'cpu_ns':(u.user+u.system)*ns_per_tick,'footprint':u.footprint,'resident':u.resident,'read':u.read,'write':u.write}
app=None
try:
    deadline=time.time()+10
    while not (root/'workbench.sock').exists():
        assert time.time()<deadline;time.sleep(.05)
    edit({'AddSpace':{'id':'s','name':'分屏验收 · 隔离终端','cwd':str(root)}})
    # /bin/sh only. No real CLI, no hooks, and no access to real transcript directories.
    for i in range(8):
        script=f'''tty > {root}/tty{i}; printf '\\033[38;5;141m            \\033[0m\\n'; printf 'Pane {i+1} · 中文实现反馈\\n'; printf 'PWD: %s\\n' "$PWD"; exec /bin/cat'''
        rpc({'Launch':{'id':f'p{i}','space':'s','argv':['/bin/sh','-c',script],'cwd':str(root),'cols':80,'rows':24}})
    edit({'PutTab':{'tab':tab('single',leaf('p0'))}})
    # A terminal has one placement; these separate terminal sets are configured per capture stage.
    edit({'PutTab':{'tab':tab('nested',split(split(leaf('p1'),leaf('p2'),'Down',6000),split(leaf('p3'),leaf('p4'),'Down',4000),'Right',5800))}})
    edit({'PutTab':{'tab':tab('eight',split(leaf('p5'),split(leaf('p6'),leaf('p7'),'Down')))}})
    if os.environ.get('KL_RESOURCE_ONLY')!='1':
        run=subprocess.run([str(BASE/'next/target/debug/keepline-app'),'--runtime',str(root),'--visual-evidence',str(OUT)],env={**os.environ,'GPUI_PAUSE_IDLE_DISPLAY_LINK':'1','KEEPLINE_SPLIT_EVIDENCE':'1','SHELL':'/bin/sh'},capture_output=True,timeout=40)
        (OUT/'native-render.log').write_bytes(run.stdout+run.stderr);assert run.returncode==0,run.stderr.decode()
    if os.environ.get("KL_NATIVE_ONLY")=="1": raise SystemExit(0)
    # One tab with eight visible leaves, original PTYs reused without spawning Agents.
    tree=split(split(split(leaf('p0'),leaf('p1'),'Down'),split(leaf('p2'),leaf('p3'),'Down')),split(split(leaf('p4'),leaf('p5'),'Down'),split(leaf('p6'),leaf('p7'),'Down')))
    edit({'PutTab':{'tab':tab('eight',tree)}})
    for t in workspace()['tabs']:
        if t['id']!='eight':edit({'CloseTab':{'id':t['id']}})
    bundle=root/'Keepline Split Check.app'; mac=bundle/'Contents/MacOS';mac.mkdir(parents=True)
    shutil.copy2(BASE/'next/target/debug/keepline-app',mac/'keepline-app')
    (bundle/'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleName':'Keepline Split Check','CFBundleDisplayName':'Keepline Split Check','CFBundleExecutable':'keepline-app','CFBundleIdentifier':'local.keepline.split-check','CFBundlePackageType':'APPL','CFBundleVersion':'1','NSHighResolutionCapable':True}))
    app=subprocess.Popen([str(mac/'keepline-app'),'--runtime',str(root)],env={**os.environ,'GPUI_PAUSE_IDLE_DISPLAY_LINK':'1'},stdout=subprocess.DEVNULL,stderr=open(root/'app.log','w'))
    time.sleep(4)
    (OUT/'resource-client.json').write_text(json.dumps({'pid':app.pid,'bundle':str(bundle),'runtime':str(root)},indent=2))
    if os.environ.get('KL_WAIT_FOR_UI')=='1':
        print('READY '+str(root),flush=True)
        until=time.monotonic()+180
        while not (root/'measure-ready').exists():
            if time.monotonic()>until:raise TimeoutError('fixture activation not confirmed')
            time.sleep(.5)
    visibility=json.loads(subprocess.check_output(['swift',str(OUT/'window-visibility.swift'),str(app.pid)]))
    (OUT/'resource-window.json').write_text(json.dumps(visibility,indent=2))
    assert not visibility['overlapping_app_window_areas'],'fixture obscured; raise only the fixture before measuring'
    pids={'client':app.pid,'daemon':host.pid,**{t['id']:t['pid'] for t in workspace()['terminals']}}
    before={k:usage(pid) for k,pid in pids.items()};start=time.monotonic();time.sleep(30);elapsed=time.monotonic()-start
    after={k:usage(pid) for k,pid in pids.items()}
    report={'kind':'8 visible idle PTYs; native macOS window; no Ghostty baseline','seconds':elapsed,'timebase':{'numer':timebase.numer,'denom':timebase.denom},'pids':pids,'processes':{k:{'cpu_seconds':(after[k]['cpu_ns']-before[k]['cpu_ns'])/1e9,'cpu_percent':(after[k]['cpu_ns']-before[k]['cpu_ns'])/1e9/elapsed*100,'physical_mib':after[k]['footprint']/2**20,'resident_mib':after[k]['resident']/2**20,'disk_read_delta':after[k]['read']-before[k]['read'],'disk_write_delta':after[k]['write']-before[k]['write']} for k in pids}}
    report['total_cpu_percent']=sum(x['cpu_percent'] for x in report['processes'].values());report['total_physical_mib']=sum(x['physical_mib'] for x in report['processes'].values())
    (OUT/'eight-pane-resources.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2),flush=True)
    assert report['total_cpu_percent']<.5,'eight-pane idle CPU budget exceeded'
    fds=[os.open((root/f'tty{i}').read_text().strip(),os.O_WRONLY|os.O_NOCTTY) for i in range(8)]
    try:
        before={k:usage(pid) for k,pid in pids.items()};start=time.monotonic();tick=0
        while time.monotonic()-start<30:
            for i,fd in enumerate(fds):os.write(fd,f'Pane {i+1} output {tick}: 中文实现反馈 0123456789\r\n'.encode())
            tick+=1;time.sleep(.125)
        elapsed=time.monotonic()-start;after={k:usage(pid) for k,pid in pids.items()}
        active={'kind':'8 visible PTYs, each writes 8 lines/sec, native renderer; no real Agents','seconds':elapsed,'timebase':{'numer':timebase.numer,'denom':timebase.denom},'ticks':tick,'processes':{k:{'cpu_seconds':(after[k]['cpu_ns']-before[k]['cpu_ns'])/1e9,'cpu_percent':(after[k]['cpu_ns']-before[k]['cpu_ns'])/1e9/elapsed*100,'physical_mib':after[k]['footprint']/2**20,'resident_mib':after[k]['resident']/2**20} for k in pids}}
        active['total_cpu_percent']=sum(x['cpu_percent'] for x in active['processes'].values());active['total_physical_mib']=sum(x['physical_mib'] for x in active['processes'].values())
        (OUT/'eight-pane-active-resources.json').write_text(json.dumps(active,indent=2));print(json.dumps(active,indent=2),flush=True)
    finally:
        for fd in fds:os.close(fd)
    visibility_end=json.loads(subprocess.check_output(['swift',str(OUT/'window-visibility.swift'),str(app.pid)]))
    (OUT/'resource-window-after.json').write_text(json.dumps(visibility_end,indent=2))
    assert not visibility_end['overlapping_app_window_areas'],'fixture became obscured'
    (OUT/'fixture-workspace.json').write_text(json.dumps(workspace(),ensure_ascii=False,indent=2))
finally:
    if app is not None:app.terminate();app.wait(timeout=5)
    for t in workspace()['terminals']:rpc({'Stop':{'id':t['id']}})
    host.terminate();host.wait(timeout=5)
