import json,pathlib,collections
j=json.loads(pathlib.Path('/tmp/kl-noise-audit.json').read_text());out=pathlib.Path('specs/terminal-workbench/evidence/overview-noise-20261021')
samples=[('449f386e','你帮我看看这个裤','普通原话'),('92e4bd71','你看看这个是什么？','去掉 X 网页粘贴，保留页尾请求'),('1fc5e267','你帮我看看这个mac上的codex','原话和追加要求'),('290034d6','我希望你帮我看看我这台Mac','用户的整理要求'),('c80e1c96','归并总方案','用户文档引用及要求'),('70f201cf','stealthprint：','项目名是用户原话的一部分'),('9ada561a','你帮我看看这个项目','用户设计要求'),('398c22b7','我想做一些升级','用户整理要求'),('f1a91e23','你看看这里面的视频','用户视频分析要求'),('4ecfe5e4','你看看我的本地的codex','用户调研要求'),('aa6f0c0f','take a deep dive','英语原话'),('849d2eb9','https://github.com/shader-effects-inc/shaders','用户链接和问题'),('a419f76e','你看这是什么呢','带属性 pasted_content 删除后保留尾部问题'),('8363caa6','npm i @lucasmarkes/hairline','用户命令引用保留，非 command-name 注入'),('76ba5c82','我想要做一个动画','用户动画目标'),('86114e6d','https://hermes-agent.nousresearch.com/docs/user-stories','用户链接问题'),('62602866','你好','初始简短问候原话'),('01a1580e','持续监控 Agentic AI','原话，不把自动打开页面包装当用户要求'),('01a1590e','只回复两个字母 OK2','完整短会话，真实首条要求'),('01a1590d','只回复两个字母 OK','完整短会话，真实首条要求')]
def strings(v):
 if isinstance(v,str):yield v
 elif isinstance(v,list):
  for x in v:yield from strings(x)
 elif isinstance(v,dict):
  for x in v.values():yield from strings(x)
def raw(p,o):
 with open(p,'rb') as f:f.seek(o);return json.loads(f.readline(262145))
rows=[]
for pref,expect,note in samples:
 c=next(c for c in j['cards'] if c['cli_id'].startswith(pref) and c['content']['interactive'] and not c['content']['hidden_reason'] and c['content']['asks'])
 t=c['content'];assert t['first_ask_known'];assert t['title'].startswith(expect),(pref,t['title']);assert c['here']['ask'].startswith(expect)
 quotes=[]
 for q in t['asks']:
  vals=list(strings(raw(c['path'],q['source']['offset'])));plain=q['text'].removesuffix('…')
  assert all(any(line.strip() in v for v in vals) for line in plain.splitlines() if line.strip()),(pref,plain)
  quotes.append({'text':q['text'],'offset':q['source']['offset']})
 rows.append({'id':c['cli_id'],'runtime':c['runtime'],'path':c['path'].replace('/Users/lifcc','~'),'title':t['title'],'ask':c['here']['ask'],'quotes':quotes,'partial':t['partial_history'],'from_offset':t['from_offset'],'judgement':note,'title_correct':True,'ask_correct':True})
counts=collections.Counter(c['content']['hidden_reason'] or ('交互式' if c['content']['interactive'] else '缺少交互入口证据') for c in j['cards'])
# Keep full identifiers but no prompt bodies for excluded/automated sessions.
hidden=[{'id':c['cli_id'],'reason':c['content']['hidden_reason'] or '缺少交互入口证据','runtime':c['runtime']} for c in j['cards'] if not c['content']['interactive'] or c['content']['hidden_reason']]
(out/'audit.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2));(out/'classification.json').write_text(json.dumps({'counts':counts,'hidden':hidden,'index':{k:v for k,v in j.items() if k!='cards'}},ensure_ascii=False,indent=2))
lines=['# 20 个真实会话：标题与“你要它”核对','', '核对者：本轮实现代理逐条对照 JSONL 来源；不是用户手测或独立人工签收。脚本用于偏移定位、原文包含检查与复现，语义判断见每条说明。只评估已读取内容中的真实用户文字；有界头/尾未覆盖的历史不声称恢复。','', '| # | 会话 | 标题 | 你要它 | 核对点 |','|---|---|---|---|---|']
for i,r in enumerate(rows,1):lines.append(f'| {i} | {r["runtime"]} {r["id"][:8]} | 通过 | 通过 | {r["judgement"]} |')
for i,r in enumerate(rows,1):
 lines += ['',f'## {i}. {r["title"]}','',f'源：`{r["path"]}`；尾段起点 {r["from_offset"]}；历史不完整：{r["partial"]}。','',f'标题：{r["title"]}','','你要它：',*['> '+x if x else '>' for x in r['ask'].splitlines()],'',f'核对：{r["judgement"]}。','', '原话字节偏移：'+', '.join(str(q['offset']) for q in r['quotes'])+'。']
(out/'audit.md').write_text('\n'.join(lines)+'\n');print('reviewed',len(rows),'counts',dict(counts))
# Native snapshot chooses only reviewed work-related sessions plus known excluded examples.
chosen={r['id'] for r in rows}
auto=[c for c in j['cards'] if any(c['cli_id'].startswith(p) for p in ['b19c5eb6','4242896d','32255d82','9617e572','b870ce40','8e0ed805','f4a31fea'])]
for reason in ['Codex 子代理','Claude 子代理 / sidechain','Codex exec 非交互']:
 auto += [next(c for c in j['cards'] if c['content']['hidden_reason']==reason)]
chosen.update(c['cli_id'] for c in auto)
(out/'native-selection.json').write_text(json.dumps([{'id':c['cli_id'],'path':c['path']} for c in j['cards'] if c['cli_id'] in chosen],ensure_ascii=False,indent=2))
