import { useRef, useCallback, memo } from 'react'
import { useTheme, Theme, THEMES } from '@/contexts/ThemeContext'
import { useClickOutside, useToggle } from '@/hooks'
import styles from './ThemeSwitcher.module.css'

// Theme preview colors - matches CSS variables in index.css
const THEME_COLORS: Record<Theme, [string, string, string]> = {
  cyberpunk: ['#00fff9', '#ff00ff', '#0a0a0f'],
  matrix: ['#00ff41', '#008f11', '#0d0208'],
  synthwave: ['#ff6ad5', '#ffd319', '#1a1025'],
  minimal: ['#2563eb', '#10b981', '#ffffff'],
  tokyo: ['#7aa2f7', '#bb9af7', '#1a1b26'],
}

export const ThemeSwitcher = memo(function ThemeSwitcher() {
  const { theme, setTheme } = useTheme()
  const [isOpen, toggle, , close] = useToggle(false)
  const containerRef = useRef<HTMLDivElement>(null)

  useClickOutside(containerRef, close, isOpen)

  const handleSelect = useCallback((t: Theme) => {
    setTheme(t)
    close()
  }, [setTheme, close])

  return (
    <div className={styles.container} ref={containerRef}>
      <button
        className={styles.button}
        onClick={toggle}
        aria-label="Change theme"
        aria-expanded={isOpen}
        aria-haspopup="listbox"
      >
        <span>🎨</span>
        <span>{THEMES[theme]}</span>
        <span aria-hidden="true">▾</span>
      </button>

      {isOpen && (
        <div
          className={styles.dropdown}
          role="listbox"
          aria-label="Theme options"
        >
          {(Object.keys(THEMES) as Theme[]).map((t) => (
            <button
              key={t}
              className={`${styles.option} ${t === theme ? styles.active : ''}`}
              onClick={() => handleSelect(t)}
              role="option"
              aria-selected={t === theme}
            >
              <div className={styles.preview} aria-hidden="true">
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
})
