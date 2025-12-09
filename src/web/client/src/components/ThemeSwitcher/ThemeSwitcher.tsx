import { useState } from 'react'
import { useTheme, Theme, THEMES } from '@/contexts/ThemeContext'
import styles from './ThemeSwitcher.module.css'

const THEME_COLORS: Record<Theme, [string, string, string]> = {
  cyberpunk: ['#00fff9', '#ff00ff', '#0a0a0f'],
  matrix: ['#00ff41', '#008f11', '#0d0208'],
  synthwave: ['#ff6ad5', '#ffd319', '#1a1025'],
  minimal: ['#2563eb', '#10b981', '#ffffff'],
  tokyo: ['#7aa2f7', '#bb9af7', '#1a1b26'],
}

export function ThemeSwitcher() {
  const { theme, setTheme } = useTheme()
  const [isOpen, setIsOpen] = useState(false)

  return (
    <div className={styles.container}>
      <button
        className={styles.button}
        onClick={() => setIsOpen(!isOpen)}
        aria-label="Change theme"
      >
        <span>🎨</span>
        <span>{THEMES[theme]}</span>
        <span>▾</span>
      </button>

      {isOpen && (
        <div className={styles.dropdown}>
          {(Object.keys(THEMES) as Theme[]).map((t) => (
            <button
              key={t}
              className={`${styles.option} ${t === theme ? styles.active : ''}`}
              onClick={() => {
                setTheme(t)
                setIsOpen(false)
              }}
            >
              <div className={styles.preview}>
                {THEME_COLORS[t].map((color, i) => (
                  <span
                    key={i}
                    className={styles.previewColor}
                    style={{
                      background: color,
                      border: t === 'minimal' && i === 2 ? '1px solid #e2e8f0' : 'none',
                    }}
                  />
                ))}
              </div>
              <span>{THEMES[t]}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
