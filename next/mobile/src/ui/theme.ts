// Colors for light and dark, picked by the system setting.
import { useColorScheme } from 'react-native';

const light = {
  bg: '#f6f6f4',
  card: '#ffffff',
  text: '#1c1c1a',
  dim: '#6b6b66',
  line: '#e4e4df',
  accent: '#2f6fed',
  onAccent: '#ffffff',
  agentBubble: '#ffffff',
  userBubble: '#2f6fed',
  waiting: '#c2410c',
  waitingBg: '#fff4ec',
  working: '#15803d',
  danger: '#b91c1c',
};

const dark: typeof light = {
  bg: '#121212',
  card: '#1d1d1c',
  text: '#ececea',
  dim: '#9b9b95',
  line: '#2e2e2c',
  accent: '#5b8ff9',
  onAccent: '#ffffff',
  agentBubble: '#232322',
  userBubble: '#3b6fd9',
  waiting: '#fb923c',
  waitingBg: '#2a1d14',
  working: '#4ade80',
  danger: '#f87171',
};

export type Theme = typeof light;

export function useTheme(): Theme {
  return useColorScheme() === 'dark' ? dark : light;
}

/** "刚刚", "5 分钟前", "3 小时前", or the date. */
export function ago(at: number): string {
  const s = Date.now() / 1000 - at;
  if (s < 60) return '刚刚';
  if (s < 3600) return `${Math.floor(s / 60)} 分钟前`;
  if (s < 86400) return `${Math.floor(s / 3600)} 小时前`;
  const d = new Date(at * 1000);
  return `${d.getMonth() + 1}月${d.getDate()}日`;
}
