// One conversation as chat: what you asked and what the Agent said, newest at the bottom,
// a box to say something, and the approval or question it is waiting on.
import { Stack, useLocalSearchParams } from 'expo-router';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ActivityIndicator,
  FlatList,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { refresh, remote, useHub } from '@/remote/hub';
import { cardTitle } from '@/ui/format';
import { type Theme, ago, useTheme } from '@/ui/theme';

type Message = { seq: number; at: number; role: 'user' | 'agent'; text: string };
type Pending = { text: string; at: number };

/** A reindexed log gives its messages new seqs; the same words at the same time are one. */
const key = (m: Message) => `${m.at}|${m.role}|${m.text}`;

function merge(have: Message[], got: Message[]): Message[] {
  const seen = new Set(have.map(key));
  const fresh = got.filter((m) => !seen.has(key(m)));
  return fresh.length ? [...have, ...fresh].sort((a, b) => a.seq - b.seq) : have;
}

const POLL_MS = 2000;

function Bubble({ m, t }: { m: Message | (Pending & { role: 'user'; pending: true }); t: Theme }) {
  const user = m.role === 'user';
  return (
    <View style={[s.bubbleRow, { justifyContent: user ? 'flex-end' : 'flex-start' }]}>
      <View
        style={[
          s.bubble,
          user
            ? { backgroundColor: t.userBubble, opacity: 'pending' in m ? 0.6 : 1 }
            : { backgroundColor: t.agentBubble, borderColor: t.line, borderWidth: StyleSheet.hairlineWidth },
        ]}
      >
        <Text selectable style={{ color: user ? t.onAccent : t.text, fontSize: 15, lineHeight: 21 }}>
          {m.text}
        </Text>
        <Text style={{ color: user ? t.onAccent : t.dim, opacity: 0.7, fontSize: 11, marginTop: 4 }}>
          {'pending' in m ? '发送中…' : ago(m.at)}
        </Text>
      </View>
    </View>
  );
}

