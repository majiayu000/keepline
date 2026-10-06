import { mkdirSync, writeFileSync, chmodSync, existsSync, readFileSync, renameSync, unlinkSync } from 'fs';
import { mkdir, readdir } from 'fs/promises';
import { join } from 'path';
import { getKeeplineHome } from '../../lib/paths.js';
import { getDatabase } from '../../infrastructure/database/sqlite.js';
import { logger } from '../../lib/logger.js';

export interface SpooledHook { id: string; runtime: 'codex' | 'claude-code'; payload: Record<string,unknown>; receivedAt: string }
function scriptSource() {
  // Local disk append happens in the foreground; HTTP delivery is detached and bounded.
  return `#!/usr/bin/env python3
import os,sys,json,uuid,datetime,urllib.request
try:
    runtime,port,home=sys.argv[1:4]
    payload=json.load(sys.stdin)
    event={"id":str(uuid.uuid4()),"runtime":runtime,"payload":payload,"receivedAt":datetime.datetime.now(datetime.timezone.utc).isoformat()}
    root=os.path.join(home,"spool")
    os.makedirs(root,mode=0o700,exist_ok=True)
    events=os.path.join(root,"events")
    os.makedirs(events,mode=0o700,exist_ok=True)
    # One immutable file per event: no shared lock, timeout, or abandoned-lock state.
    temporary=os.path.join(events,event["id"]+".tmp")
    with open(temporary,"x",encoding="utf-8") as f:
        os.chmod(temporary,0o600)
        f.write(json.dumps(event,separators=(",",":"),ensure_ascii=False))
        f.flush();os.fsync(f.fileno())
    os.replace(temporary,os.path.join(events,event["id"]+".json"))
    directory=os.open(events,os.O_RDONLY)
    try: os.fsync(directory)
    finally: os.close(directory)
    pid=os.fork()
    if pid: sys.exit(0)
    os.setsid()
    null=os.open(os.devnull,os.O_RDWR)
    for fd in (0,1,2): os.dup2(null,fd)
    try:
        request=urllib.request.Request("http://127.0.0.1:"+port+"/hook?runtime="+runtime,data=json.dumps(payload).encode(),headers={"Content-Type":"application/json","X-Keepline-Event-Id":event["id"]})
        urllib.request.urlopen(request,timeout=1).close()
    except Exception: pass
    os._exit(0)
except BaseException:
    pass
`;
}
export function installHookSpoolScript(): string {
  const path = join(getKeeplineHome(),'bin','keepline-hook');
  mkdirSync(join(getKeeplineHome(),'bin'),{ recursive: true,mode: 0o700 });
  writeFileSync(path,scriptSource(),{ mode: 0o700 }); chmodSync(path,0o700); return path;
}
export function isHookEventProcessed(id: string): boolean { return !!getDatabase().query('SELECT 1 FROM hook_spool_events WHERE id = ?').get(id); }
export function markHookEventProcessed(id: string) { getDatabase().query('INSERT OR IGNORE INTO hook_spool_events(id,processed_at) VALUES(?,?)').run(id,new Date().toISOString()); }
let replaying = false;
/** Failed records stay retryable without preventing newer events from being delivered. */
export async function replayHookSpool(deliver: (event: SpooledHook) => Promise<boolean | 'reject'>, home = getKeeplineHome()): Promise<number> {
  if (replaying) return 0; replaying = true;
  const root = join(home,'spool'), events = join(root,'events'), rejected = join(root,'rejected');
  let processed = 0;
  const quarantine = async (path: string) => {
    await mkdir(rejected,{ recursive: true,mode: 0o700 });
    renameSync(path,join(rejected,path.split('/').at(-1)!));
  };
  const ingest = async (line: string): Promise<boolean | 'reject'> => {
    let event: SpooledHook;
    try {
      event = JSON.parse(line);
      if (!event.id || !['codex','claude-code'].includes(event.runtime) || !event.payload || typeof event.payload !== 'object') throw new Error('Invalid spool event');
    } catch { logger.warn('Invalid hook spool record quarantined'); return 'reject'; }
    if (isHookEventProcessed(event.id)) return true;
    try {
      const result = await deliver(event);
      if (result === true) { markHookEventProcessed(event.id); processed++; }
      if (result === 'reject') logger.warn('Rejected hook spool record quarantined',{ id: event.id });
      return result;
    } catch (error) { logger.warn('Hook spool event could not be replayed',{ error: error instanceof Error ? error.message : String(error) }); return false; }
  };
  try {
    if (existsSync(events)) for (const name of await readdir(events)) {
      if (!name.endsWith('.json')) continue;
      const path = join(events,name);
      const result = await ingest(readFileSync(path,'utf8'));
      if (result === true) unlinkSync(path);
      else if (result === 'reject') await quarantine(path);
    }
    // Drain existing JSONL spool files too, independently: a poison pending file
    // cannot prevent today's hooks.jsonl or immutable event files from replaying.
    for (const name of ['pending.jsonl','hooks.jsonl']) {
      const path = join(root,name);
      if (!existsSync(path)) continue;
      const remaining: string[] = [], invalid: string[] = [];
      for (const line of readFileSync(path,'utf8').split('\n').filter(Boolean)) {
        const result = await ingest(line);
        if (result === false) remaining.push(line);
        else if (result === 'reject') invalid.push(line);
      }
      if (invalid.length) {
        await mkdir(rejected,{ recursive: true,mode: 0o700 });
        writeFileSync(join(rejected,`${Date.now()}-${name}`),invalid.join('\n')+'\n',{ mode: 0o600 });
      }
      if (remaining.length) writeFileSync(path,remaining.join('\n')+'\n',{ mode: 0o600 }); else unlinkSync(path);
    }
    return processed;
  } finally { replaying = false; }
}
