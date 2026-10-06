import { describe,test,expect } from 'bun:test';
import { join } from 'path';
import { existsSync,readFileSync,writeFileSync,mkdirSync,readdirSync,rmSync } from 'fs';
import { setupLedgerTest } from './helpers.js';
import { getKeeplineHome } from '../../lib/paths.js';
import { installHookSpoolScript,replayHookSpool } from '../../adapters/hook/spool.js';
import { startLifecycleReceiver } from '../../adapters/hook/completion-receiver.js';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';

describe('offline hook spool',() => {
  setupLedgerTest();
  test('a real HTTP 413 cannot strand the next valid permission event',async () => {
    const root = join(getKeeplineHome(),'spool'); mkdirSync(root,{ recursive: true });
    const payload = { session_id: 'spool-valid-123',cwd: '/project',hook_event_name: 'PermissionRequest',tool_name: 'Bash' };
    const event = (id: string,data: unknown) => ({ id,runtime: 'codex',receivedAt: new Date().toISOString(),payload: data });
    writeFileSync(join(root,'pending.jsonl'),JSON.stringify(event('oversized',{ ...payload,tool_input: { command: 'x'.repeat(70*1024) } }))+'\n');
    writeFileSync(join(root,'hooks.jsonl'),JSON.stringify(event('valid-after-large',payload))+'\n');
    const receiver = startLifecycleReceiver(0); const statuses: number[] = [];
    try {
      expect(await replayHookSpool(async e => {
        const response = await fetch(`http://127.0.0.1:${receiver.port}/hook?runtime=codex`,{ method: 'POST',headers: { 'Content-Type': 'application/json','X-Keepline-Event-Id': e.id },body: JSON.stringify(e.payload) });
        statuses.push(response.status); return response.ok ? true : response.status === 413 ? 'reject' : false;
      })).toBe(1);
      expect(statuses).toEqual([413,200]);
      expect(sessionRepository.findBySessionId('codex_spool-valid-123')?.status).toBe('needs_input');
      expect(existsSync(join(root,'pending.jsonl'))).toBe(false);
    } finally { receiver.stop(); }
  });
  test('installed hook exits successfully offline and concurrent JSON events replay once',async () => {
    const home = getKeeplineHome(), script = installHookSpoolScript();
    const started = performance.now();
    await Promise.all(Array.from({ length: 30 },async (_,i) => {
      const child = Bun.spawn([script,'codex','1',home],{ stdin: 'pipe',stdout: 'ignore',stderr: 'pipe' });
      child.stdin.write(JSON.stringify({ hook_event_name: 'PermissionRequest',session_id: `offline-${i}`,cwd: '/project',tool_input: { command: 'echo "你好"' } })); child.stdin.end();
      expect(await child.exited).toBe(0);
    }));
    expect(performance.now()-started).toBeLessThan(15000);
    const lines = readdirSync(join(home,'spool/events')).filter(n => n.endsWith('.json')).map(n => readFileSync(join(home,'spool/events',n),'utf8'));
    expect(lines).toHaveLength(30); lines.forEach(line => expect(JSON.parse(line).payload.tool_input.command).toContain('你好'));
    const ids = new Set<string>();
    const rejectedId = JSON.parse(lines[0]).id;
    expect(await replayHookSpool(async event => { if (event.id === rejectedId) return 'reject'; ids.add(event.id); return true; })).toBe(29);
    expect(ids.size).toBe(29);
    expect(existsSync(join(home,'spool/rejected',`${rejectedId}.json`))).toBe(true);
    expect(await replayHookSpool(async () => true)).toBe(0);

    writeFileSync(join(home,'spool/hooks.jsonl'),lines.slice(1).join('\n')+'\n');
    expect(await replayHookSpool(async () => { throw new Error('Duplicate delivered'); })).toBe(0);
  },20000);
  test('failed delivery survives restart, replayed before new appends',async () => {
    const root = join(getKeeplineHome(),'spool'); mkdirSync(root,{ recursive: true });
    const event = { id: 'retry-1',runtime: 'claude-code',receivedAt: new Date().toISOString(),payload: { session_id: 'retry' } };
    writeFileSync(join(root,'hooks.jsonl'),JSON.stringify(event)+'\n');
    expect(await replayHookSpool(async () => false)).toBe(0);
    expect(existsSync(join(root,'hooks.jsonl'))).toBe(true);
    writeFileSync(join(root,'pending.jsonl'),JSON.stringify({ ...event,id: 'retry-2' })+'\n');
    expect(await replayHookSpool(async () => true)).toBe(2);
    expect(await replayHookSpool(async () => true)).toBe(0);
    expect(existsSync(join(root,'pending.jsonl'))).toBe(false);
  });
  test('abandoned or busy old locks cannot drop hook events',async () => {
    const home = getKeeplineHome(), lock = join(home,'spool/lock'), script = installHookSpoolScript();
    mkdirSync(lock,{ recursive: true });
    try {
      for (const pid of ['',String(process.pid),'99999999']) {
        if (pid) writeFileSync(join(lock,'pid'),pid); else rmSync(join(lock,'pid'),{ force: true });
        const child = Bun.spawn([script,'codex','1',home],{ stdin: 'pipe',stdout: 'ignore',stderr: 'ignore' });
        child.stdin.write('{"session_id":"locked-event"}'); child.stdin.end(); expect(await child.exited).toBe(0);
      }
      expect(await replayHookSpool(async () => true)).toBe(3);
    } finally { rmSync(lock,{ recursive: true,force: true }); }
  });
  test('poison pending records do not block later files; permanent rejects are preserved',async () => {
    const root = join(getKeeplineHome(),'spool'); mkdirSync(root,{ recursive: true });
    const event = (id: string) => ({ id,runtime: 'codex',receivedAt: new Date().toISOString(),payload: { session_id: id } });
    writeFileSync(join(root,'pending.jsonl'),'invalid json\n'+JSON.stringify(event('poison'))+'\n');
    writeFileSync(join(root,'hooks.jsonl'),JSON.stringify(event('healthy'))+'\n');
    const delivered: string[] = [];
    expect(await replayHookSpool(async e => { if (e.id === 'poison') return false; delivered.push(e.id); return true; })).toBe(1);
    expect(delivered).toEqual(['healthy']);
    writeFileSync(join(root,'hooks.jsonl'),JSON.stringify(event('later'))+'\n');
    expect(await replayHookSpool(async e => e.id === 'poison' ? 'reject' : true)).toBe(1);
    expect(existsSync(join(root,'pending.jsonl'))).toBe(false);
    expect(readdirSync(join(root,'rejected')).length).toBeGreaterThan(0);
  });
});
