"""Private HOME with 40 synthetic sessions. No credentials, model calls or user settings."""
import json,os,pathlib,plistlib,shutil,subprocess,tempfile,time,runpy,shlex
BASE=pathlib.Path(__file__).resolve().parents[4]; OUT=pathlib.Path(__file__).resolve().parent
m=runpy.run_path(str(OUT.parent/'r4-20261021/native-fixture.py'));rpc=m['rpc'];edit=m['edit']
STATE=pathlib.Path('/tmp/kl-r5.json')
def leaf(id):return {'Leaf':id}
def split(a,b,axis='Right'):return {'Split':{'axis':axis,'ratio':5000,'first':a,'second':b}}
if __name__=='__main__':
 import sys
 if sys.argv[1]=='prepare':
  parent=pathlib.Path.home()/'Library/Caches/KeeplineAcceptance';parent.mkdir(exist_ok=True)
  root=pathlib.Path(tempfile.mkdtemp(prefix='r5-',dir=parent));root.chmod(0o700);home=pathlib.Path(tempfile.mkdtemp(prefix='kl5-home-',dir='/tmp'));logs=home/'.codex/sessions';logs.mkdir(parents=True);(home/'.claude/projects').mkdir(parents=True)
  for s in range(4):
   project=root/f'projects/space{s+1}';project.mkdir(parents=True)
   for i in range(10):
    now=int(time.time());id=f'r5-{s+1}-{i+1:02}';records=[{'type':'session_meta','payload':{'id':id,'cwd':str(project),'source':'cli','originator':'codex-tui','thread_source':'user'}},{'type':'event_msg','payload':{'type':'user_message','message':f'空间 {s+1} 会话 {i+1:02}：核对隔离导航计数。'}}]
    event={'type':'request_user_input','message':f'空间 {s+1} 问题 {i+1}：继续还是暂停？'} if i<2 else ({'type':'error','message':f'空间 {s+1} 测试错误'} if i==2 else {'type':'task_complete','last_agent_message':'模拟回合结束。'})
    records.append({'type':'event_msg','payload':event});(logs/(id+'.jsonl')).write_text(''.join(json.dumps({'timestamp':now,**r},ensure_ascii=False)+'\n' for r in records))
  bundle=root/'Keepline Native Acceptance.app';mac=bundle/'Contents/MacOS';mac.mkdir(parents=True)
  for name in ['keepline-app','keeplined']:shutil.copy2(BASE/'next/target/release'/name,mac/name)
  launch=mac/'launch';launch.write_text('#!/bin/sh\n'+''.join('export '+k+'='+shlex.quote(str(v))+'\n' for k,v in {'HOME':home,'CODEX_HOME':home/'.codex','CLAUDE_CONFIG_DIR':home/'.claude'}.items())+'exec '+shlex.quote(str(mac/'keepline-app'))+' --runtime '+shlex.quote(str(root/'runtime'))+' >>'+shlex.quote(str(root/'client.log'))+' 2>&1\n');launch.chmod(0o755)
  (bundle/'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleName':'Keepline Native Acceptance','CFBundleDisplayName':'Keepline Native Acceptance','CFBundleIdentifier':'local.keepline.native-acceptance','CFBundleExecutable':'launch','CFBundlePackageType':'APPL','CFBundleVersion':'1','NSHighResolutionCapable':True}))
  subprocess.run(['codesign','--force','--sign','-',str(bundle)],check=True,capture_output=True)
  state={'root':str(root),'bundle':str(bundle),'home':str(home)};STATE.write_text(json.dumps(state));(OUT/'runtime.json').write_text(json.dumps(state,indent=2)+'\n');print(json.dumps(state))
 else:
  root=pathlib.Path(json.loads(STATE.read_text())['root'])
  if sys.argv[1]=='seed':
   for s in range(4):edit(root,{'AddSpace':{'id':f's{s+1}','name':f'验收空间 {s+1}','cwd':str(root/f'projects/space{s+1}')}})
   for i in range(4):rpc(root,{'Launch':{'id':f'pane{i+1}','space':'s1','argv':['/bin/zsh','-f'],'cwd':str(root/'projects/space1'),'cols':80,'rows':24}})
   tree=split(split(leaf('pane1'),leaf('pane2'),'Down'),split(leaf('pane3'),leaf('pane4'),'Down'))
   edit(root,{'PutTab':{'tab':{'id':'four','space':'s1','title':'四画面输入验收','default_task':None,'closed':False,'layout':tree}}})
   print(json.dumps(rpc(root,'Workspace')['Workspace']['spaces'],ensure_ascii=False))
  elif sys.argv[1]=='readback':
   w=rpc(root,'Workspace')['Workspace'];(OUT/'workspace.json').write_text(json.dumps(w,ensure_ascii=False,indent=2)+'\n');print({'sessions':len(w['agent_sessions']),'terminals':len(w['terminals'])})
