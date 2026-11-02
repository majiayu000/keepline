// The bytes `next/crates/keepline-remote/vectors/v1.json` fixes, which the machine's Rust
// side also checks itself against: if these pass, this client and the machine agree.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { b64url, hex, text, unb64url, unhex, utf8 } from '../src/remote/bytes';
import { frame, linkText, linkUrl, parseLink, token, tokenHash } from '../src/remote/link';
import { Handshake, PATTERN, psk, respond } from '../src/remote/noise';

const v = JSON.parse(
  readFileSync(join(import.meta.dir, '../../crates/keepline-remote/vectors/v1.json'), 'utf8'),
);
const code = unhex(v.machine_code.bytes);
const key = unhex(v.device_key.bytes);

describe('vectors/v1.json', () => {
  test('codes, token, token hash and PSK', () => {
    expect(b64url(code)).toBe(v.machine_code.text);
    expect(hex(unb64url(v.machine_code.text)!)).toBe(v.machine_code.bytes);
    expect(b64url(key)).toBe(v.device_key.text);
    expect(token(code)).toBe(v.token);
    expect(tokenHash(v.token)).toBe(v.token_hash);
    expect(hex(psk(key))).toBe(v.device_key.psk);
  });

  test('handshake and messages, both ways', () => {
    expect(PATTERN).toBe(v.noise.pattern);
    const hs = new Handshake(key, unhex(v.noise.phone_ephemeral_private));
    expect(hex(hs.first)).toBe(v.noise.handshake_1_phone);
    const machine = respond(key, hs.first, unhex(v.noise.machine_ephemeral_private));
    expect(hex(machine.answer)).toBe(v.noise.handshake_2_machine);
    const phone = hs.finish(unhex(v.noise.handshake_2_machine));

    const [m1, m2, m3] = v.messages;
    expect(hex(phone.data(utf8(m1.plain)))).toBe(m1.sealed);
    expect(hex(machine.channel.data(utf8(m2.plain)))).toBe(m2.sealed);
    expect(hex(phone.ack(m3.n))).toBe(m3.sealed);

    // And each side opens what the other sealed.
    const got = machine.channel.open(unhex(m1.sealed));
    expect(got.kind === 'data' && text(got.bytes)).toBe(m1.plain);
    const reply = phone.open(unhex(m2.sealed));
    expect(reply.kind === 'data' && text(reply.bytes)).toBe(m2.plain);
    expect(machine.channel.open(unhex(m3.sealed))).toEqual({ kind: 'ack', n: m3.n });
  });

  test('relay frame', () => {
    const f = v.relay_frames[0];
    expect(hex(frame(f.kind, f.id, unhex(f.payload)))).toBe(f.frame);
  });

  test('pairing link and URL', () => {
    const link = parseLink(v.link.text);
    expect(link.name).toBe('Lin 的 Mac');
    expect(hex(link.code)).toBe(v.machine_code.bytes);
    expect(hex(link.key)).toBe(v.device_key.bytes);
    expect(linkText(link)).toBe(v.link.text);
    expect(linkUrl(link)).toBe(v.link.url);
  });
});

describe('the channel refuses', () => {
  test('a wrong key, tampering and replay', () => {
    const hs = new Handshake(key);
    expect(() => respond(code, hs.first)).toThrow();

    const a = new Handshake(key);
    const m = respond(key, a.first);
    const phone = a.finish(m.answer);
    const sealed = phone.data(utf8('hi'));
    const bad = sealed.slice();
    bad[3] ^= 1;
    expect(() => m.channel.open(bad)).toThrow();
    expect(m.channel.open(sealed).kind).toBe('data');
    expect(() => m.channel.open(sealed)).toThrow();
  });

  test('a bad link', () => {
    expect(() => parseLink('https://example.com')).toThrow();
    expect(() => parseLink(v.link.text.replace('v=2', 'v=1'))).toThrow();
    expect(() => parseLink(v.link.text.replace(/key=[^&]+/, 'key=short'))).toThrow();
  });
});
