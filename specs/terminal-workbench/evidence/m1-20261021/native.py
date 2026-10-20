"""Isolated native acceptance. Fake CLI executables record input; no model/network call."""
import json,os,pathlib,socket,struct,subprocess,tempfile,time,plistlib,shutil
BASE=pathlib.Path(__file__).resolve().parents[4]
OUT=pathlib.Path(__file__).resolve().parent
root=pathlib.Path(tempfile.mkdtemp(prefix='kl-m1-',dir='/tmp'));os.chmod(root,0o700)
home=root/'home';home.mkdir();bins=root/'bin';bins.mkdir()
(home/'.zprofile').write_text(f"export PATH='{bins}:/usr/bin:/bin:/usr/sbin:/sbin'\n")
source=root/'fake.c';source.write_text(r'''
#include <stdio.h>
#include <unistd.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char**argv){
 const char*root=getenv("KL_FIXTURE");char path[2048];snprintf(path,sizeof(path),"%s/invoked-%d",root,getpid());FILE*out=fopen(path,"w");for(int i=0;i<argc;i++)fprintf(out,"%s\n",argv[i]);fclose(out);
 snprintf(path,sizeof(path),"%s/logs/mock-%d.jsonl",root,getpid());FILE*log=fopen(path,"w");fprintf(log,"{\"type\":\"session_meta\",\"payload\":{\"id\":\"mock-%d\",\"cwd\":\"/fixture-project\",\"source\":\"cli\",\"originator\":\"codex-tui\"}}\n",getpid());fflush(log);
 puts("模拟 Agent：仅记录参数，无模型调用。输入只回显。");fflush(stdout);char line[4096];while(fgets(line,sizeof(line),stdin)){printf("%s",line);fflush(stdout);}fclose(log);return 0;
}''')
subprocess.run(['cc',str(source),'-o',str(bins/'codex')],check=True)
shutil.copy2(bins/'codex',bins/'claude')
host=subprocess.Popen([str(BASE/'next/target/debug/examples/content_fixture'),str(root)],env={**os.environ,'HOME':str(home),'SHELL':'/bin/zsh','KL_FIXTURE':str(root),'PATH':f'{bins}:/usr/bin:/bin:/usr/sbin:/sbin'},stdout=subprocess.DEVNULL,stderr=open(root/'host.log','w'))
def read(s,n):
 b=b''
 while len(b)<n:
  d=s.recv(n-len(b))
  if not d:raise EOFError()
  b+=d
 return b
def recv(s):return json.loads(read(s,struct.unpack('>I',read(s,4))[0])[1:])
def send(s,v):
 b=b'\1'+json.dumps(v).encode();s.sendall(struct.pack('>I',len(b))+b)
def rpc(v):
 with socket.socket(socket.AF_UNIX) as s:
  s.settimeout(15);s.connect(str(root/'workbench.sock'));send(s,{'Hello':{'major':3}});recv(s);send(s,v);r=recv(s)
  if 'Error' in r:raise RuntimeError(r)
  return r
app=None
try:
 until=time.monotonic()+15
 while not (root/'workbench.sock').exists():
  assert time.monotonic()<until;time.sleep(.1)
 w=rpc('Workspace')['Workspace']
 rpc({'Edit':{'revision':w['revision'],'edit':{'AddSpace':{'id':'m1','name':'M1 隔离项目','cwd':str(root)}}}})
 bundle=root/'Keepline Milestone Test.app';mac=bundle/'Contents/MacOS';mac.mkdir(parents=True)
 shutil.copy2(BASE/'next/target/debug/keepline-app',mac/'keepline-app')
 (bundle/'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleName':'Keepline Milestone Test','CFBundleExecutable':'keepline-app','CFBundleIdentifier':'local.keepline.milestones','CFBundlePackageType':'APPL','CFBundleVersion':'1'}))
 app=subprocess.Popen([str(mac/'keepline-app'),'--runtime',str(root)],env={**os.environ,'HOME':str(home),'SHELL':'/bin/zsh','KL_FIXTURE':str(root),'GPUI_PAUSE_IDLE_DISPLAY_LINK':'1'},stdout=subprocess.DEVNULL,stderr=open(root/'app.log','w'))
 state={'app':app.pid,'host':host.pid,'bundle':str(bundle),'root':str(root)};(OUT/'native.json').write_text(json.dumps(state,indent=2)+'\n');print(json.dumps(state),flush=True)
 while not (root/'finish').exists():time.sleep(1)
 (OUT/'workspace.json').write_text(json.dumps(rpc('Workspace'),ensure_ascii=False,indent=2)+'\n')
finally:
 if app is not None:app.terminate();app.wait(timeout=5)
 try:
  for t in rpc('Workspace')['Workspace']['terminals']:rpc({'Stop':{'id':t['id']}})
 finally:host.terminate();host.wait(timeout=5)
