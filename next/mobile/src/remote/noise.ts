// The encrypted channel between this phone and a Keepline machine, byte for byte what
// `next/crates/keepline-remote/src/secure.rs` does: Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s
// keyed by this device's key, then DATA / ACK messages. Pure JS, no native module, so
// the same code runs in the app, in Bun tests and in Node. `vectors/v1.json` checks it.
import { chacha20poly1305 } from '@noble/ciphers/chacha.js';
import { x25519 } from '@noble/curves/ed25519.js';
import { blake2s } from '@noble/hashes/blake2.js';
import { hmac } from '@noble/hashes/hmac.js';
import { sha256 } from '@noble/hashes/sha2.js';
import { randomBytes } from '@noble/hashes/utils.js';
import { concat, utf8 } from './bytes';

export const PATTERN = 'Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s';
const PROLOGUE = utf8('keepline remote 1');
export const DATA = 0;
export const ACK = 1;
/** The most bytes one DATA message carries. */
export const CHUNK = 32 * 1024;
const TAG = 16;

/** `sha256(label ‖ 0 ‖ secret)`: the route token's and the PSK's derivation. */
export function derive(label: string, secret: Uint8Array): Uint8Array {
  return sha256(concat(utf8(label), new Uint8Array([0]), secret));
}

/** The pre-shared key a device key gives. */
export function psk(deviceKey: Uint8Array): Uint8Array {
  return derive('keepline psk 1', deviceKey);
}

function hkdf(ck: Uint8Array, ikm: Uint8Array, n: 2 | 3): Uint8Array[] {
  const temp = hmac(blake2s, ck, ikm);
  const out: Uint8Array[] = [];
  let prev = new Uint8Array(0);
  for (let i = 1; i <= n; i++) {
    prev = hmac(blake2s, temp, concat(prev, new Uint8Array([i])));
    out.push(prev);
  }
  return out;
}

function nonce(n: number): Uint8Array {
  const out = new Uint8Array(12);
  new DataView(out.buffer).setBigUint64(4, BigInt(n), true);
  return out;
}

/** One direction of a Noise cipher: a key and a counter that never repeats. */
class Cipher {
  private n = 0;
  constructor(private k: Uint8Array) {}
  seal(ad: Uint8Array, plain: Uint8Array): Uint8Array {
    return chacha20poly1305(this.k, nonce(this.n++), ad).encrypt(plain);
  }
  open(ad: Uint8Array, sealed: Uint8Array): Uint8Array {
    // Throws on a wrong key, tampering, replay or reordering; the counter moves only on success.
    const plain = chacha20poly1305(this.k, nonce(this.n), ad).decrypt(sealed);
    this.n++;
    return plain;
  }
}

/** Noise's SymmetricState for this pattern. */
class Symmetric {
  h: Uint8Array;
  ck: Uint8Array;
  k: Cipher | null = null;
  constructor() {
    const name = utf8(PATTERN);
    this.h = name.length <= 32 ? concat(name, new Uint8Array(32 - name.length)) : blake2s(name);
    this.ck = this.h;
    this.mixHash(PROLOGUE);
  }
  mixHash(data: Uint8Array) {
    this.h = blake2s(concat(this.h, data));
  }
  mixKey(ikm: Uint8Array) {
    const [ck, k] = hkdf(this.ck, ikm, 2);
    this.ck = ck;
    this.k = new Cipher(k);
  }
  mixKeyAndHash(ikm: Uint8Array) {
    const [ck, h, k] = hkdf(this.ck, ikm, 3);
    this.ck = ck;
    this.mixHash(h);
    this.k = new Cipher(k);
  }
  encryptAndHash(plain: Uint8Array): Uint8Array {
    const out = this.k!.seal(this.h, plain);
    this.mixHash(out);
    return out;
  }
  decryptAndHash(sealed: Uint8Array): Uint8Array {
    const plain = this.k!.open(this.h, sealed);
    this.mixHash(sealed);
    return plain;
  }
  split(): [Cipher, Cipher] {
    const [a, b] = hkdf(this.ck, new Uint8Array(0), 2);
    return [new Cipher(a), new Cipher(b)];
  }
}

export type Message = { kind: 'data'; bytes: Uint8Array } | { kind: 'ack'; n: number };

/** The channel after the handshake. */
export class Channel {
  constructor(
    private send: Cipher,
    private recv: Cipher,
  ) {}
  /** Seals at most CHUNK bytes of the protocol's JSON lines. */
  data(bytes: Uint8Array): Uint8Array {
    if (bytes.length > CHUNK) throw new Error('DATA payload over CHUNK');
    return this.seal(DATA, bytes);
  }
  /** Tells the machine this many more DATA bytes were taken in. */
  ack(n: number): Uint8Array {
    const b = new Uint8Array(4);
    new DataView(b.buffer).setUint32(0, n, false);
    return this.seal(ACK, b);
  }
  private seal(kind: number, payload: Uint8Array): Uint8Array {
    return this.send.seal(new Uint8Array(0), concat(new Uint8Array([kind]), payload));
  }
  open(sealed: Uint8Array): Message {
    const plain = this.recv.open(new Uint8Array(0), sealed);
    const rest = plain.subarray(1);
    if (plain[0] === DATA) return { kind: 'data', bytes: rest };
    if (plain[0] === ACK && rest.length === 4)
      return { kind: 'ack', n: new DataView(rest.buffer, rest.byteOffset).getUint32(0, false) };
    throw new Error(plain[0] === ACK ? 'bad ack' : 'unknown message kind');
  }
}

/** The phone's side, which speaks first. */
export class Handshake {
  private s = new Symmetric();
  private e: Uint8Array;
  /** The first message to send. */
  readonly first: Uint8Array;
  /** `ephemeral` is for the test vectors only; leave it out. */
  constructor(deviceKey: Uint8Array, ephemeral?: Uint8Array) {
    this.e = ephemeral ?? randomBytes(32);
    // -> psk, e
    this.s.mixKeyAndHash(psk(deviceKey));
    const pub = x25519.getPublicKey(this.e);
    this.s.mixHash(pub);
    this.s.mixKey(pub);
    this.first = concat(pub, this.s.encryptAndHash(new Uint8Array(0)));
  }
  /** Reads the machine's answer: `<- e, ee`. Throws if the machine has another key. */
  finish(answer: Uint8Array): Channel {
    if (answer.length !== 32 + TAG) throw new Error('noise: bad answer length');
    const re = answer.subarray(0, 32);
    this.s.mixHash(re);
    this.s.mixKey(re);
    this.s.mixKey(x25519.getSharedSecret(this.e, re));
    this.s.decryptAndHash(answer.subarray(32));
    const [toMachine, toPhone] = this.s.split();
    return new Channel(toMachine, toPhone);
  }
}

/** The machine's side, for tests: reads the phone's first message and answers it. */
export function respond(
  deviceKey: Uint8Array,
  first: Uint8Array,
  ephemeral?: Uint8Array,
): { channel: Channel; answer: Uint8Array } {
  const s = new Symmetric();
  s.mixKeyAndHash(psk(deviceKey));
  const re = first.subarray(0, 32);
  s.mixHash(re);
  s.mixKey(re);
  s.decryptAndHash(first.subarray(32));
  const e = ephemeral ?? randomBytes(32);
  const pub = x25519.getPublicKey(e);
  s.mixHash(pub);
  s.mixKey(pub);
  s.mixKey(x25519.getSharedSecret(e, re));
  const answer = concat(pub, s.encryptAndHash(new Uint8Array(0)));
  const [toMachine, toPhone] = s.split();
  return { channel: new Channel(toPhone, toMachine), answer };
}
