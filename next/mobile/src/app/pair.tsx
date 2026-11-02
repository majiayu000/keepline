// Pairing: scan the QR code the computer shows, or paste its link. Scanning it with the
// phone's own camera opens this screen too, since the app owns the `keepline://` scheme.
import { CameraView, useCameraPermissions } from 'expo-camera';
import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { Pressable, StyleSheet, Text, TextInput, View } from 'react-native';
import { pair } from '@/remote/hub';
import { escape, parseLink } from '@/remote/link';
import { useTheme } from '@/ui/theme';

export default function Pair() {
  const t = useTheme();
  const params = useLocalSearchParams<Record<string, string>>();
  const [permission, requestPermission] = useCameraPermissions();
  const [text, setText] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const done = useRef(false);

  async function use(linkText: string) {
    if (done.current || busy) return;
    let link;
    try {
      link = parseLink(linkText);
    } catch (e) {
      setError(`这不是 Keepline 的配对链接（${(e as Error).message}）`);
      return;
    }
    done.current = true;
    setBusy(true);
    try {
      await pair(link);
      router.dismissTo('/');
    } catch (e) {
      done.current = false;
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  // Opened from keepline://pair?…: the link arrives as this route's params.
  useEffect(() => {
    if (params.code && params.key) {
      const query = Object.entries(params)
        .map(([k, v]) => `${k}=${escape(String(v))}`)
        .join('&');
      use(`keepline://pair?${query}`);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [params.code, params.key]);

  return (
    <View style={[s.page, { backgroundColor: t.bg }]}>
      <Text style={[s.hint, { color: t.dim }]}>
        在电脑的 Keepline 里点标题栏的手机按钮，选「配对新手机…」，把二维码对准下面的框。
      </Text>
      <View style={[s.camera, { borderColor: t.line }]}>
        {permission?.granted ? (
          <CameraView
            style={StyleSheet.absoluteFill}
            facing="back"
            barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
            onBarcodeScanned={busy ? undefined : (r) => use(r.data)}
          />
        ) : (
          <Pressable onPress={requestPermission} style={s.center}>
            <Text style={{ color: t.accent, fontSize: 16 }}>允许使用相机来扫码</Text>
          </Pressable>
        )}
      </View>
      <Text style={[s.hint, { color: t.dim }]}>或者粘贴电脑上「复制链接」得到的链接：</Text>
      <TextInput
        value={text}
        onChangeText={(v) => {
          setText(v);
          setError('');
        }}
        placeholder="keepline://pair?…"
        placeholderTextColor={t.dim}
        autoCapitalize="none"
        autoCorrect={false}
        style={[s.input, { color: t.text, borderColor: t.line, backgroundColor: t.card }]}
      />
      <Pressable
        disabled={!text || busy}
        onPress={() => use(text)}
        style={[s.button, { backgroundColor: t.accent, opacity: !text || busy ? 0.5 : 1 }]}
      >
        <Text style={{ color: t.onAccent, fontSize: 16 }}>{busy ? '正在连接…' : '配对'}</Text>
      </Pressable>
      {!!error && <Text style={{ color: t.danger }}>{error}</Text>}
    </View>
  );
}

const s = StyleSheet.create({
  page: { flex: 1, padding: 16, gap: 12 },
  hint: { fontSize: 14 },
  camera: {
    aspectRatio: 1,
    borderRadius: 16,
    overflow: 'hidden',
    borderWidth: StyleSheet.hairlineWidth,
  },
  center: { flex: 1, alignItems: 'center', justifyContent: 'center' },
  input: { borderWidth: 1, borderRadius: 10, padding: 12, fontSize: 14 },
  button: { alignItems: 'center', paddingVertical: 12, borderRadius: 10 },
});
