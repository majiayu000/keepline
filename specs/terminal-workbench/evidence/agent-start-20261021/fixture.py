"""R14 fixture with a stand-in `codex` on PATH, to start an Agent from a task and place its text. No model, credentials or user data."""
import json,os,pathlib,runpy,shlex,subprocess,tempfile,time,sys
OUT=pathlib.Path(__file__).resolve().parent
BASE=OUT.parents[3]
m=runpy.run_path(str(OUT.parent/'r5-20261021/fixture.py'));rpc=m['rpc'];edit=m['edit']
STATE=pathlib.Path('/tmp/kl-agent-start.json')
names=['lib','sys','模型上线 work','keepline','codex-mall']
titles=['添加 CLI 问题反馈接口','诊断 SSH 执行异常','核对反馈子命令的验收','审查反馈接口改动','实现缓存结果补传','核查指纹变更','评估视频队列方案','整理项目说明']
if sys.argv[1]=='prepare':
 parent=pathlib.Path.home()/'Library/Caches/KeeplineAcceptance';parent.mkdir(exist_ok=True)
 root=pathlib.Path(tempfile.mkdtemp(prefix='visual-',dir=parent));home=pathlib.Path(tempfile.mkdtemp(prefix='kl14-home-',dir='/tmp'));logs=home/'.codex/sessions';logs.mkdir(parents=True);(home/'.claude/projects').mkdir(parents=True)
 now=int(time.time())
 for n in range(40):
  project=root/'projects'/names[n//8];project.mkdir(parents=True,exist_ok=True);stamp=now-(6*86400 if n%8==5 else 60*(n+1));cid=f'visual-{n}'
  records=[('session_meta',{'id':cid,'cwd':str(project),'source':'cli','originator':'codex-tui','thread_source':'user'}),('event_msg',{'type':'user_message','message':titles[n%8]+('' if n<8 else ' · '+names[n//8])}),('event_msg',{'type':'task_started','turn_id':cid+'-turn'})]
  group=n%8
  if group==0:records.append(('event_msg',{'type':'request_user_input','message':'是否允许推送分支并创建 PR？改动已经完成，等待你确认。'}))
  elif group==1:records.append(('event_msg',{'type':'error','message':'SSH 命令退出码 255，连接中断。'}))
  elif group==2:records.extend([('response_item',{'type':'function_call','name':'exec_command','call_id':'test','arguments':json.dumps({'cmd':'bun test'})}),('response_item',{'type':'function_call_output','call_id':'test','output':'Process exited with code 0\n31 pass 0 fail'}),('event_msg',{'type':'task_complete','last_agent_message':'已完成，请核对命令行为与隐私要求。'})])
  elif group!=4:records.append(('event_msg',{'type':'turn_aborted'}))
  (logs/(cid+'.jsonl')).write_text(''.join(json.dumps({'timestamp':stamp,'type':t,'payload':p},ensure_ascii=False)+'\n' for t,p in records))
 (root/'bin').mkdir();(root/'bin'/'codex').symlink_to(OUT/'fake_codex.py')
 env={'PATH':str(root/'bin')+':/usr/bin:/bin:/usr/sbin:/sbin','HOME':str(home),'CODEX_HOME':str(home/'.codex'),'CLAUDE_CONFIG_DIR':str(home/'.claude'),'LANG':'en_US.UTF-8'}
 host=subprocess.Popen([str(BASE/'next/target/release/keeplined'),'serve','--runtime',str(root/'runtime')],env=env,stdout=subprocess.DEVNULL,stderr=open(root/'host.log','w'),start_new_session=True)
 deadline=time.monotonic()+30
 while not (root/'runtime/workbench.sock').exists():
  assert host.poll() is None and time.monotonic()<deadline;time.sleep(.05)
 for n,name in enumerate(names):edit(root,{'AddSpace':{'id':f's{n}','name':name,'cwd':str(root/'projects'/name)}})
 for n,title in enumerate(['CLI 问题反馈','给 CLI 加离线模式','清理旧的扫描轮询代码']):
  edit(root,{'AddTask':{'id':f'task{n}','space':'s0','title':title,'description':['给 CLI 加 feedback 子命令，提交版本号和系统信息，不上传本地文件内容。','网络不可用时常用命令照常可用；上传结果先存本地，恢复联网后补传。','删除旧扫描逻辑，保持空闲时无轮询。'][n],'acceptance':[{'id':f't{n}-c{i}','description':text,'evidence':'隔离测试：31 通过，0 失败' if n==0 and i==0 else None} for i,text in enumerate(['bun test 全部通过','不上传任何本地文件内容','推送分支并开 PR'] if n==0 else ['断网时返回缓存结果','恢复联网后 30 秒内补传','bun test 全部通过'])],'agent':'Codex'}})
 (root/'codex').symlink_to('/bin/sh')
 for n in range(3):
  lines=['› '+titles[n],'','• 已读取项目上下文','• 修改反馈子命令与错误处理','  bun test: 31 passed, 0 failed','','下一步：请核对验收标准。','此处为隔离视觉测试输出，未调用模型。']
  script='exec 3<'+shlex.quote(str(logs/f'visual-{n}.jsonl'))+'; printf "%s\\n" '+' '.join(shlex.quote(s) for s in lines)+'; while IFS= read -r line; do printf "%s\\n" "$line"; done'
  if n==1:
   # The log ends in an error; only the screen shows the newer approval, after binding.
   prompt=['','• 重试 SSH 前需要你确认','','  Would you like to run the following command?','','  $ ssh deploy@staging.internal \'systemctl restart api\'','','› 1. Yes, proceed (y)','  2. Yes, and don\'t ask again for this command in this session (a)','  3. No, and tell Codex what to do differently (esc)','','  Press enter to confirm or esc to cancel']
   script='exec 3<'+shlex.quote(str(logs/f'visual-{n}.jsonl'))+'; printf "%s\\n" '+' '.join(shlex.quote(s) for s in lines)+'; while [ ! -e '+shlex.quote(str(root/'show-approval'))+' ]; do sleep 0.2; done; printf "%s\\n" '+' '.join(shlex.quote(s) for s in prompt)+'; while IFS= read -r line; do printf "%s\\n" "$line"; done'
  r=rpc(root,{'Launch':{'id':f'p{n}','space':'s0','argv':[str(root/'codex'),'-c',script],'cwd':str(root/'projects/lib'),'cols':90,'rows':35}});assert 'Error' not in r,r
 rpc(root,{'Launch':{'id':'pstart','space':'s0','argv':['/bin/zsh','-f'],'cwd':str(root/'projects/lib'),'cols':100,'rows':35}})
 def leaf(p):return {'Leaf':p}
 def split(a,b,axis):return {'Split':{'axis':axis,'ratio':5000,'first':a,'second':b}}
 for id,title,task,layout in [('feedback','CLI 问题反馈','task0',split(leaf('p0'),split(leaf('p1'),leaf('p2'),'Down'),'Right')),('start','给 CLI 加离线模式','task1',leaf('pstart')),('tes','tes',None,None),('webs','webs',None,None)]:
  edit(root,{'PutTab':{'tab':{'id':id,'space':'s0','title':title,'default_task':task,'closed':False,'layout':layout}}})
 edit(root,{'SetPendingTask':{'terminal':'pstart','task':'task1'}})
 while True:
  w=rpc(root,'Workspace')['Workspace']
  if len(w['agent_sessions'])>=40:break
  assert time.monotonic()<deadline,'index timeout';time.sleep(.1)
 for n in range(3):
  t=next(t for t in w['terminals'] if t['id']==f'p{n}')
  r=rpc(root,{'Hook':{'runtime':'Codex','pid':t['pid'],'cli_session_id':f'visual-{n}','transcript':str(logs/f'visual-{n}.jsonl'),'ended':False,'event':None}});assert 'Error' not in r,r
  session=next(s for s in rpc(root,'Workspace')['Workspace']['agent_sessions'] if s['cli_session_id']==f'visual-{n}')
  edit(root,{'AssignTask':{'id':session['id'],'task':'task0'}})
 (root/'show-approval').touch()
 while True:
  w=rpc(root,'Workspace')['Workspace'];s1=next(s for s in w['agent_sessions'] if s['cli_session_id']=='visual-1')
  if '画面上的提示' in w['content'].get(s1['id'],{}).get('waiting',''):break
  assert time.monotonic()<deadline+30,'screen approval was not recognized';time.sleep(.1)
 print('screen wait:',w['content'][s1['id']]['waiting'],w['content'][s1['id']]['turn'])
 for s in rpc(root,'Workspace')['Workspace']['agent_sessions']:
  n=int(s['cli_session_id'].split('-')[-1])
  if n%8 in (6,7):edit(root,{'SetOutcome':{'id':s['id'],'outcome':{'Deferred':{'reason':'等待上游接口','until':now+86400}} if n%8==6 else 'Completed'}})
 state={'root':str(root),'home':str(home),'host':host.pid};STATE.write_text(json.dumps(state));print(state)
else:
 s=json.loads(STATE.read_text());root=pathlib.Path(s['root']);home=pathlib.Path(s['home'])
 if sys.argv[1]=='capture':
  for n in range(4,40,8):
   with (home/'.codex/sessions'/f'visual-{n}.jsonl').open('a') as f:f.write(json.dumps({'timestamp':int(time.time()),'type':'event_msg','payload':{'type':'task_started','turn_id':f'visual-{n}-refresh'}})+'\n')
  directory=OUT/sys.argv[2];directory.mkdir(exist_ok=True)
  env={'PATH':'/usr/bin:/bin:/usr/sbin:/sbin','HOME':str(home),'CODEX_HOME':str(home/'.codex'),'CLAUDE_CONFIG_DIR':str(home/'.claude'),'LANG':'en_US.UTF-8','GPUI_PAUSE_IDLE_DISPLAY_LINK':'1','KEEPLINE_PAGE_EVIDENCE':'1','KEEPLINE_PAGE_LIST':'TodoStart,StartPick,Main'}
  result=subprocess.run([str(BASE/'next/target/debug/keepline-app'),'--runtime',str(root/'runtime'),'--visual-evidence',str(directory)],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=60)
  (directory/'native-render.log').write_bytes(result.stdout);assert result.returncode==0,result.stdout.decode()[-4000:]
  print('native captures',len(list(directory.glob('*-native.png'))))
 elif sys.argv[1]=='stop':
  for t in rpc(root,'Workspace')['Workspace']['terminals']:rpc(root,{'Stop':{'id':t['id']}})
  os.kill(s['host'],15)
