// A client of one Keepline machine through a relay: the WebSocket, the Noise handshake,
// JSON lines both ways, ACKs, requests by id and streams. What
// `next/crates/keepline-relay/examples/phone.rs` does, for the app. Protocol shapes:
// `specs/extensions/remote-protocol.md`.
import { concat, text, utf8 } from './bytes';
import { type Link, linkUrl } from './link';
import { CHUNK, type Channel, Handshake } from './noise';

/** Why the connection ended, from the relay's or machine's close code. */
export type Ended = {
  code: number;
  reason: string;
  /** 4401: unknown device, wrong key, revoked or role changed. Do not retry blindly. */
  refused: boolean;
};

export class RemoteError extends Error {}

/** What a close code means to the person holding the phone; the number stays for support. */
export function closeText(code: number): string {
  switch (code) {
    case 4401:
      return '这台手机没有配对，或者已经被取消';
    case 1012:
      return '电脑上的 Keepline 不在线';
    case 1013:
      return '网络太慢，中继断开了连接';
    default:
      return `连不上：电脑或中继不在线，或者网络不通（${code}）`;
  }
}

/** WebSocket.send's types want bytes over a plain ArrayBuffer, which these always are. */
const own = (b: Uint8Array) => b as Uint8Array<ArrayBuffer>;

type Pending = { resolve: (ok: any) => void; reject: (e: Error) => void };
type StreamHandler = (message: any) => void;

export class Remote {
  private nextId = 1;
  private pending = new Map<number, Pending>();
  private streams = new Map<number, StreamHandler>();
  private buffer: Uint8Array = new Uint8Array(0);
  private endedHandlers: ((e: Ended) => void)[] = [];
  ended: Ended | null = null;

  private constructor(
    private ws: WebSocket,
    private channel: Channel,
  ) {
    ws.onmessage = (ev) => this.receive(new Uint8Array(ev.data as ArrayBuffer));
    ws.onclose = (ev) => this.end({ code: ev.code, reason: ev.reason, refused: ev.code === 4401 });
    ws.onerror = () => undefined;
  }

  /** Opens the WebSocket and finishes the handshake; `hello` is the caller's first request. */
  static connect(link: Link, timeoutMs = 10_000): Promise<Remote> {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(linkUrl(link));
      ws.binaryType = 'arraybuffer';
      const hs = new Handshake(link.key);
      const timer = setTimeout(() => {
        ws.close();
        reject(new RemoteError('连接超时'));
      }, timeoutMs);
      ws.onopen = () => ws.send(own(hs.first));
      ws.onmessage = (ev) => {
        clearTimeout(timer);
        try {
          resolve(new Remote(ws, hs.finish(new Uint8Array(ev.data as ArrayBuffer))));
        } catch (e) {
          ws.close();
          reject(new RemoteError(`握手失败：${(e as Error).message}`));
        }
      };
      ws.onclose = (ev) => {
        clearTimeout(timer);
        reject(new RemoteError(closeText(ev.code)));
      };
      ws.onerror = () => undefined;
    });
  }

  /** One request; resolves with `ok`, rejects with the machine's `error`. */
  request<T = any>(op: string, fields: Record<string, unknown> = {}): Promise<T> {
    if (this.ended) return Promise.reject(new RemoteError('连接已断开'));
    const id = this.nextId++;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.send({ ...fields, id, op });
    });
  }

  /** Opens `events` or `watch`; `onMessage` gets each stream message until it ends. */
  async stream(
    op: 'events' | 'watch',
    fields: Record<string, unknown>,
    onMessage: StreamHandler,
  ): Promise<{ stream: number; close: () => Promise<void> }> {
    const id = this.nextId;
    this.streams.set(id, onMessage);
    try {
      const ok = await this.request(op, fields);
      return { stream: ok.stream, close: () => this.request('close', { stream: ok.stream }) };
    } catch (e) {
      this.streams.delete(id);
      throw e;
    }
  }

  onEnded(handler: (e: Ended) => void) {
    if (this.ended) handler(this.ended);
    else this.endedHandlers.push(handler);
  }

  close() {
    this.ws.close(1000);
  }

  private send(value: unknown) {
    const line = utf8(JSON.stringify(value) + '\n');
    for (let at = 0; at < line.length; at += CHUNK) {
      this.ws.send(own(this.channel.data(line.subarray(at, at + CHUNK))));
    }
  }

  private receive(sealed: Uint8Array) {
    let message;
    try {
      message = this.channel.open(sealed);
    } catch {
      // A message this channel cannot open means the stream is broken; start over.
      this.ws.close(4000, 'undecryptable');
      return;
    }
    if (message.kind !== 'data') return;
    this.ws.send(own(this.channel.ack(message.bytes.length)));
    this.buffer = concat(this.buffer, message.bytes);
    let nl;
    while ((nl = this.buffer.indexOf(10)) >= 0) {
      const line = text(this.buffer.subarray(0, nl));
      this.buffer = this.buffer.slice(nl + 1);
      if (line.trim()) this.line(JSON.parse(line));
    }
  }

  private line(m: any) {
    if (typeof m.stream === 'number') {
      const handler = this.streams.get(m.stream);
      if (m.type === 'end') this.streams.delete(m.stream);
      handler?.(m);
      return;
    }
    const p = typeof m.id === 'number' ? this.pending.get(m.id) : undefined;
    if (!p) return;
    this.pending.delete(m.id);
    if ('ok' in m) p.resolve(m.ok);
    else p.reject(new RemoteError(m.error));
  }

  private end(e: Ended) {
    if (this.ended) return;
    this.ended = e;
    for (const p of this.pending.values()) p.reject(new RemoteError(closeText(e.code)));
    this.pending.clear();
    for (const h of this.streams.values()) h({ type: 'end', reason: e.reason || String(e.code) });
    this.streams.clear();
    for (const h of this.endedHandlers) h(e);
  }
}
