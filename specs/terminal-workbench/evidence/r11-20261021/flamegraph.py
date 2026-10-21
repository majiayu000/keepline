"""Sanitize xctrace time-profile table into a standalone CPU flamegraph (no trace env)."""
import collections,html,json,pathlib,sys,xml.etree.ElementTree as E
root=E.parse(sys.argv[1]);ids={n.get('id'):n for n in root.iter() if n.get('id')}
def deref(n):return ids[n.get('ref')] if n.get('ref') else n
stacks=collections.Counter();inc=collections.Counter();leaf=collections.Counter()
for row in root.iter('row'):
 if row.find('tagged-backtrace') is None: continue # profiler lost samples have no stack
 bt=deref(deref(row.find('tagged-backtrace')).find('backtrace'))
 names=tuple(deref(f).get('name','unknown') for f in bt)
 weight=int(deref(row.find('weight')).text)/1e6
 stacks[tuple(reversed(names))]+=weight;leaf[names[0]]+=weight
 for name in set(names):inc[name]+=weight
total=sum(stacks.values());tree={'n':'all CPU samples','w':0,'c':{}}
for names,w in stacks.items():
 node=tree;node['w']+=w
 for name in names:
  node=node['c'].setdefault(name,{'n':name,'w':0,'c':{}});node['w']+=w
height=(max(map(len,stacks))+1)*20;svg=[]
def draw(node,x,y,width):
 if width<.3:return
 name=node['n'];color=f'hsl({sum(map(ord,name))%55},75%,68%)'
 tip=html.escape(f'{name}: {node["w"]:.1f} ms ({node["w"]/total*100:.1f}%)')
 svg.append(f'<g><title>{tip}</title><rect x="{x:.2f}" y="{height-y-20}" width="{width:.2f}" height="19" fill="{color}"/>')
 if width>40:svg.append(f'<text x="{x+3:.2f}" y="{height-y-6}" font-size="11">{html.escape(name[:int(width/7)])}</text>')
 svg.append('</g>');offset=x
 for child in sorted(node['c'].values(),key=lambda c:c['n']):
  w=width*child['w']/node['w'];draw(child,offset,y+20,w);offset+=w
draw(tree,0,0,1600)
pathlib.Path(sys.argv[2]).write_text('<!doctype html><meta charset="utf-8"><title>Time Profiler CPU flamegraph</title><h1>Time Profiler · CPU samples</h1><p>Root at bottom; width = sampled CPU, not wall time. Hover for symbols. Samples: '+str(total)+' ms. No process environment exported.</p><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1600 '+str(height)+'">'+''.join(svg)+'</svg>')
pathlib.Path(sys.argv[2]).with_suffix('.json').write_text(json.dumps({'sampled_ms':total,'inclusive_ms':inc.most_common(),'self_ms':leaf.most_common()},indent=2))
