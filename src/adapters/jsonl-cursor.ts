import { createReadStream } from 'fs';

/** Derived parser state at the last complete JSONL record, shared across scan processes. */
export interface JsonlCheckpoint {
  offset: number;
  lineNumber: number;
  state: unknown;
}
export interface JsonlCursorOptions {
  resume?: JsonlCheckpoint;
  end?: number;
  onCheckpoint?: (checkpoint: JsonlCheckpoint) => void;
}

/** Byte offsets remain correct for CRLF, UTF-8 and partially written final records. */
export async function* jsonlLines(path: string, options: JsonlCursorOptions = {}) {
  let offset = options.resume?.offset ?? 0;
  let lineNumber = options.resume?.lineNumber ?? 0;
  if (options.end !== undefined && offset >= options.end) return;
  const stream = createReadStream(path, { start: offset, ...(options.end === undefined ? {} : { end: options.end - 1 }) });
  let parts: Buffer[] = [];
  let pending: { text: string; offset: number; lineNumber: number; terminated: boolean } | undefined;
  for await (const chunk of stream) {
    const buffer = chunk as Buffer;
    let start = 0, newline: number;
    while ((newline = buffer.indexOf(10, start)) !== -1) {
      const piece = buffer.subarray(start,newline);
      const line = parts.length ? Buffer.concat([...parts,piece]) : piece;
      offset += line.length + 1;
      if (pending) yield { ...pending, last: false };
      pending = { text: line.toString('utf8').replace(/\r$/, ''), offset, lineNumber: ++lineNumber, terminated: true };
      parts = []; start = newline + 1;
    }
    if (start < buffer.length) parts.push(buffer.subarray(start));
  }
  if (parts.length) {
    if (pending) yield { ...pending, last: false };
    yield { text: Buffer.concat(parts).toString('utf8'), offset, lineNumber: lineNumber + 1, terminated: false, last: true };
  } else if (pending) yield { ...pending, last: true };
}
