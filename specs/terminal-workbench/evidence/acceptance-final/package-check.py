"""Launch packaged binaries with private HOME; process/RPC proof, not Finder/Computer Use proof."""
import json,os,pathlib,plistlib,socket,struct,subprocess,tempfile,time
BASE=pathlib.Path(__file__).resolve().parents[4]
OUT=pathlib.Path(__file__).resolve().parent
root=pathlib.Path(tempfile.mkdtemp(prefix='kl-package-rpc-',dir='/tmp'));home=root/'home';home.mkdir()
runtime=home/'.local/share/keepline-workbench'
binary=BASE/'next/target/bundle/Keepline.app/Contents/MacOS/keepline-app'
env={**os.environ,'HOME':str(home),'CODEX_HOME':str(home/'.codex'),'CLAUDE_CONFIG_DIR':str(home/'.claude')}
env.pop('GPUI_PAUSE_IDLE_DISPLAY_LINK',None)
def read(s,n):
    data=b''
    while len(data)<n:
        chunk=s.recv(n-len(data))
        if not chunk:raise EOFError()
        data+=chunk
    return data
def receive(s):return json.loads(read(s,struct.unpack('>I',read(s,4))[0])[1:])
def send(s,obj):
    b=b'\x01'+json.dumps(obj).encode();s.sendall(struct.pack('>I',len(b))+b)
def rpc(obj):
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(10);s.connect(str(runtime/'workbench.sock'));send(s,{'Hello':{'major':3}});receive(s);send(s,obj);return receive(s)
def workspace():return rpc('Workspace')['Workspace']
def launch():return subprocess.Popen([str(binary)],cwd=BASE,env=env,stdout=subprocess.DEVNULL,stderr=open(root/'client.log','a'))
app=launch();host=None
try:
    deadline=time.monotonic()+20
    while not (runtime/'workbench.sock').exists():
        assert app.poll() is None;assert time.monotonic()<deadline;time.sleep(.1)
    while not workspace()['spaces']:
        assert time.monotonic()<deadline;time.sleep(.1)
    # SO_PEERPID obtains only the isolated host behind this private socket.
    with socket.socket(socket.AF_UNIX) as s:
        s.connect(str(runtime/'workbench.sock'));host=struct.unpack('i',s.getsockopt(0,2,4))[0]
    w=workspace();rpc({'Launch':{'id':'package-cat','space':w['spaces'][0]['id'],'argv':['/bin/cat'],'cwd':str(root),'cols':80,'rows':24}})
    before=workspace()['terminals'][0]
    second=launch();assert second.wait(timeout=10)==0;assert app.poll() is None
    first_pid=app.pid;app.terminate();app.wait(timeout=10)
    assert workspace()['terminals'][0]['pid']==before['pid'];os.kill(before['pid'],0)
    app=launch();time.sleep(2);assert app.poll() is None
    after=workspace()['terminals'][0];assert after==before
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(10);s.connect(str(runtime/'workbench.sock'));send(s,{'Hello':{'major':3}});receive(s)
        send(s,{'Attach':{'id':'package-cat','takeover':False}});attached=receive(s)
        # Automatic ownership tested only if the app has no active visible pane (no tab was added).
        assert attached['Attached']['writable']
        while True:
            b=read(s,struct.unpack('>I',read(s,4))[0])
            if b[0]==3:break
        send(s,{'Input':{'bytes':list(b'PACKAGE-RECONNECTED\n')}})
        output=b''
        while b'PACKAGE-RECONNECTED' not in output:
            b=read(s,struct.unpack('>I',read(s,4))[0])
            if b[0]==4:output+=b[1:]
    report={'passed':True,'evidence_type':'packaged process/RPC integration; desktop locked, no Finder double-click or native screenshot proof','first_client':first_pid,'reopened_client':app.pid,'host':host,'terminal_before':before,'terminal_after':after,'duplicate_exit':second.returncode,'auto_host':True,'automatic_input_owner':True,'reconnected_output':True,'external_idle_env_removed':True}
    (OUT/'package-check.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));print(json.dumps(report,ensure_ascii=False),flush=True)
finally:
    app.terminate();app.wait(timeout=10)
    if host:
        rpc({'Stop':{'id':'package-cat'}});os.kill(host,15)
