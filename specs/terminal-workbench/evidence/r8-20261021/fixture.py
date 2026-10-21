"""Fresh isolated one-pane resource fixture; native activation follows separately."""
import pathlib,json,runpy,subprocess,tempfile,os,shutil,plistlib,shlex,time
repo=pathlib.Path(__file__).resolve().parents[4];out=pathlib.Path(__file__).resolve().parent; prior=out.parent/'r5-20261021'
m=runpy.run_path(str(prior/'fixture.py'));rpc=m['rpc'];edit=m['edit']
r=pathlib.Path(tempfile.mkdtemp(prefix='kl8-res-',dir='/tmp'));h=r/'home';h.mkdir();(h/'.codex/sessions').mkdir(parents=True);(h/'.claude/projects').mkdir(parents=True)
b=r/'Keepline Profile Test.app';mac=b/'Contents/MacOS';mac.mkdir(parents=True)
for n in ('keepline-app','keeplined'):shutil.copy2(repo/'next/target/release'/n,mac/n)
env={'PATH':'/usr/bin:/bin:/usr/sbin:/sbin','LANG':'en_US.UTF-8','HOME':str(h),'CODEX_HOME':str(h/'.codex'),'CLAUDE_CONFIG_DIR':str(h/'.claude')}
p=subprocess.Popen([str(mac/'keeplined'),'serve','--runtime',str(r/'runtime')],env=env,stdout=subprocess.DEVNULL,stderr=open(r/'host.log','w'),start_new_session=True)
end=time.monotonic()+15
while not (r/'runtime/workbench.sock').exists():
 assert time.monotonic()<end;time.sleep(.1)
edit(r,{'AddSpace':{'id':'s','name':'资源对照','cwd':str(r)}})
rpc(r,{'Launch':{'id':'load1','space':'s','argv':['/usr/bin/python3',str(out/'load-one.py')],'cwd':str(r),'cols':160,'rows':48}})
edit(r,{'PutTab':{'tab':{'id':'load','space':'s','title':'资源对照','default_task':None,'closed':False,'layout':{'Leaf':'load1'}}}})
f=mac/'launch';f.write_text('#!/bin/sh\nexec /usr/bin/env -i '+ ' '.join(k+'='+shlex.quote(v) for k,v in env.items())+' '+shlex.quote(str(mac/'keepline-app'))+' --runtime '+shlex.quote(str(r/'runtime'))+' >>'+shlex.quote(str(r/'app.log'))+' 2>&1\n');f.chmod(0o755)
(b/'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleName':'Keepline Profile Test','CFBundleDisplayName':'Keepline Profile Test','CFBundleIdentifier':'local.keepline.profile-test','CFBundleExecutable':'launch','CFBundlePackageType':'APPL','CFBundleVersion':'1','NSHighResolutionCapable':True}))
subprocess.run(['codesign','--force','--sign','-',str(b)],check=True,capture_output=True)
state={'root':str(r),'home':str(h),'bundle':str(b),'host':p.pid};pathlib.Path('/tmp/kl8-resource.json').write_text(json.dumps(state));print(json.dumps(state))
