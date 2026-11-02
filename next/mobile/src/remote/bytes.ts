// Byte helpers shared by the remote client: no Buffer, so they run in React Native too.
const encoder = new TextEncoder();
const decoder = new TextDecoder();

export const utf8 = (s: string): Uint8Array => encoder.encode(s);
export const text = (b: Uint8Array): string => decoder.decode(b);

export function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

export const hex = (b: Uint8Array): string =>
  Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');

export function unhex(s: string): Uint8Array {
  const out = new Uint8Array(s.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(s.slice(2 * i, 2 * i + 2), 16);
  return out;
}

const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';

/** base64url without padding, as the machine writes codes and tokens. */
export function b64url(b: Uint8Array): string {
  let out = '';
  for (let i = 0; i < b.length; i += 3) {
    const n = (b[i] << 16) | ((b[i + 1] ?? 0) << 8) | (b[i + 2] ?? 0);
    const chars = Math.min(4, Math.ceil(((b.length - i) * 8) / 6));
    for (let j = 0; j < chars; j++) out += B64[(n >> (18 - 6 * j)) & 63];
  }
  return out;
}

/** Reads base64url with or without padding; `null` on any other character. */
export function unb64url(s: string): Uint8Array | null {
  s = s.trim().replace(/=+$/, '');
  const bits: number[] = [];
  for (const c of s) {
    const v = B64.indexOf(c);
    if (v < 0) return null;
    bits.push(v);
  }
  const out = new Uint8Array(Math.floor((bits.length * 6) / 8));
  for (let i = 0, acc = 0, n = 0, o = 0; i < bits.length; i++) {
    acc = (acc << 6) | bits[i];
    n += 6;
    if (n >= 8) {
      n -= 8;
      if (o < out.length) out[o++] = (acc >> n) & 255;
    }
  }
  return out;
}
