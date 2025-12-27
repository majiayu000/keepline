import React from 'react';

export type ThemeName = 'light' | 'dark' | 'claude' | 'claude-dark' | 'minimal' | 'minimal-dark' | 'ocean';

interface Theme {
  id: ThemeName;
  name: string;
  icon: string;
}

const themes: Theme[] = [
  { id: 'light', name: 'Light', icon: '☀️' },
  { id: 'dark', name: 'Dark', icon: '🌙' },
  { id: 'claude', name: 'Claude', icon: '🧡' },
  { id: 'claude-dark', name: 'Claude Dark', icon: '🔥' },
  { id: 'minimal', name: 'Minimal', icon: '◯' },
  { id: 'minimal-dark', name: 'Minimal Dark', icon: '●' },
  { id: 'ocean', name: 'Ocean', icon: '🌊' },
];

interface ThemeSelectorProps {
  currentTheme: ThemeName;
  onThemeChange: (theme: ThemeName) => void;
}

export default function ThemeSelector({ currentTheme, onThemeChange }: ThemeSelectorProps) {
  return (
    <div className="theme-selector">
      {themes.map((theme) => (
        <button
          key={theme.id}
          className={`theme-btn ${currentTheme === theme.id ? 'active' : ''}`}
          data-theme={theme.id}
          onClick={() => onThemeChange(theme.id)}
          title={theme.name}
          aria-label={`Switch to ${theme.name} theme`}
        />
      ))}
    </div>
  );
}
