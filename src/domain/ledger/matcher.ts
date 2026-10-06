import { createHash } from 'crypto';
import { extractTaskPrompt } from '../session/index.js';
import type { Anchors, Ask, Constraint, Correction, LedgerConfig, LedgerEvidence, LedgerRule, LedgerStep, OffPlanRun, RequirementItem, TranscriptFact } from './types.js';

export function ledgerId(...parts: string[]): string { return createHash('sha256').update(parts.join('\0')).digest('hex').slice(0, 32); }
export function anchorsFromText(text: string): Anchors {
  return {
    paths: [...new Set([...text.matchAll(/(?:`|\s|^)((?:[\w.-]+\/)+[\w.*?/-]+|[\w.-]+\.(?:ts|tsx|js|json|rs|py|md|toml))/g)].map(m => m[1]))],
    commands: [...text.matchAll(/(?:`|\b)((?:bun|npm|pnpm|cargo|pytest|git)\s+[^`\n。;]+)(?:`|$)/g)].map(m => m[1].trim()),
    keywords: [...new Set(text.toLowerCase().match(/[\p{L}\p{N}_-]{3,}/gu) ?? [])].slice(0, 30),
  };
}
export function extractAsks(facts: TranscriptFact[], sessionId: string): Ask[] {
  const asks: Ask[] = [];
  let hasRequirement = false;
  for (const f of facts) if (f.kind === 'user_message') {
    const text = extractTaskPrompt(f.text);
    if (!text) continue;
    // Keep uncertain intent by default. Remove only explicit quoted/program content.
    let authoredText = text.replace(/<pasted_content\b[^>]*>[\s\S]*?(?:<\/pasted_content>|$)/gi,'')
      .replace(/(^|\n)\s*(```|~~~)[^\n]*\n[\s\S]*?(?:\n\s*\2[^\n]*(?=\n|$)|$)/g,'$1')
      .replace(/^\s*>[^\n]*(?:\n|$)/gm,'').trim();
    const report = authoredText.match(/(?:^|\n)\s*#{1,6}\s+(?:结论|修复说明|验收记录|实现汇报|完成的检查|Summary|Verification|Implementation report)\s*[:：]?\s*(?:\n|$)/i);
    if (report && authoredText.slice(report.index).length > 200 && /(?:\|[^\n]+\||\b\d+\s+(?:pass|fail)|测试[^\n]*(?:通过|失败))/i.test(authoredText.slice(report.index))) authoredText = authoredText.slice(0,report.index).trim();
    if (!authoredText || /^(?:现在呢|然后呢|怎么样了|进度呢|还有吗|what now|how is it going|status)[?？.!。！\s]*$/i.test(authoredText)) continue;
    if (/^(?:ok|okay|yes|continue|go ahead|approved|好|好的|继续|可以|同意|是的|谢谢|thank you)[.!。！\s]*$/i.test(authoredText)) continue;
    const lines = authoredText.split('\n').map(line => line.trim()).filter(line => line && !line.startsWith('#'));
    if (lines.length && lines.every(line => /^(?:[$>]\s*|(?:bun|npm|pnpm|yarn|git|cargo|go|uv|python3?|node|bash|zsh|sh|ls|pwd|cd|cat|rg|grep|find|curl|echo|sleep|mkdir|touch|rm|make|keepline)(?:\s|$)|\.{0,2}\/\S+|\/\w+(?:\s|$))/.test(line.replace(/^`([^`]+)`$/,'$1')))) continue;
    if (/^(?:import\s+(?:[^\n]+\bfrom\s+['"]|['"])|export\s+(?:default|const|function|class|interface|type)\b|(?:const|let|var)\s+\w+\s*[=:]|function\s+\w+\s*\(|def\s+\w+\s*\(|class\s+\w+\s*[:{])/.test(authoredText)) continue;
    if (/^[{[]/.test(authoredText)) { try { JSON.parse(authoredText); continue; } catch {} }
    const sentences = authoredText.split(/\n+|(?<=[。.!！])\s+/).filter(s => s.trim());
    const question = sentences.every(sentence => /[?？]\s*$|(?:吗|么|呢|有没有|什么|如何|怎么)\s*[.!。！]*$/i.test(sentence) || /^(?:什么|怎么|为何|为什么|如何|有没有|是否|what\b|how\b|why\b|where\b|when\b|can\s+you\b|could\s+you\b|would\s+you\b)/i.test(sentence));
    const kind = question ? 'question' : hasRequirement ? 'addition' : 'initial';
    if (!question) hasRequirement = true;
    asks.push({ id: ledgerId(sessionId, 'ask', f.at, f.text), text: text === f.text.trim() ? f.text : text, authoredText, at: f.at, turnId: f.turnId, kind });
  }
  return asks;
}
export function constraintsFromText(text: string): Constraint[] {
  const constraints: Constraint[] = [];
  for (const sentence of text.split(/[\n。;；]+/)) {
    if (/(?:do not|don't|never|keep|preserve|no\b).*(?:public\s+api)|(?:不要|不得|禁止|不能|不改|保持|保留).*(?:公共|公开|public)\s*(?:接口|api)/i.test(sentence)) constraints.push({ kind: 'no_public_api_change' });
    if (/(?:do not|don't|never|不要|不得|禁止|不能|勿).*(?:change|edit|touch|write|修改|改动|编辑|写入)/i.test(sentence)) for (const value of anchorsFromText(sentence).paths) constraints.push({ kind: 'path_forbidden',value });
    for (const match of sentence.matchAll(/(?:keep|preserve|保留|保持)\s*(?:the\s+)?(?:old\s+|旧的?\s*)?[`'"]?([\w-]+)[`'"]?\s*(?:tags?|标签)/gi)) constraints.push({ kind: 'preserve_text',value: match[1] });
  }
  return [...new Map(constraints.map(c => [JSON.stringify(c),c])).values()];
}
export function followUpSuggestions(facts: TranscriptFact[]): string[] {
  const suggestions: string[] = [];
  for (const fact of facts) if (fact.kind === 'agent_message' && fact.final) {
    let proposing = false;
    for (const line of fact.text.split('\n')) {
      const heading = line.replace(/\*\*/g,'').match(/^\s*(?:#{1,6}\s*)?(?:next(?: steps?)?|follow[- ]?up(?: work)?|后续(?:工作|建议)?|下一步(?:建议)?)(?:\s*[:：]\s*(.*)|\s*)$/i);
      if (heading) { proposing = true; if (heading[1]?.trim()) suggestions.push(heading[1].trim()); continue; }
      if (proposing && /^\s*(?:[-*]|\d+[.)、])\s+/.test(line)) suggestions.push(line.replace(/^\s*(?:[-*]|\d+[.)、])\s+/, '').trim());
      else if (line.trim()) proposing = false;
    }
  }
  return [...new Set(suggestions)].filter(Boolean);
}
export function decompose(asks: Ask[], sessionId: string, checklist: Array<{ id: string; text: string }> = []): RequirementItem[] {
  const candidates = asks.filter(a => a.kind !== 'question');
  const constraints = constraintsFromText(candidates.map(a => a.authoredText).join('\n'));
  const base = checklist.length ? checklist.map(c => ({ text: c.text, key: c.id, checklistId: c.id, source: 'work_item' as const }))
    : candidates.map(a => ({ text: a.authoredText, key: a.id, checklistId: undefined, source: 'fallback' as const }));
  return base.map((a, ordinal) => ({ id: ledgerId(sessionId, 'item', a.key), ordinal, title: a.text.slice(0, 120),
    anchors: anchorsFromText(a.text), constraints: checklist.length ? [...new Map([...constraints,...constraintsFromText(a.text)].map(c => [JSON.stringify(c),c])).values()] : [...constraints], source: a.source, status: 'todo', statusSource: 'rule', evidenceIds: [], checklistId: a.checklistId }));
}
export function pathMatches(pattern: string, path: string): boolean {
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*\*/g, '\u0000').replace(/\*/g, '[^/]*').replace(/\?/g, '[^/]').replace(/\u0000/g, '.*');
  return new RegExp(`(?:^|/)${escaped}(?:$|/)`).test(path);
}
function commandMatches(pattern: string,summary: string): boolean {
  try { return new RegExp(pattern).test(summary); } catch { return summary.includes(pattern); }
}
function score(anchors: Anchors, summary: string, paths: string[], lowerSummary: string): number {
  return anchors.paths.reduce((n, path) => n + (paths.some(p => pathMatches(path,p)) || summary.includes(path) ? 5 : 0), 0)
    + anchors.commands.reduce((n, cmd) => n + (commandMatches(cmd,summary) ? 5 : 0), 0)
    + anchors.keywords.reduce((n, word) => n + (lowerSummary.includes(word.toLowerCase()) ? 1 : 0), 0);
}
export function matchLedger(facts: TranscriptFact[], inputItems: RequirementItem[], corrections: Correction[], rules: LedgerRule[], cfg: LedgerConfig, sessionId = '') {
  const items = structuredClone(inputItems);
  for (const item of items) if (item.statusSource !== 'user') { item.evidenceIds = []; item.status = 'todo'; }
  const evidence: LedgerEvidence[] = [];
  const evidenceIds = new Set<string>();
  const trail: LedgerStep[] = [];
  let readOnlyCount = 0;
  for (const fact of facts) {
    if (fact.kind !== 'tool') continue;
    const summary = `${fact.name} ${JSON.stringify(fact.input)}`;
    const lowerSummary = summary.toLowerCase();
    const paths: string[] = [];
    const scanPaths = (input: unknown) => {
      if (!input || typeof input !== 'object') return;
      for (const [key,value] of Object.entries(input)) {
        if (['path','file_path','filePath','notebook_path'].includes(key) && typeof value === 'string') paths.push(value);
        else if (value && typeof value === 'object') scanPaths(value);
      }
    };
    scanPaths(fact.input);
    const ownEvidence = (fact.facts ?? []).map((e, i) => ({ ...e, id: ledgerId(sessionId, fact.callId, String(i), e.kind, e.value), callId: fact.callId, at: fact.at }));
    evidence.push(...ownEvidence); ownEvidence.forEach(e => evidenceIds.add(e.id));
    paths.push(...ownEvidence.filter(e => e.kind === 'file').map(e => e.value));
    // Successful tests are evidence even though the test invocation is not a file edit.
    if (!fact.mutating) { readOnlyCount++; continue; }
    const correction = corrections.find(c => c.callId === fact.callId);
    const rule = rules.find(r => r.matcher.paths.some(p => paths.some(path => pathMatches(p,path)) || summary.includes(p)) || r.matcher.commands.some(c => commandMatches(c,summary)));
    const candidates = items.filter(i => !i.dropped).map(item => ({ item, score: score(item.anchors, summary, paths,lowerSummary) })).sort((a,b) => b.score - a.score || b.item.ordinal - a.item.ordinal);
    const itemId = correction ? correction.itemId : rule?.itemId ?? (candidates[0]?.score >= 2 ? candidates[0].item.id : undefined);
    const item = items.find(i => i.id === itemId);
    const violations: string[] = [];
    if (cfg.constraints) for (const i of items) for (const c of i.constraints) {
      const files = ownEvidence.filter(e => e.kind === 'file');
      if (c.kind === 'path_forbidden' && c.value && files.some(f => pathMatches(c.value!, f.value))) violations.push(`Forbidden path: ${c.value}`);
      if (c.kind === 'no_public_api_change' && /apply_patch|Edit|Write/.test(fact.name) && /(?:\\n|^|\n)[+-](?![+-])[^\n]*\b(?:export|pub)\b/.test(JSON.stringify(fact.input))) violations.push('Public API declaration changed');
      if (c.kind === 'preserve_text' && c.value) {
        const data = fact.input && typeof fact.input === 'object' ? fact.input as Record<string,unknown> : {};
        const patch = String(data.input ?? data.patch ?? '').replace(/\\n/g,'\n');
        const removed = patch.split('\n').filter(l => /^-(?!-)/.test(l) && l.includes(c.value!)).length;
        const added = patch.split('\n').filter(l => /^\+(?!\+)/.test(l) && l.includes(c.value!)).length;
        if (removed > added || typeof data.old_string === 'string' && data.old_string.includes(c.value) && typeof data.new_string === 'string' && !data.new_string.includes(c.value)) violations.push(`Preserved text removed: ${c.value}`);
      }
    }
    const successful = ownEvidence.some(e => e.kind === 'test' && e.exitCode !== 0) ? [] : ownEvidence.filter(e => (e.kind === 'command' || e.kind === 'test') && e.exitCode === 0);
    const anchoredCheck = item && item.source !== 'fallback' && fact.exitCode === 0 && item.anchors.commands.some(cmd => commandMatches(cmd,summary));
    if (item && item.statusSource !== 'user') {
      // Raw, potentially multi-part asks never become done through keyword overlap.
      const ids = anchoredCheck ? successful.map(e => e.id) : [];
      item.evidenceIds = [...new Set([...item.evidenceIds, ...ids])].filter(id => evidenceIds.has(id));
      item.status = item.source !== 'fallback' && item.evidenceIds.length ? 'done' : 'doing'; item.statusSource = 'rule';
    }
    trail.push({ callId: fact.callId, name: fact.name, summary: summary.slice(0, 600), at: fact.at, turnId: fact.turnId || 'unknown', itemId: item?.id,
      evidenceIds: ownEvidence.map(e => e.id), violations, acceptedOffPlan: correction?.acceptOffPlan ?? false });
  }
  // Revalidate computed done rows against current evidence; never invent completion from prose.
  for (const item of items) if (item.statusSource !== 'user') {
    item.evidenceIds = item.evidenceIds.filter(id => evidenceIds.has(id));
    if (item.status === 'done' && !item.evidenceIds.length) item.status = 'unverified';
  }
  const offPlan: OffPlanRun[] = [];
  let unmatched: LedgerStep[] = [];
  const flush = () => {
    if (cfg.deviation !== 'off' && unmatched.length >= (cfg.deviation === 'sensitive' ? 3 : 8) &&
      (cfg.deviation === 'sensitive' || Date.parse(unmatched.at(-1)!.at) - Date.parse(unmatched[0].at) >= 600_000)) {
      offPlan.push({ id: unmatched[0].callId, callIds: unmatched.map(s => s.callId), at: unmatched[0].at });
    }
    unmatched = [];
  };
  for (const step of trail) {
    if (!step.acceptedOffPlan && step.violations.length) offPlan.push({ id: step.callId, callIds: [step.callId], at: step.at });
    if (!step.itemId && !step.acceptedOffPlan) unmatched.push(step); else flush();
  }
  flush();
  return { items, evidence, trail, readOnlyCount, offPlan, progress: { done: items.filter(i => i.status === 'done' && i.evidenceIds.some(id => evidenceIds.has(id))).length, total: items.length } };
}
