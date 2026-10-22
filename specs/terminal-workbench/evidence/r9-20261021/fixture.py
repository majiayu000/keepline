"""Private Claude/GLM acceptance. Read-only provider env at exec; never persist credentials."""
import json,os,pathlib,plistlib,runpy,shlex,shutil,subprocess,tempfile,time
BASE=pathlib.Path(__file__).resolve().parents[4];OUT=pathlib.Path(__file__).resolve().parent
m=runpy.run_path(str(OUT.parent/'r4-20261021/native-fixture.py'));rpc=m['rpc'];edit=m['edit']
if __name__=='__main__':
 parent=pathlib.Path.home()/'Library/Caches/KeeplineAcceptance';parent.mkdir(exist_ok=True)
 r=pathlib.Path(tempfile.mkdtemp(prefix='r9-',dir=parent));r.chmod(0o700);h=pathlib.Path(tempfile.mkdtemp(prefix='kl9-home-',dir='/tmp'));(r/'home').symlink_to(h);project=r/'project';project.mkdir();bin=r/'bin';bin.mkdir()
 (h/'.codex/sessions').mkdir(parents=True);(h/'.claude/projects').mkdir(parents=True)
 (h/'.claude/settings.json').write_text(json.dumps({'disableAllHooks':True}))
 (h/'.claude.json').write_text(json.dumps({'hasCompletedOnboarding':True,'theme':'dark','projects':{str(project):{'hasTrustDialogAccepted':True,'hasCompletedProjectOnboarding':True}}}))
 (r/'mcp.json').write_text('{"mcpServers":{}}')
 # Only provider DB path and CLI path are embedded, never the provider values.
 script='''#!/usr/bin/python3
import os,sys,json,sqlite3
from pathlib import Path
root=Path(__file__).resolve().parents[1]
with sqlite3.connect("file:"+DB+"?mode=ro",uri=True) as db:
 row=db.execute("select settings_config from providers where id=? and app_type='claude'",("fe4d110b-81b5-43d1-80db-b42ccaeafde5",)).fetchone()
 if row is None: raise SystemExit("authorized provider missing")
 env=json.loads(row[0])["env"]
base={"PATH":str(root/"bin")+":/usr/bin:/bin:/usr/sbin:/sbin","LANG":"en_US.UTF-8","HOME":str((root/"home").resolve()),"CLAUDE_CONFIG_DIR":str((root/"home").resolve()/".claude")}
base.update({k:str(v) for k,v in env.items()})
base.update({"HOME":str((root/"home").resolve()),"CLAUDE_CONFIG_DIR":str((root/"home").resolve()/".claude"),"DISABLE_AUTOUPDATER":"1"})
# keeplined sets these on the PTY; without them Claude Code renders monochrome.
base.update({k:os.environ[k] for k in ("TERM","COLORTERM") if k in os.environ})
os.execve(CLI,[CLI,"--permission-mode","default","--strict-mcp-config","--mcp-config",str(root/"mcp.json"),*sys.argv[1:]],base)
'''.replace('DB',repr(str(pathlib.Path.home()/'.cc-switch/cc-switch.db'))).replace('CLI',repr(str(pathlib.Path.home()/'.local/bin/claude')))
 launcher=bin/'claude';launcher.write_text(script);launcher.chmod(0o700)
 b=r/'Keepline Claude ID Test.app';mac=b/'Contents/MacOS';mac.mkdir(parents=True)
 for n in ['keepline-app','keeplined']:shutil.copy2(BASE/'next/target/release'/n,mac/n)
 env={'PATH':str(bin)+':/usr/bin:/bin:/usr/sbin:/sbin','LANG':'en_US.UTF-8','HOME':str(h),'CODEX_HOME':str(h/'.codex'),'CLAUDE_CONFIG_DIR':str(h/'.claude')}
 launch=mac/'launch';launch.write_text('#!/bin/sh\nexec /usr/bin/env -i '+' '.join(k+'='+shlex.quote(v) for k,v in env.items())+' '+shlex.quote(str(mac/'keepline-app'))+' --runtime '+shlex.quote(str(r/'runtime'))+' >>'+shlex.quote(str(r/'client.log'))+' 2>&1\n');launch.chmod(0o755)
 (b/'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleName':'Keepline Claude ID Test','CFBundleDisplayName':'Keepline Claude ID Test','CFBundleIdentifier':'local.keepline.claude-id-test','CFBundleExecutable':'launch','CFBundlePackageType':'APPL','CFBundleVersion':'1','NSHighResolutionCapable':True}))
 subprocess.run(['codesign','--force','--sign','-',str(b)],capture_output=True,check=True)
 host=subprocess.Popen([str(mac/'keeplined'),'serve','--runtime',str(r/'runtime')],cwd=project,env=env,stdout=subprocess.DEVNULL,stderr=open(r/'host.log','w'),start_new_session=True)
 end=time.monotonic()+15
 while not (r/'runtime/workbench.sock').exists():
  assert time.monotonic()<end;time.sleep(.1)
 edit(r,{'AddSpace':{'id':'s','name':'Claude ID 隔离验收','cwd':str(project)}})
 edit(r,{'AddTask':{'id':'proof','space':'s','title':'Write r9-proof.txt containing exactly ok','description':'Use the Write tool only. No other tools. Then reply done.','acceptance':[{'id':'c1','description':'File contains ok','evidence':None}],'agent':'Claude'}})
 state={'root':str(r),'bundle':str(b),'host':host.pid,'project':str(project),'home':str(h)};pathlib.Path('/tmp/kl9-native.json').write_text(json.dumps(state));print(json.dumps(state))
