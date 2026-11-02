// The Markdown Agents write, enough of it for a chat bubble: headings, lists, quotes,
// fenced code, and inline code, bold, italics and links. No dependency; anything it does
// not know stays as the text it was.
import { Fragment, type ReactNode } from 'react';
import { Linking, Platform, StyleSheet, Text, View } from 'react-native';
import { blocks } from './blocks';
import type { Theme } from './theme';

const INLINE = /(`[^`\n]+`)|(\*\*[^*\n]+\*\*|__[^_\n]+__)|(\*[^*\s][^*\n]*\*|_[^_\s][^_\n]*_)|(\[[^\]\n]+\]\([^)\s]+\))/g;

function inline(text: string, t: Theme): ReactNode[] {
  const out: ReactNode[] = [];
  let at = 0;
  for (const m of text.matchAll(INLINE)) {
    if (m.index! > at) out.push(text.slice(at, m.index));
    const [whole, code, bold, em, link] = m;
    const k = out.length;
    if (code) out.push(<Text key={k} style={[s.code, { backgroundColor: t.bg }]}>{code.slice(1, -1)}</Text>);
    else if (bold) out.push(<Text key={k} style={{ fontWeight: '700' }}>{bold.slice(2, -2)}</Text>);
    else if (em) out.push(<Text key={k} style={{ fontStyle: 'italic' }}>{em.slice(1, -1)}</Text>);
    else if (link) {
      const [, label, url] = link.match(/^\[([^\]]+)\]\(([^)]+)\)$/)!;
      out.push(
        <Text key={k} style={{ color: t.accent }} onPress={() => /^https?:/.test(url) && Linking.openURL(url)}>
          {label}
        </Text>,
      );
    } else out.push(whole);
    at = m.index! + whole.length;
  }
  if (at < text.length) out.push(text.slice(at));
  return out;
}

export function Markdown({ text, t, color }: { text: string; t: Theme; color: string }) {
  const base = { color, fontSize: 15, lineHeight: 21 };
  return (
    <View style={{ gap: 6 }}>
      {blocks(text).map((b, i) => {
        switch (b.kind) {
          case 'code':
            return (
              <Text key={i} selectable style={[s.block, { color, backgroundColor: t.bg, borderColor: t.line }]}>
                {b.text}
              </Text>
            );
          case 'heading':
            return (
              <Text key={i} selectable style={[base, { fontWeight: '700', fontSize: b.level <= 2 ? 17 : 15 }]}>
                {inline(b.text, t)}
              </Text>
            );
          case 'item':
            return (
              <View key={i} style={{ flexDirection: 'row', paddingLeft: b.depth * 14 }}>
                <Text style={[base, { width: b.mark === '•' ? 14 : 22 }]}>{b.mark}</Text>
                <Text selectable style={[base, { flex: 1 }]}>
                  {inline(b.text, t)}
                </Text>
              </View>
            );
          case 'quote':
            return (
              <Text key={i} selectable style={[base, s.quote, { borderColor: t.line, color: t.dim }]}>
                {inline(b.text, t)}
              </Text>
            );
          case 'rule':
            return <View key={i} style={{ height: StyleSheet.hairlineWidth, backgroundColor: t.line }} />;
          default:
            return (
              <Text key={i} selectable style={base}>
                {inline(b.text, t).map((n, j) => (
                  <Fragment key={j}>{n}</Fragment>
                ))}
              </Text>
            );
        }
      })}
    </View>
  );
}

const mono = Platform.select({ ios: 'Menlo', android: 'monospace', default: 'monospace' });

const s = StyleSheet.create({
  code: { fontFamily: mono, fontSize: 13 },
  block: {
    fontFamily: mono,
    fontSize: 12,
    lineHeight: 17,
    padding: 8,
    borderRadius: 8,
    borderWidth: StyleSheet.hairlineWidth,
  },
  quote: { borderLeftWidth: 3, paddingLeft: 8 },
});
