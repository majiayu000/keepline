// Every paired machine and its conversations, the ones waiting on you first.
import { Link, Stack, router } from 'expo-router';
import { Alert, Platform, Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import { type Card, type Machine, forget, reconnect, useHub } from '@/remote/hub';
import { cardTitle } from '@/ui/format';
import { type Theme, useTheme } from '@/ui/theme';

/** Removes a machine from this phone after asking; the computer keeps its own pairing list. */
function askForget(m: Machine) {
  const title = `从手机上移除「${m.name || '电脑'}」？`;
  const detail = '要再连这台电脑得重新扫码配对。电脑那边的配对记录不受影响。';
  if (Platform.OS === 'web') {
    if (globalThis.confirm?.(`${title}\n${detail}`)) forget(m.id);
    return;
  }
  Alert.alert(title, detail, [
    { text: '取消', style: 'cancel' },
    { text: '移除', style: 'destructive', onPress: () => forget(m.id) },
  ]);
}

const rank = (c: Card) => (c.approval || c.phase === 'waiting' ? 0 : c.phase === 'working' ? 1 : 2);


function status(c: Card): string {
  if (c.approval) return '等你审批';
  if (c.phase === 'waiting') return '等你回答';
  if (c.phase === 'working') return '干活中';
  return '空闲';
}

function Row({ m, c, t }: { m: Machine; c: Card; t: Theme }) {
  const waiting = rank(c) === 0;
  return (
    <Pressable
      onPress={() => router.push({ pathname: '/chat', params: { machine: m.id, terminal: c.id } })}
      style={({ pressed }) => [
        s.row,
        { backgroundColor: waiting ? t.waitingBg : t.card, borderColor: t.line },
        pressed && { opacity: 0.7 },
      ]}
    >
      <View style={s.rowTop}>
        <Text style={[s.title, { color: t.text }]} numberOfLines={1}>
          {cardTitle(c)}
        </Text>
        <Text
          style={[
            s.badge,
            { color: waiting ? t.waiting : c.phase === 'working' ? t.running : t.dim },
          ]}
        >
          {status(c)}
        </Text>
      </View>
      <Text style={[s.meta, { color: t.dim }]} numberOfLines={1}>
        {[c.agent, m.spaces[c.space] ?? c.space, c.cwd].filter(Boolean).join(' · ')}
      </Text>
      {(c.approval || c.waiting) && (
        <Text style={[s.ask, { color: t.text }]} numberOfLines={3}>
          {c.approval || c.waiting}
        </Text>
      )}
    </Pressable>
  );
}

function MachineSection({ m, t }: { m: Machine; t: Theme }) {
  const cards = m.cards.filter((c) => c.agent).sort((a, b) => rank(a) - rank(b));
  const shells = m.cards.length - cards.length;
  return (
    <View style={s.section}>
      <View style={s.sectionHead}>
        <View
          style={[
            s.dot,
            { backgroundColor: m.status === 'online' ? t.ok : m.status === 'refused' ? t.danger : t.dim },
          ]}
        />
        <Text style={[s.machine, { color: t.text }]}>{m.name || '电脑'}</Text>
        {m.role && m.role !== 'full' && (
          <Text style={[s.meta, { color: t.dim }]}>{m.role === 'view' ? '只看' : '能答审批'}</Text>
        )}
      </View>
      {m.status !== 'online' && (
        <View style={s.offline}>
          <Pressable style={{ flex: 1 }} onPress={() => reconnect(m.id)}>
            <Text style={[s.meta, { color: m.status === 'refused' ? t.danger : t.dim }]}>
              {m.status === 'connecting' ? '正在连接…' : `${m.error ?? '没连上'}（点这里重试）`}
            </Text>
          </Pressable>
          {m.status !== 'connecting' && (
            <Pressable hitSlop={8} onPress={() => askForget(m)}>
              <Text style={[s.meta, { color: t.accent }]}>移除</Text>
            </Pressable>
          )}
        </View>
      )}
      {cards.map((c) => (
        <Row key={c.id} m={m} c={c} t={t} />
      ))}
      {m.status === 'online' && cards.length === 0 && (
        <Text style={[s.meta, { color: t.dim }]}>没有正在运行的 Agent 对话</Text>
      )}
      {shells > 0 && <Text style={[s.meta, { color: t.dim }]}>另有 {shells} 个普通终端</Text>}
    </View>
  );
}

export default function Sessions() {
  const t = useTheme();
  const { loaded, machines } = useHub();
  return (
    <>
      <Stack.Screen
        options={{
          headerRight: () => (
            <Link href="/pair" style={{ color: t.accent, fontSize: 16, paddingHorizontal: 12 }}>
              配对
            </Link>
          ),
        }}
      />
      <ScrollView contentContainerStyle={s.page}>
        {loaded && machines.length === 0 && (
          <View style={s.empty}>
            <Text style={[s.emptyTitle, { color: t.text }]}>还没有配对电脑</Text>
            <Text style={[s.meta, { color: t.dim, textAlign: 'center' }]}>
              在电脑的 Keepline 里点标题栏的手机按钮，选「配对新手机…」，再用这里扫码。
            </Text>
            <Pressable
              onPress={() => router.push('/pair')}
              style={[s.button, { backgroundColor: t.accent }]}
            >
              <Text style={{ color: t.onAccent, fontSize: 16 }}>扫码配对</Text>
            </Pressable>
          </View>
        )}
        {machines.map((m) => (
          <MachineSection key={m.id} m={m} t={t} />
        ))}
      </ScrollView>
    </>
  );
}

const s = StyleSheet.create({
  page: { padding: 16, gap: 20 },
  section: { gap: 8 },
  sectionHead: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  dot: { width: 8, height: 8, borderRadius: 4 },
  machine: { fontSize: 15, fontWeight: '600' },
  row: { borderWidth: StyleSheet.hairlineWidth, borderRadius: 12, padding: 12, gap: 4 },
  rowTop: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  title: { flex: 1, fontSize: 16, fontWeight: '500' },
  badge: { fontSize: 13 },
  meta: { fontSize: 13 },
  offline: { flexDirection: 'row', alignItems: 'flex-start', gap: 12 },
  ask: { fontSize: 14, marginTop: 4 },
  empty: { alignItems: 'center', gap: 12, paddingTop: 80, paddingHorizontal: 24 },
  emptyTitle: { fontSize: 18, fontWeight: '600' },
  button: { paddingHorizontal: 20, paddingVertical: 12, borderRadius: 10, marginTop: 8 },
});
