import { createReadStream } from 'fs';
import { stat } from 'fs/promises';
import { createInterface } from 'readline';
import { TranscriptFacts } from '../../adapters/transcript-facts.js';
import type { TranscriptFact } from '../../domain/ledger/types.js';
import { readCachedLedgerFacts, writeCachedLedgerFacts, ledgerFactFingerprint } from '../../infrastructure/session-summary-cache.js';

interface FactSnapshot { fingerprint: string; facts: TranscriptFact[]; unknownRecords: number }
const cache = new Map<string, { value: FactSnapshot; bytes: number }>();
const MAX_CACHE_BYTES = 16 * 1024 * 1024;
let cacheBytes = 0;
const stats = { reads: 0, diskHits: 0 };
export async function readTranscriptFacts(path: string, runtime: 'codex' | 'claude', now = Date.now()): Promise<FactSnapshot> {
  const info = await stat(path);
  const { since,window,fingerprint } = ledgerFactFingerprint(info,now);
  if (info.mtimeMs < since) return { fingerprint, facts: [], unknownRecords: 0 };
  const key = `${runtime}:${path}`;
  const cached = cache.get(key); if (cached?.value.fingerprint === fingerprint) return cached.value;
  let result = readCachedLedgerFacts<FactSnapshot>(key,fingerprint,window);
  if (result) stats.diskHits++;
  else {
    stats.reads++;
    const parser = new TranscriptFacts(runtime,since);
    const input = createReadStream(path);
    const lines = createInterface({ input, crlfDelay: Infinity });
    for await (const line of lines) {
      try { parser.add(JSON.parse(line)); } catch { parser.unknownRecords++; }
    }
    result = { fingerprint, facts: parser.facts, unknownRecords: parser.unknownRecords };
    const after = await stat(path);
    if (after.size === info.size && after.mtimeMs === info.mtimeMs && after.ctimeMs === info.ctimeMs) writeCachedLedgerFacts(key,fingerprint,result);
    else return result; // A live append must force a fresh read next time.
  }
  const bytes = Buffer.byteLength(JSON.stringify(result));
  if (cached) { cacheBytes -= cached.bytes; cache.delete(key); }
  if (bytes <= MAX_CACHE_BYTES) {
    while (cacheBytes + bytes > MAX_CACHE_BYTES && cache.size) {
      const oldest = cache.keys().next().value!; cacheBytes -= cache.get(oldest)!.bytes; cache.delete(oldest);
    }
    cache.set(key,{ value: result,bytes }); cacheBytes += bytes;
  }
  if (info.size >= 8 * 1024 * 1024) Bun.gc(true);
  return result;
}
export function clearLedgerFactCache() { cache.clear(); cacheBytes = 0; }
export function ledgerFactCacheStats() { return { ...stats,bytes: cacheBytes,entries: cache.size }; }
