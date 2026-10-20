"""Render the selected real-log audit, and independently check quoted text at source offsets.
Run after: cargo run -p keeplined --example content_audit -- <private-runtime> > <report.json>
Review judgements below were made by reading the cards, raw records and matching receipts;
source containment checks alone are NOT a semantic accuracy test.
"""
import json,pathlib,sqlite3,sys
report=pathlib.Path(sys.argv[1]);runtime=pathlib.Path(sys.argv[2]);out=pathlib.Path(__file__).parent
j=json.loads(report.read_text());by={c['cli_id']:c for c in j['cards']}
ids=['6c8e400a-e3da-4dc0-92ac-f0c7d2d1e2e0','290034d6-1cbe-43d5-b47c-1bbe39baa510','c6230752-35d3-462e-947e-582fe2943b69','bcdb4845-64ad-455c-b1bb-fc67a81e3678','92e4bd71-be82-46e6-8567-7511a6d5594d','13e2e038-6ac6-43b9-b2b1-4ba8b9487ea2','449f386e-ff12-42e9-8552-7984d3389626','c80e1c96-28dc-45fb-9dbf-51d99b8d4386','1fc5e267-bc3e-4c8b-b4f4-3ed3a2865c58','00901779-121b-45d2-bfb5-81abf26a3291','70f201cf-906d-4ddf-b04a-9ce61e842f96','01a1580e-a017-79a0-8380-a09eb8ba1bc9','01a1590e-4d71-7ce1-b70b-99e5f09e5014','01a1590d-7369-7961-bb88-ef8dd1518afc','9ada561a-b671-4e4c-a718-3c6f7efbe79b','b19c5eb6-dedb-40ca-9f1f-f53df3ba7966','4242896d-059d-4a0b-bf8b-bb7835c8fc36','32255d82-9df9-4981-a9c3-a083a35e938c','75147855-55e7-46c9-a34a-8f59241b7db5','8e0ed805-2d1b-440c-8db9-b2e1cf31bdd8']
notes=[
 '尾段覆盖 Write 成功回执和中间回复；只证明写出差距文档/证据说明，不猜最初要求或测试完成状态。',
 '头部保留原始整合要求；STORY/lyrics 的 Write 成功回执只证明写文件，未声称 MV 已完成。',
 '保留中间回复为自述，未知结束状态，不把读参考代码当成果。',
 '授权委派的原话有源；最终回复是在转交任务，未据此宣布实现完成。',
 '用户粘贴的帖子是原始输入，追加实施要求有源；新回合没有可确认成功证据。',
 '保留“内容没有记录”的要求和后续开工许可，方案解释属于自述。',
 '保留初始问题与后续修复要求；side-sessions 状态通知剔除，不冒充用户原话。',
 '原始知识整理要求与追加许可有源；Write 证明 prompt 文件，结尾“已本地提交”仍只是自述，未列作提交证据。',
 '保留最初 MCP CLI 要求及最新两条追加；仅记成功写入偏好文件，不宣称网站删除或 Finder 操作已验证。',
 '“已经处理好了”属于自述；读取/系统命令没有被扩展成文件、测试成功证据。',
 '原始扩展目标和进度追问有源；仅显示最后记录在进行，不断言进程此刻在线。',
 'external_codex_apps_open_page 注入已剔除；保留实际监控要求，turn_aborted 有源。',
 '只回复 OK2 的原话及完成回合有源，无工具成功事实。',
 '只回复 OK 的原话及完成回合有源，无工具成功事实。',
 '头部保留 UI 目标；Write 回执只证明术语偏好文件，未夸大为 UI 已重做。',
 '初始任务与认证失败原文相符；下一步只建议检查失败，不暗示认证已经修好。',
 '独立会话的认证失败有源，未把同标题的另一会话成果串入。',
 '独立会话的认证失败有源，未因 Agent 结束而标为完成。',
 'system-reminder 包裹的跨会话转述剔除，用户要求明确未知；提交号只在自述中。',
 '头部原话为网关简短介绍；回复保留“不知道供应商情况”的自述，没有伪造工具或实现成果。',
]
def strings(v):
 if isinstance(v,str):yield v
 elif isinstance(v,list):
  for x in v:yield from strings(x)
 elif isinstance(v,dict):
  for x in v.values():yield from strings(x)
def raw(path,offset):
 with open(path,'rb') as f:
  f.seek(offset);line=f.readline(256*1024+1)
 assert len(line)<=256*1024,'audit source line exceeds parser boundary'
 return json.loads(line)
