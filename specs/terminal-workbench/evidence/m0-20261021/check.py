"""Read-only comparison of each recent Codex header to the imported visibility projection."""
import json, collections
from pathlib import Path
out = Path(__file__).parent
report = json.loads(Path('/tmp/kl-m0-final.json').read_text())
warm = json.loads(Path('/tmp/kl-m0-final-warm.json').read_text())
counts = collections.Counter()
rows = []
for card in report['cards']:
    c = card['content']
    visible = c['interactive'] and not c['hidden_reason']
    if visible:
        counts[card['runtime'] + '_visible'] += 1
        counts[card['runtime'] + '_first_known'] += int(c['first_ask_known'])
    if card['runtime'] != 'Codex':
        continue
    with open(card['path'], 'rb') as f:
        m = json.loads(f.readline(64 * 1024))['payload']
    expected = m.get('source') in ('cli', 'vscode') and m.get('thread_source') in (None, 'user') and not m.get('is_exec') and 'exec' not in m.get('originator', '') and 'sdk' not in m.get('originator', '')
    assert expected == visible, (card['cli_id'], c['hidden_reason'])
    rows.append({'id':card['cli_id'],'cwd':card['cwd'],'source':m.get('source') if isinstance(m.get('source'),str) else 'subagent','thread_source':m.get('thread_source'),'originator':m.get('originator'),'visible':visible,'first_known':c['first_ask_known']})
summary={'counts':dict(counts),'index':{k:v for k,v in report.items() if k!='cards'},'warm':{k:v for k,v in warm.items() if k!='cards'},'codex':rows}
(out/'results.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({k:v for k,v in summary.items() if k!='codex'},ensure_ascii=False))
