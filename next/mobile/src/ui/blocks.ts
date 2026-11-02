// Markdown blocks as Agents write them: headings, list items, quotes, rules, fenced code
// and paragraphs. Pure, so tests run it without React Native.
export type Block =
  | { kind: 'code'; text: string }
  | { kind: 'heading'; level: number; text: string }
  | { kind: 'item'; mark: string; depth: number; text: string }
  | { kind: 'quote'; text: string }
  | { kind: 'rule' }
  | { kind: 'para'; text: string };

export function blocks(src: string): Block[] {
  const out: Block[] = [];
  const lines = src.replace(/\r\n/g, '\n').split('\n');
  let para: string[] = [];
  const flush = () => {
    if (para.length) out.push({ kind: 'para', text: para.join('\n') });
    para = [];
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const fence = line.match(/^\s*(```|~~~)/);
    if (fence) {
      flush();
      const body: string[] = [];
      for (i++; i < lines.length && !lines[i].trimStart().startsWith(fence[1]); i++) body.push(lines[i]);
      out.push({ kind: 'code', text: body.join('\n') });
      continue;
    }
    let m;
    if (!line.trim()) flush();
    else if ((m = line.match(/^(#{1,6})\s+(.*)$/))) {
      flush();
      out.push({ kind: 'heading', level: m[1].length, text: m[2] });
    } else if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) {
      flush();
      out.push({ kind: 'rule' });
    } else if ((m = line.match(/^(\s*)([-*+]|\d+[.)])\s+(.*)$/))) {
      flush();
      const mark = /\d/.test(m[2]) ? m[2].replace(')', '.') : '•';
      out.push({ kind: 'item', mark, depth: Math.floor(m[1].length / 2), text: m[3] });
    } else if ((m = line.match(/^>\s?(.*)$/))) {
      flush();
      const last = out.at(-1);
      if (last?.kind === 'quote') last.text += '\n' + m[1];
      else out.push({ kind: 'quote', text: m[1] });
    } else para.push(line);
  }
  flush();
  return out;
}