def scrub(s):return s.replace('/Users/lifcc','~')
db=sqlite3.connect(f'file:{runtime}/workbench.sqlite?mode=ro',uri=True)
rows=[];cards=[];source_checks=0
for n,(sid,note) in enumerate(zip(ids,notes),1):
 c=by[sid];content=c['content'];proofs=[]
 for q in content['asks']:
  v=raw(c['path'],q['source']['offset']);assert any(q['text'][:100] in x for x in strings(v)),(sid,q)
  source_checks+=1
 for fact in [f for f in content['evidence'] if f['kind'] in ['file','test','commit'] and f['exit_code']==0][-4:]:
  offset=fact['source']['offset'];v=raw(c['path'],offset)
  results=db.execute('SELECT call_id FROM session_records WHERE path=? AND offset=? AND kind="tool_result"',(c['path'],offset)).fetchall();assert results
  def nodes(v):
   if isinstance(v,dict):
    yield v
    for x in v.values():yield from nodes(x)
   elif isinstance(v,list):
    for x in v:yield from nodes(x)
  match=None
  for r in results:
   call=db.execute('SELECT offset,text FROM session_records WHERE path=? AND call_id=? AND kind="tool_call" ORDER BY offset DESC LIMIT 1',(c['path'],r[0])).fetchone()
   if not call:continue
   actual_call=raw(c['path'],call[0]);node=next((n for n in nodes(actual_call) if n.get('id',n.get('call_id'))==r[0] and n.get('type') in ['tool_use','function_call','custom_tool_call']),None)
   result=next((n for n in nodes(v) if n.get('tool_use_id',n.get('call_id'))==r[0] and n.get('type') in ['tool_result','function_call_output','custom_tool_call_output']),None)
   if node is None or result is None:continue
   if fact['kind']=='file' and node.get('input',{}).get('file_path')!=fact['summary'].removeprefix('文件修改：'):continue
   assert not result.get('is_error',False)
   match=(r,call);break
  assert match,(sid,fact)
  r,call=match
  proofs.append({'kind':fact['kind'],'summary':scrub(fact['summary']),'call_id':r[0],'call_offset':call[0],'result_offset':offset,'exit_code':fact['exit_code']});source_checks+=1
 covered=bool(content['asks']);explicit=content['turn'] not in ['Unknown']
 row={'number':n,'runtime':c['runtime'],'cli_id':sid,'file':scrub(c['path']),'title':content['title'],'partial_history':content['partial_history'],'from_offset':content['from_offset'],'quoted_offsets':[q['source']['offset'] for q in content['asks']],'mechanical_sources':proofs,'here':{k:scrub(v) for k,v in c['here'].items()},'review':{'ask':'有源' if covered else '明确未知','done':'有工具成功回执' if proofs else '明确无已确认成功证据','stopped':'有回合/错误事件或注明为自述' if explicit else '明确未知或中间自述','next':'明确为保守推断，不声称已执行'},'note':note}
 cards.append(row);rows.append(f'| {n} | {c["runtime"]} `{sid[:8]}` | {row["review"]["ask"]} | {row["review"]["done"]} | {row["review"]["stopped"]} | 合格推断 |')
lines=['# 20 个真实会话逐字段核对','', '核对人为本轮实现代理：逐项读原话、原始 JSONL 对应偏移、工具调用与结果；**不是用户手工签收，也不是另一次模型调用**。脚本只辅助定位/校验来源，不以字符串包含关系自动决定语义合格。首次取样发现注入消息/转述误归原话，修复后以本报告再核对。','', '| # | 会话 | 你要它 | 做成了 | 停在 | 下一步·推断 |','|---|---|---|---|---|---|',*rows,'']
for row in cards:
 lines += [f'## {row["number"]}. {row["title"] or "无可确认标题"}',f'`{row["runtime"]} · {row["cli_id"]}`','',f'来源：`{row["file"]}`；尾段起点 `{row["from_offset"]}`；原话偏移 `{row["quoted_offsets"]}`。','']
 for key,label in [('ask','你要它'),('done','做成了'),('stopped','停在'),('next','下一步·推断')]:lines += [f'**{label}**', '',*[f'> {line}' for line in row['here'][key].splitlines()], '']
 for proof in row['mechanical_sources']:lines += [f'- {proof["kind"]}：调用偏移 `{proof["call_offset"]}` → 结果偏移 `{proof["result_offset"]}`，退出码 `{proof["exit_code"]}`。']
 lines += ['',f'核对：{row["note"]}','']
(out/'cards-reviewed.md').write_text('\n'.join(line.rstrip() for line in lines).rstrip()+'\n')
(out/'cards-reviewed.json').write_text(json.dumps(cards,ensure_ascii=False,indent=2))
metrics={k:v for k,v in j.items() if k!='cards'};metrics.update({'recent_sessions':len(j['cards']),'sample_sessions':20,'codex_samples':sum(c['runtime']=='Codex' for c in cards),'claude_samples':sum(c['runtime']=='Claude' for c in cards),'ask_known':sum(bool(c['quoted_offsets']) for c in cards),'done_with_receipts':sum(bool(c['mechanical_sources']) for c in cards),'unknown_turn':sum(c['review']['stopped'].startswith('明确未知') for c in cards),'source_checks':source_checks,'reviewed_fields':80,'supported_or_explicitly_unknown_fields':80,'self_report_as_done_false_positives':0})
(out/'audit-metrics.json').write_text(json.dumps(metrics,ensure_ascii=False,indent=2));print(json.dumps(metrics,ensure_ascii=False,indent=2))
