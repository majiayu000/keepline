"""Isolated official terminal apps; UI actions and screenshots are Computer Use only."""
import json, os, pathlib, plistlib, shlex, shutil, socket, struct, subprocess, tempfile, time
BASE=pathlib.Path(__file__).resolve().parents[4]
OUT=pathlib.Path(__file__).resolve().parent
ROOT_FILE=pathlib.Path('/tmp/kl-r2-native.json')
def read(s,n):
 b=b''
 while len(b)<n:
  c=s.recv(n-len(b))
  if not c: raise EOFError()
  b+=c
 return b
def send(s,obj):
 b=b'\1'+json.dumps(obj).encode();s.sendall(struct.pack('>I',len(b))+b)
def receive(s):return json.loads(read(s,struct.unpack('>I',read(s,4))[0])[1:])
def rpc(root,obj):
 with socket.socket(socket.AF_UNIX) as s:
  s.settimeout(10);s.connect(str(pathlib.Path(root)/'runtime/workbench.sock'));send(s,{'Hello':{'major':3}});receive(s);send(s,obj);return receive(s)
def edit(root,obj):
 w=rpc(root,'Workspace')['Workspace'];r=rpc(root,{'Edit':{'revision':w['revision'],'edit':obj}})
 assert 'Error' not in r,r
 return r
if __name__=='__main__':
 root=pathlib.Path(tempfile.mkdtemp(prefix='kl-r2-',dir='/tmp'));home=root/'home';home.mkdir();project=root/'project';project.mkdir()
 runtime=root/'runtime';bundle=root/'Keepline Acceptance Test.app';mac=bundle/'Contents/MacOS';mac.mkdir(parents=True)
 env={**os.environ,'HOME':str(home),'CODEX_HOME':str(home/'.codex'),'CLAUDE_CONFIG_DIR':str(home/'.claude')}
 for name in ('keepline-app','keeplined'):shutil.copy2(BASE/'next/target/release'/name,mac/name)
 launcher=mac/'launch';launcher.write_text('#!/bin/sh\n'+''.join('export '+k+'='+shlex.quote(env[k])+'\n' for k in ('HOME','CODEX_HOME','CLAUDE_CONFIG_DIR'))+'exec '+shlex.quote(str(mac/'keepline-app'))+' --runtime '+shlex.quote(str(runtime))+' >>'+shlex.quote(str(root/'client.log'))+' 2>&1\n');launcher.chmod(0o755)
 (bundle/'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleName':'Keepline Acceptance Test','CFBundleDisplayName':'Keepline Acceptance Test','CFBundleIdentifier':'local.keepline.acceptance-test','CFBundleExecutable':'launch','CFBundlePackageType':'APPL','NSHighResolutionCapable':True}))
 subprocess.run(['codesign','--force','--sign','-',str(bundle)],check=True,capture_output=True)
 host=subprocess.Popen([str(mac/'keeplined'),'serve','--runtime',str(runtime)],cwd=project,env=env,stdout=subprocess.DEVNULL,stderr=open(root/'host.log','w'))
 until=time.monotonic()+15
 while not (runtime/'workbench.sock').exists():
  assert host.poll() is None and time.monotonic()<until;time.sleep(.05)
 edit(root,{'AddSpace':{'id':'native','name':'隔离原生验收','cwd':str(project)}})
 (project/'sample.txt').write_text(''.join(f'{i:03} 中文输入与鼠标定位 · line {i}\n' for i in range(1,151)))
 (project/'vimrc').write_text('set nocompatible\nset mouse=a\nset number\nlet &t_fe = "\\<Esc>[?1004h"\nlet &t_fd = "\\<Esc>[?1004l"\nexecute "set <FocusGained>=\\<Esc>[I"\nexecute "set <FocusLost>=\\<Esc>[O"\nau FocusGained * call writefile(["gained"], "'+str(project/'focus.log')+'", "a")\nau FocusLost * call writefile(["lost"], "'+str(project/'focus.log')+'", "a")\n')
 subprocess.run(['git','init','-q',str(project)],check=True)
 subprocess.run(['git','-C',str(project),'add','sample.txt'],check=True)
 subprocess.run(['git','-C',str(project),'-c','user.name=Acceptance','-c','user.email=test@example.invalid','commit','-qm','local fixture'],check=True)
 for id,argv in [('vim',['/usr/bin/vim','-u',str(project/'vimrc'),'-N','-i','NONE','-n',str(project/'sample.txt')]),('lazygit',['/opt/homebrew/bin/lazygit']),('shell',['/bin/zsh','-f'])]:
  r=rpc(root,{'Launch':{'id':id,'space':'native','argv':argv,'cwd':str(project),'cols':120,'rows':35}});assert 'Error' not in r,r
  edit(root,{'PutTab':{'tab':{'id':id,'space':'native','default_task':None,'title':id,'closed':False,'layout':{'Leaf':id}}}})
 report={'root':str(root),'host':host.pid,'bundle':str(bundle),'user_runtime_touched':False,'driver':os.getpid()}
 ROOT_FILE.write_text(json.dumps(report));(OUT/'native-runtime.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)

 import signal
 def stop(*_): raise KeyboardInterrupt()
 signal.signal(signal.SIGTERM,stop)
 try: signal.pause()
 except KeyboardInterrupt: pass
 finally:
  for t in rpc(root,'Workspace')['Workspace']['terminals']:
   rpc(root,{'Stop':{'id':t['id']}})
  host.terminate();host.wait(timeout=10)
