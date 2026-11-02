// The phone client against a real machine, run by the Rust test
// `the_js_phone_client_chats_through_a_real_relay` (keeplined/tests/managed_session.rs):
//   bun test/phone-e2e.ts '<pairing link>' <terminal>
// Prints one JSON line of what it saw; the Rust side asserts on it and on the terminal.
import { Remote } from '../src/remote/client';
import { parseLink } from '../src/remote/link';

const [linkText, terminal] = process.argv.slice(2);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const out: Record<string, unknown> = {};
const t0 = performance.now();
const remote = await Remote.connect(parseLink(linkText));
out.connect_ms = Math.round(performance.now() - t0);
const hello = await remote.request('hello', { version: 1 });
out.role = hello.role;
out.ops = hello.ops;
const events: string[] = [];
await remote.stream('events', {}, (m) => m.type === 'event' && events.push(m.event?.event));
const list = await remote.request('list');
out.card = list.terminals.find((t: any) => t.id === terminal);
let page: any;
for (let i = 0; i < 100; i++) {
  page = await remote.request('chat', { terminal, limit: 50 });
  if (page.messages.length >= 2) break;
  await sleep(100);
}
out.before = page.messages.map((m: any) => `${m.role}:${m.text}`);
const last = page.messages.at(-1).seq;
const sent = performance.now();
await remote.request('send', { terminal, text: '手机发来的话', enter: true });
out.send_ms = Math.round(performance.now() - sent);
let newer: any;
for (let i = 0; i < 100; i++) {
  newer = await remote.request('chat', { session: page.session, after: last });
  if (newer.messages.some((m: any) => m.role === 'agent')) break;
  await sleep(100);
}
out.reply_ms = Math.round(performance.now() - sent);
out.after = newer.messages.map((m: any) => `${m.role}:${m.text}`);
try {
  await remote.request('chat', {});
} catch (e) {
  out.bad = (e as Error).message;
}
out.events = events;
remote.close();
console.log(JSON.stringify(out));
process.exit(0);
