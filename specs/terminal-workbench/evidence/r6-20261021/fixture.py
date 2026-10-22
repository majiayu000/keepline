"""Synthetic, isolated notification transitions. No model calls or user configuration."""
import json,pathlib,runpy,os,shlex,time,sys,sqlite3
OUT=pathlib.Path(__file__).resolve().parent
m=runpy.run_path(str(OUT.parent/'r5-20261021/fixture.py'));rpc=m['rpc'];edit=m['edit'];s=json.loads(pathlib.Path('/tmp/kl-r6.json').read_text());root=pathlib.Path(s['root']);log=pathlib.Path(s['home'])/'.codex/sessions/r6-native.jsonl'
def encoded(payload,type='event_msg'):return json.dumps({'timestamp':int(time.time()),'type':type,'payload':payload},ensure_ascii=False)+'\n'
if sys.argv[1]=='prepare':
 executable=root/'codex'
 if not executable.exists():executable.symlink_to('/bin/sh')
 seed=root/'notification-seed.jsonl';seed.write_text(encoded({'id':'r6-native','cwd':str(root/'projects/space1'),'source':'cli','originator':'codex-tui','thread_source':'user'},'session_meta')+encoded({'type':'user_message','message':'通知验收：核对等待和出错提醒。'})+encoded({'type':'task_started','turn_id':'r6-round'}))
 script='exec 3>'+shlex.quote(str(log))+'; cat '+shlex.quote(str(seed))+' >&3; printf "Notification fixture ready\\n"; while IFS= read -r line; do printf "%s\\n" "$line"; done'
 reply=rpc(root,{'Launch':{'id':'notify','space':'s1','argv':[str(executable),'-c',script],'cwd':str(root/'projects/space1'),'cols':100,'rows':30}});assert 'Error' not in reply,reply
elif sys.argv[1]=='tab':
 edit(root,{'PutTab':{'tab':{'id':'notify','space':'s1','title':'通知验收','default_task':None,'closed':False,'layout':{'Leaf':'notify'}}}})
elif sys.argv[1] in ('waiting','error','duplicate'):
 event={'type':'error','message':'R6 故障验收：测试命令退出码 7。'} if sys.argv[1]=='error' else {'type':'request_user_input','message':'R6 通知验收：继续还是暂停？'}
 with log.open('a') as f:f.write(encoded(event))
else:
 w=rpc(root,'Workspace')['Workspace'];sessions=[a for a in w['agent_sessions'] if a['cli_session_id']=='r6-native'];ids=[a['id'] for a in sessions]
 db=sqlite3.connect('file:'+str(root/'runtime/workbench.sqlite')+'?mode=ro',uri=True)
 receipts=[dict(zip(['session_id','fact_key'],row)) for row in db.execute('SELECT session_id,fact_key FROM notification_receipts') if row[0] in ids]
 print(json.dumps({'sessions':sessions,'content':{i:w['content'][i] for i in ids},'receipts':receipts},ensure_ascii=False,indent=2))