export default function Chat() {
  const t = useTheme();
  const insets = useSafeAreaInsets();
  const { machine, terminal } = useLocalSearchParams<{ machine: string; terminal: string }>();
  const { machines } = useHub();
  const m = machines.find((x) => x.id === machine);
  const card = m?.cards.find((c) => c.id === terminal);
  const online = m?.status === 'online';
  const can = (op: string) => !!m?.ops.includes(op);

  const [messages, setMessages] = useState<Message[]>([]);
  const [session, setSession] = useState<string | null>(null);
  const [more, setMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [draft, setDraft] = useState('');
  const [pending, setPending] = useState<Pending[]>([]);
  const last = useRef(0);
  last.current = messages.at(-1)?.seq ?? 0;

  // The newest page; again whenever the connection comes back.
  useEffect(() => {
    const r = online ? remote(machine) : null;
    if (!r) return;
    let live = true;
    r.request('chat', { terminal, limit: 50 })
      .then((page) => {
        if (!live) return;
        setSession(page.session);
        setMore(page.more);
        setMessages((have) => merge(have, page.messages));
        setError('');
      })
      .catch((e) => live && setError(e.message))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [online, machine, terminal]);

  // New messages: the Agent's words mid-turn raise no event, so ask every two seconds.
  useEffect(() => {
    if (!session || !online) return;
    const timer = setInterval(async () => {
      const r = remote(machine);
      if (!r) return;
      try {
        const page = await r.request('chat', { session, after: last.current });
        if (page.messages.length) setMessages((have) => merge(have, page.messages));
      } catch {
        // The next tick or the reconnect tries again.
      }
    }, POLL_MS);
    return () => clearInterval(timer);
  }, [session, online, machine]);

  // A sent message shows until the log has it, or for half a minute.
  useEffect(() => {
    setPending((p) =>
      p.filter(
        (x) =>
          Date.now() / 1000 - x.at < 30 &&
          !messages.some((m) => m.role === 'user' && m.at >= x.at - 5 && m.text.trim() === x.text.trim()),
      ),
    );
  }, [messages]);

  const older = useCallback(async () => {
    const r = remote(machine);
    if (!r || !session || !more || !messages.length) return;
    const page = await r.request('chat', { session, before: messages[0].seq });
    setMore(page.more);
    setMessages((have) => merge(have, page.messages));
  }, [machine, session, more, messages]);

  async function send() {
    const text = draft.trim();
    const r = remote(machine);
    if (!text || !r) return;
    setDraft('');
    setPending((p) => [...p, { text, at: Date.now() / 1000 }]);
    try {
      await r.request('send', { terminal, text, enter: true });
    } catch (e) {
      setPending((p) => p.filter((x) => x.text !== text));
      setDraft(text);
      setError(`没发出去：${(e as Error).message}`);
    }
  }

  async function act(op: string, fields: Record<string, unknown>) {
    const r = remote(machine);
    if (!r) return;
    try {
      await r.request(op, { terminal, ...fields });
      setError('');
      setTimeout(() => refresh(machine), 300);
    } catch (e) {
      setError((e as Error).message);
    }
  }

  // Inverted list: index 0 sits at the bottom.
  const rows = useMemo(
    () => [
      ...pending.map((p) => ({ ...p, role: 'user' as const, pending: true as const })).reverse(),
      ...[...messages].reverse(),
    ],
    [messages, pending],
  );

  const ask = card?.approval || (card?.phase === 'waiting' ? card.waiting : null);

  // What the approval is for (the command, the file) is on screen above the question.
  const [screen, setScreen] = useState('');
  useEffect(() => {
    const r = ask && online ? remote(machine) : null;
    if (!r) return setScreen('');
    let live = true;
    r.request('read', { terminal, lines: 14 })
      .then((x) => live && setScreen(x.text.replace(/\s+$/, '')))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [ask, online, machine, terminal]);

  return (
    <KeyboardAvoidingView
      style={{ flex: 1, backgroundColor: t.bg }}
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      keyboardVerticalOffset={Platform.OS === 'ios' ? 90 : 0}
    >
      <Stack.Screen options={{ title: card ? cardTitle(card) : '对话' }} />
      {!online && (
        <Text style={[s.notice, { color: t.dim }]}>
          {m?.status === 'connecting' ? '正在连接…' : (m?.error ?? '电脑没连上')}
        </Text>
      )}
      {loading && online ? (
        <ActivityIndicator style={{ marginTop: 40 }} />
      ) : (
        <FlatList
          inverted
          data={rows}
          keyExtractor={(m) => ('pending' in m ? `p${m.at}${m.text}` : String(m.seq))}
          renderItem={({ item }) => <Bubble m={item} t={t} />}
          onEndReached={older}
          onEndReachedThreshold={0.3}
          contentContainerStyle={{ padding: 12, gap: 8 }}
          ListEmptyComponent={
            <Text style={[s.notice, { color: t.dim, transform: [{ scaleY: -1 }] }]}>
              {card?.agent ? '这段对话还没有消息' : '这个终端里没有 Agent 对话'}
            </Text>
          }
        />
      )}
      {!!ask && (
        <View style={[s.ask, { backgroundColor: t.waitingBg, borderColor: t.line }]}>
          <Text style={{ color: t.waiting, fontSize: 13, fontWeight: '600' }}>
            {card?.approval ? '等你审批' : '等你回答'}
          </Text>
          {!!screen && (
            <Text selectable style={[s.screen, { color: t.text, backgroundColor: t.card, borderColor: t.line }]} numberOfLines={14}>
              {screen}
            </Text>
          )}
          {!screen && (
            <Text style={{ color: t.text, fontSize: 14 }} numberOfLines={6}>
              {ask}
            </Text>
          )}
          <View style={s.actions}>
            {card?.approval && can('answer') && (
              <>
                <Chip t={t} label="允许一次" primary onPress={() => act('answer', { question: card.approval, allow: true })} />
                <Chip t={t} label="拒绝" onPress={() => act('answer', { question: card.approval, allow: false })} />
              </>
            )}
            {!card?.approval && can('keys') &&
              ['1', '2', '3'].map((k) => (
                <Chip key={k} t={t} label={`选 ${k}`} onPress={() => act('keys', { keys: [k] })} />
              ))}
            {can('keys') && <Chip t={t} label="Esc" onPress={() => act('keys', { keys: ['esc'] })} />}
          </View>
        </View>
      )}
      {!!error && <Text style={[s.notice, { color: t.danger }]}>{error}</Text>}
      {can('send') ? (
        <View
          style={[
            s.composer,
            { borderColor: t.line, backgroundColor: t.card, paddingBottom: 8 + insets.bottom },
          ]}
        >
          <TextInput
            value={draft}
            onChangeText={setDraft}
            placeholder={online ? '说点什么…' : '电脑没连上'}
            placeholderTextColor={t.dim}
            editable={online}
            multiline
            style={[s.input, { color: t.text, backgroundColor: t.bg }]}
          />
          <Pressable
            disabled={!draft.trim() || !online}
            onPress={send}
            style={[s.send, { backgroundColor: t.accent, opacity: !draft.trim() || !online ? 0.4 : 1 }]}
          >
            <Text style={{ color: t.onAccent, fontSize: 15 }}>发送</Text>
          </Pressable>
        </View>
      ) : (
        m?.role && (
          <Text style={[s.notice, { color: t.dim, paddingBottom: 8 + insets.bottom }]}>
            这台手机的权限是「{m.role === 'view' ? '只看' : '能答审批'}」，不能发消息
          </Text>
        )
      )}
    </KeyboardAvoidingView>
  );
}

function Chip({ t, label, onPress, primary }: { t: Theme; label: string; onPress: () => void; primary?: boolean }) {
  return (
    <Pressable
      onPress={onPress}
      style={({ pressed }) => [
        s.chip,
        primary ? { backgroundColor: t.accent } : { borderColor: t.line, borderWidth: 1, backgroundColor: t.card },
        pressed && { opacity: 0.7 },
      ]}
    >
      <Text style={{ color: primary ? t.onAccent : t.text, fontSize: 14 }}>{label}</Text>
    </Pressable>
  );
}

const s = StyleSheet.create({
  notice: { textAlign: 'center', fontSize: 13, padding: 8 },
  bubbleRow: { flexDirection: 'row' },
  bubble: { maxWidth: '85%', borderRadius: 14, paddingHorizontal: 12, paddingVertical: 8 },
  ask: { margin: 8, padding: 12, borderRadius: 12, gap: 6, borderWidth: StyleSheet.hairlineWidth },
  screen: {
    fontFamily: Platform.select({ ios: 'Menlo', android: 'monospace', default: 'monospace' }),
    fontSize: 12,
    lineHeight: 16,
    padding: 8,
    borderRadius: 8,
    borderWidth: StyleSheet.hairlineWidth,
  },
  actions: { flexDirection: 'row', flexWrap: 'wrap', gap: 8, marginTop: 4 },
  chip: { paddingHorizontal: 14, paddingVertical: 8, borderRadius: 999 },
  composer: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    gap: 8,
    paddingHorizontal: 12,
    paddingTop: 8,
    borderTopWidth: StyleSheet.hairlineWidth,
  },
  input: { flex: 1, maxHeight: 140, borderRadius: 18, paddingHorizontal: 14, paddingVertical: 9, fontSize: 15 },
  send: { paddingHorizontal: 16, paddingVertical: 10, borderRadius: 18 },
});
