import { chmodSync, mkdirSync, readFileSync, writeFileSync } from 'fs';
import { join } from 'path';
import type { LedgerDetail } from '../../domain/ledger/types.js';
import type { ClaudeSettings } from '../hook/types.js';
import { getKeeplineHome } from '../../lib/paths.js';
import { isValidSessionId } from '../../lib/session-id.js';
import { logger } from '../../lib/logger.js';

const MARKER = 'KEEPLINE_CONTEXT_MARKER=keepline-claude-context';
const ORIGINAL = 'claude-statusline-original.json';
interface StatusLine { type: 'command'; command: string; [key: string]: unknown }

function scriptSource() {
  // Keep stdout and stdin for the user's status line; collection errors go to stderr.
  return `#!/usr/bin/env python3
import datetime,json,os,re,sys,tempfile
home=sys.argv[1]
raw=sys.stdin.buffer.read()
with open(os.path.join(home,"${ORIGINAL}"),encoding="utf-8") as source:
    original=json.load(source)
temporary=None
try:
    payload=json.loads(raw)
    session=payload["session_id"]
    if not isinstance(session,str) or not re.fullmatch(r"[a-zA-Z0-9_-]{8,64}",session):
        raise ValueError("Invalid session ID")
    snapshot={"sessionId":session,"at":datetime.datetime.now(datetime.timezone.utc).isoformat(),"contextWindow":payload.get("context_window")}
    root=os.path.join(home,"context","claude-code")
    os.makedirs(root,mode=0o700,exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w",dir=root,prefix=session+"-",suffix=".tmp",delete=False,encoding="utf-8") as target:
        temporary=target.name
        json.dump(snapshot,target,separators=(",",":"),ensure_ascii=False)
    os.replace(temporary,os.path.join(root,session+".json"))
    temporary=None
except (OSError,ValueError,TypeError,KeyError) as error:
    print("Keepline context capture failed: "+str(error),file=sys.stderr)
finally:
    if temporary is not None:
        try: os.unlink(temporary)
        except OSError as error: print("Keepline context cleanup failed: "+str(error),file=sys.stderr)
if original is not None:
    with tempfile.TemporaryFile() as source:
        source.write(raw)
        source.seek(0)
        os.dup2(source.fileno(),0)
        os.execv("/bin/sh",["/bin/sh","-c",original["command"]])
`;
}

/** Wrap the existing command once, preserving every other status-line setting. */
export function installClaudeStatusline(settings: ClaudeSettings, home = getKeeplineHome()): boolean {
  const current = settings.statusLine as StatusLine | undefined;
  if (current !== undefined && (!current || current.type !== 'command' || typeof current.command !== 'string')) {
    throw new Error('Cannot wrap an unsupported Claude statusLine configuration');
  }
  const bin = join(home, 'bin');
  const script = join(bin, 'keepline-claude-statusline');
  mkdirSync(bin, { recursive: true, mode: 0o700 });
  writeFileSync(script, scriptSource(), { mode: 0o700 });
  chmodSync(script, 0o700);
  if (current?.command.startsWith(`${MARKER} `)) return false;
  writeFileSync(join(home, ORIGINAL), JSON.stringify(current ?? null), { mode: 0o600 });
  const quote = (value: string) => "'" + value.replace(/'/g, "'\"'\"'") + "'";
  settings.statusLine = { ...current, type: 'command', command: `${MARKER} ${quote(script)} ${quote(home)}` };
  return true;
}

export function uninstallClaudeStatusline(settings: ClaudeSettings, home = getKeeplineHome()): boolean {
  const current = settings.statusLine as StatusLine | undefined;
  if (!current?.command?.startsWith(`${MARKER} `)) return false;
  const original = JSON.parse(readFileSync(join(home, ORIGINAL), 'utf8')) as StatusLine | null;
  if (original === null) delete settings.statusLine;
  else settings.statusLine = { ...current, command: original.command };
  return true;
}

/** Read the latest snapshot independently of the transcript's computation cache. */
export function readClaudeContextUsage(sessionId: string, transcriptUsage?: LedgerDetail['contextUsage'], home = getKeeplineHome()): LedgerDetail['contextUsage'] {
  if (!isValidSessionId(sessionId)) return undefined;
  try {
    const snapshot = JSON.parse(readFileSync(join(home, 'context', 'claude-code', `${sessionId}.json`), 'utf8'));
    if (snapshot.sessionId !== sessionId || !Number.isFinite(Date.parse(snapshot.at))) return undefined;
    if (transcriptUsage && Date.parse(transcriptUsage.at) > Date.parse(snapshot.at)) return transcriptUsage;
    const windowTokens = snapshot.contextWindow?.context_window_size;
    const usage = snapshot.contextWindow?.current_usage;
    if (!usage || typeof windowTokens !== 'number' || !Number.isFinite(windowTokens) || windowTokens <= 0) return undefined;
    // Claude's context percentage counts input, including cache reads/writes, not output.
    const tokens = [usage.input_tokens, usage.cache_creation_input_tokens, usage.cache_read_input_tokens];
    if (!tokens.every(value => typeof value === 'number' && Number.isFinite(value) && value >= 0)) return undefined;
    const usedTokens = tokens.reduce((total, value) => total + value, 0);
    if (!Number.isFinite(usedTokens)) return undefined;
    return { usedTokens, windowTokens, at: snapshot.at };
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return transcriptUsage;
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') {
      logger.warn('Could not read Claude context snapshot', { sessionId, error: error instanceof Error ? error.message : String(error) });
    }
    return undefined;
  }
}
