import { expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join, resolve } from 'path';

test('filesystem changes wake a long-interval service and notify both ledger and session clients after reconciliation',async () => {
  const root = mkdtempSync(join(tmpdir(),'keepline-live-fixture-'));
  mkdirSync(join(root,'.codex/sessions'),{ recursive: true });mkdirSync(join(root,'.claude/projects'),{ recursive: true });
  const script = `
    import { appendFileSync,readFileSync,writeFileSync,mkdirSync } from 'fs';
    import { join } from 'path';
    import { startKeeplineService } from ${JSON.stringify(resolve('src/services/service-runtime.ts'))};
    const root = process.env.HOME, count = join(root,'count');
    const scan = [process.execPath,'-e', 'import {appendFileSync} from "fs";appendFileSync('+JSON.stringify(count)+',"x");console.log('+JSON.stringify('__KEEPLINE_SERVICE_SCAN__'+JSON.stringify({runtimeScan:[],pendingDispatches:0}))+')'];
    const service = await startKeeplineService({port:0,hookPort:0,scanIntervalMs:60000,scanCommand:scan});
    const origin = 'http://127.0.0.1:'+service.server.port;
    let ws;
    try {
      const auth = await (await fetch(origin+'/api/v1/auth/local',{method:'POST'})).json();
      const token = auth.data.token, events = [], failures = [];
      ws = new WebSocket(origin.replace('http:','ws:')+'/ws?token='+encodeURIComponent(token));
      ws.onmessage = event => {
        const message = JSON.parse(event.data);events.push(message.type);
        if(message.type==='ledger:update') failures.push(fetch(origin+'/api/ledger',{headers:{Authorization:'Bearer '+token}}).then(r=>r.status));
      };
      await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=()=>reject(new Error('WebSocket did not connect'));});
      const until = async predicate => { const end=Date.now()+5000;while(!predicate()){if(Date.now()>end)throw new Error('No timely notification');await Bun.sleep(25);} };
      await until(()=>events.includes('sync:complete'));
      const baseline=readFileSync(count,'utf8').length;events.length=0;
      const started=Date.now();
      // A new nested directory is discovered without restarting any watcher.
      mkdirSync(join(root,'.codex/sessions/new'),{recursive:true});
      writeFileSync(join(root,'.codex/sessions/new/rollout-test.jsonl'),'{}\\n');
      await until(()=>events.includes('ledger:update')&&events.includes('sync:complete'));
      const latencyMs=Date.now()-started;
      const statuses=await Promise.all(failures);
      if(statuses.some(s=>s!==200))throw new Error('Broadcast preceded startup gate');
      if(readFileSync(count,'utf8').length<=baseline)throw new Error('No scan triggered');
      console.log(JSON.stringify({latencyMs,statuses,ledger:true,sessions:true}));
    } finally {ws?.close();await service.stop();}
  `;
  try {
    const child = Bun.spawn([process.execPath,'-e',script],{ env: { ...process.env,HOME: root,KEEPLINE_HOME: join(root,'home'),KEEPLINE_PROJECT_ROOTS: join(root,'.claude/projects') },stdout: 'pipe',stderr: 'pipe' });
    const [out,err,code] = await Promise.all([new Response(child.stdout).text(),new Response(child.stderr).text(),child.exited]);
    expect(code,err).toBe(0);const result = JSON.parse(out.trim().split('\n').find(line => line.startsWith('{'))!);
    expect(result.ledger).toBe(true);expect(result.sessions).toBe(true);expect(result.latencyMs).toBeLessThan(5000);expect(result.statuses.every((s: number) => s === 200)).toBe(true);
  } finally { rmSync(root,{ recursive: true,force: true }); }
},15000);
