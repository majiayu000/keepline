import type { TranscriptFact, ToolEvidence } from '../domain/ledger/types.js';
import { directCommandWords, isSedWriteCommand, sedInPlaceFiles } from '../domain/ledger/sed-in-place.js';
import { extractTaskPrompt } from '../domain/session/index.js';

function record(value: unknown): Record<string, any> {
  return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, any> : {};
}
function textContent(value: unknown): string {
  if (typeof value === 'string') return value;
  if (Array.isArray(value)) return value.map(v => typeof v === 'string' ? v : record(v).text ?? '').join('\n');
  return JSON.stringify(value) ?? '';
}
function parseInput(value: unknown): unknown {
  if (typeof value !== 'string') return value;
  try { return JSON.parse(value); } catch { return { input: value }; }
}
// Substrings of large outputs can retain their entire backing transcript record.
function detached(text: string): string { return Buffer.from(text).toString(); }
function ripgrepRunsPreprocessor(words: string[]): boolean {
  let enabled = false;
  for (let index = 1; index < words.length; index++) {
    const option = words[index];
    if (option === '--') break;
    if (option === '--no-pre') { enabled = false; continue; }
    if (option === '--pre') { enabled = words[++index] !== ''; continue; }
    if (option.startsWith('--pre=')) { enabled = option.slice('--pre='.length) !== ''; continue; }
    // Values are data even when they resemble --pre or --no-pre. Long option
    // values attached with =, and short values attached in a cluster, stay here.
    if (/^--(?:after-context|before-context|color|colors|context|context-separator|dfa-size-limit|encoding|engine|field-context-separator|field-match-separator|file|generate|glob|hostname-bin|hyperlink-format|iglob|ignore-file|max-columns|max-count|max-depth|maxdepth|max-filesize|path-separator|pre-glob|regex-size-limit|regexp|replace|sort|sortr|threads|type|type-add|type-clear|type-not)$/.test(option)) {
      index++; continue;
    }
    if (option.startsWith('-') && !option.startsWith('--')) {
      for (let flag = 1; flag < option.length; flag++) {
        if (!'ABCEefgMmdrjtT'.includes(option[flag])) continue;
        if (flag + 1 === option.length) index++;
        break;
      }
    }
  }
  return enabled;
}
export function isMutatingTool(name: string, input: unknown): boolean {
  const data = record(input);
  const command = data.command ?? data.cmd;
  if (isSedWriteCommand(command)) return true;
  const words = typeof command === 'string' ? directCommandWords(command) : undefined;
  const executable = words?.[0]?.split('/').at(-1);
  if (executable === 'find' && words) {
    for (let index = 1; index < words.length; index++) {
      const option = words[index];
      if (/^-(?:delete|exec|execdir|ok|okdir|fprint|fprint0|fprintf|fls)$/.test(option)) return true;
      // A predicate's pattern, path, or printf format is data, not an action.
      if (/^-(?:name|iname|path|ipath|wholename|iwholename|regex|iregex|lname|ilname|printf|files0-from|samefile|newer|anewer|cnewer|newer[aBcm][aBcmt]|type|xtype|uid|gid|user|group|inum|links|perm|size|used|amin|atime|cmin|ctime|mmin|mtime|context|fstype|maxdepth|mindepth|regextype|D)$/.test(option)) index++;
    }
  }
  if (executable === 'rg' && words && ripgrepRunsPreprocessor(words)) return true;
  if (/(?:^|[_.])(?:wait|sleep|read_file|read_text_file|read|list|list_directory|list_agents|search|find|view_image|getState|get_goal|clock__curr_time)$/.test(name) || /^(Read|Glob|Grep|LS|TodoWrite)$/.test(name)) return false;
  if (/write_stdin$/.test(name) && !data.chars) return false;
  if (typeof command === 'string') {
    // A read followed by a write must remain a step. Shell substitution is not read-only.
    if (!words) return true;
    if (executable === 'git') {
      let index = 1;
      while (index < words.length) {
        if (words[index] === '--no-pager') index++;
        else if (words[index] === '-C' && words[index + 1] !== undefined) index += 2;
        else if (words[index].startsWith('-C') && words[index].length > 2) index++;
        else break;
      }
      const args = words.slice(index + 1);
      const separator = args.indexOf('--');
      const options = separator < 0 ? args : args.slice(0, separator);
      if (options.some(option => option === '--output' || option.startsWith('--output='))) return true;
      let externalDiff = false, textconv = false;
      for (let optionIndex = 0; optionIndex < options.length; optionIndex++) {
        const option = options[optionIndex];
        if (option === '--ext-diff') externalDiff = true;
        else if (option === '--no-ext-diff') externalDiff = false;
        else if (option === '--textconv') textconv = true;
        else if (option === '--no-textconv') textconv = false;
        // Pattern/format operands can look like helper flags without enabling them.
        else if (/^(?:-G|-S|--grep|--author|--committer|--format|--since|--until|--after|--before|--date|--diff-filter|--find-object|--max-count|-n)$/.test(option)) optionIndex++;
      }
      if (externalDiff || textconv) return true;
      if (words[index] === 'branch') return !(args.length === 1 && args[0] === '--show-current');
      return !/^(?:status|diff|log|show|ls-files)$/.test(words[index] ?? '');
    }
    return !/^(?:sleep|cat|ls|rg|grep|head|tail|pwd|stat|find|sed)$/.test(executable ?? '');
  }
  if (/functions.exec$/.test(name) && typeof data.input === 'string') {
    const calls = [...data.input.matchAll(/tools\.([\w]+)\s*\(/g)].map(m => m[1]);
    const commands = [...data.input.matchAll(/(?:cmd|command)\s*:\s*["']([^"']+)["']/g)].map(m => m[1]);
    if (calls.length && calls.every(n => n === 'exec_command') && commands.length === calls.length) return commands.some(cmd => isMutatingTool('exec_command',{ cmd }));
    if (calls.length && calls.every(n => n === 'write_stdin') && !/chars\s*:\s*["'][^"']+/.test(data.input)) return false;
    return calls.length === 0 || calls.some(n => !/^(?:read_|list_|get_|clock__|web__)/.test(n));
  }
  return true;
}
function outputEvidence(output: unknown, input: unknown, name: string, resultCode?: number, implicit = false): { exitCode?: number; outputHead: string; facts: ToolEvidence[] } {
  const out = textContent(output);
  let obj = record(output);
  if (typeof output === 'string') { try { obj = record(JSON.parse(output)); } catch { /* plain terminal result */ } }
  // Structured exec results contain the real newlines inside output. Scanning
  // their JSON encoding can miss a failure after a literal escaped newline.
  const observed = typeof obj.output === 'string' ? obj.output : out;
  const code = obj.exit_code ?? obj.exitCode ?? record(obj.metadata).exit_code;
  const codes = [...out.matchAll(/(?:Process exited with code|exit[_ ]code[^0-9-]{0,8}|Exit code:)\s*(-?\d+)/gi)].map(m => Number(m[1]));
  // A wrapper may contain several command results. Any failure prevents whole-call success.
  const structuredCode = typeof code === 'number' ? code : obj.is_error === true ? 1 : undefined;
  // Host status and numeric result fields outrank ordinary text in stdout.
  // Claude success remains implicit only relative to another structured status.
  const reportedCode = structuredCode ?? codes.find(c => c !== 0) ?? codes.at(-1);
  // An MCP exec tool can succeed while its child command fails. Its host flag
  // is only a fallback; explicit process receipts still determine completion.
  const execFallback = implicit && resultCode === 0 && /(?:^|[_.])exec_command$/.test(name);
  const exitCode = execFallback ? reportedCode ?? resultCode : typeof resultCode === 'number'
    ? implicit && resultCode === 0 && structuredCode !== undefined ? structuredCode : resultCode
    : reportedCode;
  const facts: ToolEvidence[] = [];
  const data = record(input);
  const embedded = typeof data.input === 'string' ? [...data.input.matchAll(/(?:cmd|command)\s*:\s*["']([^"']+)["']/g)].map(m => m[1]) : [];
  const command = data.command ?? data.cmd ?? (embedded.length === 1 ? embedded[0] : undefined);
  if (command && exitCode !== undefined) facts.push({ kind: 'command', value: String(command), exitCode });
  const counts = [...observed.matchAll(/\b(\d+)\s+(passed|failed|pass|fail)\b/gi)];
  const failed = counts.some(m => /^fail/i.test(m[2]) && Number(m[1]) > 0);
  const passed = counts.some(m => /^pass/i.test(m[2]) && Number(m[1]) > 0);
  const testCode = failed ? 1 : passed ? 0 : exitCode;
  for (const m of observed.matchAll(/(?:\d+\s+(?:passed|failed|pass|fail)(?:\b)|Tests?:[^\n]*|test result:[^\n]*)/gi)) facts.push({ kind: 'test', value: detached(m[0]), exitCode: exitCode && exitCode !== 0 ? exitCode : testCode });
  for (const m of out.matchAll(/\[[^\]\n]+\s+([a-f0-9]{7,40})\][^\n]*/g)) facts.push({ kind: 'commit', value: detached(m[0]), exitCode });
  for (const m of out.matchAll(/https:\/\/github\.com\/[^\s"\\]+\/pull\/\d+/g)) facts.push({ kind: 'pr', value: detached(m[0]), exitCode });
  if (exitCode === 0 || exitCode === undefined && !obj.is_error && (/(?:Success|successfully|updated|created)/i.test(out) || /^(?:Write|Edit)$/.test(name))) {
    if (/apply_patch|^(?:Write|Edit)$/.test(name) || /apply_patch/.test(String(data.input ?? ''))) {
      const patch = String(data.input ?? data.patch ?? '').replace(/\\n/g,'\n');
      const paths = [data.file_path, data.path, ...[...String(patch).matchAll(/\*\*\* (?:Update|Add|Delete) File: (.+)/g)].map(m => m[1])].filter(Boolean);
      for (const path of paths) facts.push({ kind: 'file', value: String(path), exitCode });
    }
  }
  // In-place shell edits carry file evidence just like Edit/apply_patch.
  if (exitCode === 0) {
    for (const path of sedInPlaceFiles(command) ?? []) facts.push({ kind: 'file', value: path, exitCode });
  }
  if (/agent|collaboration/.test(name) && /(?:FINAL_ANSWER|verdict|findings|approved)/i.test(out)) facts.push({ kind: 'verdict', value: detached(out.slice(0, 400)) });
  return { exitCode, outputHead: detached(out.slice(0, 400)), facts };
}

/** Record-local normalization; tool results update the original call, never become agent claims. */
export class TranscriptFacts {
  readonly facts: TranscriptFact[] = [];
  unknownRecords = 0;
  private calls = new Map<string, Extract<TranscriptFact, { kind: 'tool' }>>();
  private sessions = new Map<string, string>();
  private turnId = '';
  private messages = new Set<string>();
  private order = 0;
  constructor(private runtime: 'codex' | 'claude', private since = 0) {}
  add(value: unknown): void {
    const entry = record(value);
    const at = typeof entry.timestamp === 'string' ? entry.timestamp : undefined;
    if (!at || !Number.isFinite(Date.parse(at)) || Date.parse(at) < this.since) return;
    this.order++;
    const payload = record(entry.payload);
    if (this.runtime === 'codex') {
      if (entry.type === 'event_msg' && payload.type === 'item_completed') {
        this.add({ type: 'item_completed',timestamp: at,payload }); return;
      }
      if (entry.type === 'event_msg') {
        const phases = { task_started: 'started', task_complete: 'completed', turn_aborted: 'aborted' } as const;
        const phase = phases[payload.type as keyof typeof phases];
        if (phase) {
          this.turnId = payload.turn_id ?? (this.turnId || at);
          this.facts.push({ kind: 'turn', phase, at, turnId: this.turnId, reason: payload.reason });
          if (payload.last_agent_message) this.message('agent_message', payload.last_agent_message, at, true);
        } else if (payload.type === 'user_message') this.message('user_message', payload.message, at);
        else if (/usage_limited|budget_limited/.test(payload.type ?? '')) this.facts.push({ kind: 'limit', scope: payload.type.startsWith('budget') ? 'budget' : 'usage', at });
        return;
      }
      if (entry.type === 'response_item' || entry.type === 'item_completed' || entry.type === 'item.completed') {
        const p = entry.type !== 'response_item' ? record(payload.item ?? entry.item ?? payload) : payload;
        if (p.type === 'message') {
          if (p.role === 'assistant') this.message('agent_message', textContent(p.content), at, p.phase === 'final_answer' || p.channel === 'final');
          if (p.role === 'user') this.message('user_message', textContent(p.content), at);
        } else if (/^(?:function_call|custom_tool_call)$/.test(p.type)) this.tool(p.call_id ?? p.id, p.name, parseInput(p.arguments ?? p.input), at);
        else if (/^(?:command_execution|CommandExecution)$/.test(p.type)) {
          const id = p.call_id ?? p.id;
          if (!this.calls.has(id) && typeof p.command === 'string') this.tool(id,'exec_command',{ command: p.command },at);
          this.result(id,p.output ?? p.aggregated_output ?? p,p.exit_code ?? p.exitCode,false,at);
        } else if (/^(?:function_call_output|custom_tool_call_output)$/.test(p.type)) this.result(p.call_id ?? p.id, p.output ?? p, p.exit_code,false,at);
        return;
      }
      if (!['session_meta', 'turn_context', 'token_usage_record', 'world_state', 'inter_agent_communication_metadata', 'compacted'].includes(entry.type)) this.unknownRecords++;
      return;
    }
    const message = record(entry.message);
    const content = message.content;
    if (entry.type === 'user' && !entry.isMeta) {
      const authored = Array.isArray(content) ? content.filter(b => record(b).type === 'text') : content;
      const text = extractTaskPrompt(textContent(authored));
      if (text) {
        this.turnId = entry.uuid ?? at;
        this.facts.push({ kind: 'turn', phase: 'started', at, turnId: this.turnId });
        this.message('user_message', text, at);
      }
    }
    if (Array.isArray(content)) for (const b of content) {
      const block = record(b);
      if (block.type === 'tool_use') this.tool(block.id, block.name, block.input, at);
      else if (block.type === 'tool_result') {
        // Claude's completed tool_result uses is_error instead of a successful exit code.
        // Background Bash results only announce a task ID; they are not completed commands.
        const call = this.calls.get(block.tool_use_id);
        const exec = /(?:^|[_.])exec_command$/.test(call?.name ?? '');
        const output = textContent(block.content);
        const pendingExecution = exec && (record(parseInput(output)).session_id !== undefined || /Process running with session ID\s+\d+/i.test(output));
        const background = record(entry.toolUseResult).backgroundTaskId || /running in (?:the )?background|background task/i.test(output) || pendingExecution;
        const succeeded = /^(?:Bash|Write|Edit)$/.test(call?.name ?? '') || exec ? 0 : undefined;
        this.result(block.tool_use_id, block.content, block.is_error ? 1 : background ? undefined : succeeded,true,at);
      }
    }
    if (entry.type === 'assistant') {
      const text = Array.isArray(content) ? textContent(content.filter(b => record(b).type === 'text')) : textContent(content);
      const final = message.stop_reason === 'end_turn' || message.stop_reason === 'stop_sequence';
      if (text) this.message('agent_message', text, at, final);
      if (final) this.facts.push({ kind: 'turn', phase: 'completed', at, turnId: this.turnId || at });
    }
    if (entry.type === 'system' && /turn_duration|stop/.test(entry.subtype ?? '')) {
      const last = [...this.facts].reverse().find(f => f.kind === 'agent_message');
      if (last?.kind === 'agent_message') last.final = true;
      if (!this.facts.some(f => f.kind === 'turn' && f.phase === 'completed' && f.turnId === this.turnId)) this.facts.push({ kind: 'turn', phase: 'completed', at, turnId: this.turnId || at });
    }
  }
  private message(kind: 'user_message' | 'agent_message', text: string, at: string, final = false): void {
    if (typeof text !== 'string' || !text) return;
    if (kind === 'user_message') { const authored = extractTaskPrompt(text); if (!authored) return; text = authored; }
    // Codex records the same user message as both event_msg and response_item.
    const key = JSON.stringify([kind,this.turnId,text,kind === 'agent_message' ? final : false]);
    if (this.messages.has(key)) return;
    this.messages.add(key);
    if (kind === 'user_message') this.facts.push({ kind, text, at, turnId: this.turnId });
    else this.facts.push({ kind, text, at, turnId: this.turnId, final });
  }
  private tool(callId: string, name: string, input: unknown, at: string): void {
    if (!callId || !name) return;
    const fact: Extract<TranscriptFact, { kind: 'tool' }> = { kind: 'tool', callId, name, input, at, turnId: this.turnId, startedOrder: this.order, mutating: isMutatingTool(name, input) };
    this.calls.set(callId, fact); this.facts.push(fact);
  }
  private result(callId: string, output: unknown, code?: number, implicit = false, at?: string): void {
    const call = this.calls.get(callId);
    if (!call) return;
    const previousExitCode = call.exitCode;
    const data = record(call.input);
    let result = record(output);
    if (typeof output === 'string') { try { result = record(JSON.parse(output)); } catch { /* plain terminal result */ } }
    if (/(?:^|[_.])exec_command$/.test(call.name)) {
      const sessionId = result.session_id ?? textContent(output).match(/session ID\s+(\d+)/i)?.[1];
      if (sessionId !== undefined) this.sessions.set(String(sessionId), callId);
    }
    if (/(?:^|[_.])write_stdin$/.test(call.name)) {
      const original = this.sessions.get(String(data.session_id));
      if (original) {
        this.result(original, output, code, implicit,at);
      }
    }
    const execution = call.name === 'Bash' || /(?:^|[_.])exec_command$/.test(call.name);
    if (!call.mutating && !execution) {
      call.outputHead = detached(textContent(output).slice(0,400));
      call.exitCode = code; call.facts = []; return;
    }
    const priorFailure = call.facts?.find(fact => fact.kind === 'test' && fact.exitCode !== undefined && fact.exitCode !== 0);
    Object.assign(call, outputEvidence(output, call.input, call.name, code, implicit));
    if (priorFailure) call.facts?.push(priorFailure);
    // Shell reads still prove their own literal command, never tests copied from a log.
    if (!call.mutating) call.facts = call.facts?.filter(f => f.kind === 'command');
    // Keep both boundaries even when timestamps are equal. Repeated terminal
    // polls must not move an old attempt past a newer check.
    const terminal = call.exitCode !== undefined || !execution && call.facts?.some(f => f.kind === 'file');
    if (terminal && (call.completedOrder === undefined || previousExitCode !== call.exitCode)) call.completedOrder = this.order;
    if (execution && previousExitCode !== call.exitCode && call.exitCode !== undefined) {
      // Reuse the execution fact at its terminal receipt boundary. Empty polls
      // remain reads, and no duplicate execution or synthetic stdin write is added.
      const index = this.facts.indexOf(call);
      if (index >= 0) { this.facts.splice(index, 1); this.facts.push(call); }
      if (at) call.at = at;
    }
  }
}
