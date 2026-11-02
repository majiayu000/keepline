import { Stack } from 'expo-router';
import { StatusBar } from 'expo-status-bar';
import { useEffect } from 'react';
import { start } from '@/remote/hub';
import { useTheme } from '@/ui/theme';

export default function Layout() {
  const t = useTheme();
  useEffect(() => {
    start();
  }, []);
  return (
    <>
      <StatusBar style="auto" />
      <Stack
        screenOptions={{
          headerStyle: { backgroundColor: t.bg },
          headerTintColor: t.text,
          headerShadowVisible: false,
          contentStyle: { backgroundColor: t.bg },
        }}
      >
        <Stack.Screen name="index" options={{ title: '会话' }} />
        <Stack.Screen name="pair" options={{ title: '配对电脑', presentation: 'modal' }} />
        <Stack.Screen name="chat" options={{ title: '' }} />
      </Stack>
    </>
  );
}
