import { createContext, useContext, useState, useEffect, ReactNode } from 'react'

export type Theme = 'cyberpunk' | 'matrix' | 'synthwave' | 'minimal' | 'tokyo'

export const THEMES: Record<Theme, string> = {
  cyberpunk: 'Cyberpunk',
  matrix: 'Matrix',
  synthwave: 'Synthwave',
  minimal: 'Minimal',
  tokyo: 'Tokyo Night',
}

interface ThemeContextType {
  theme: Theme
  setTheme: (theme: Theme) => void
  themes: typeof THEMES
}

const ThemeContext = createContext<ThemeContextType | null>(null)

const STORAGE_KEY = 'keepline-theme'

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(() => {
    const saved = localStorage.getItem(STORAGE_KEY)
    return (saved as Theme) || 'cyberpunk'
  })

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme)
    localStorage.setItem(STORAGE_KEY, theme)
  }, [theme])

  const setTheme = (newTheme: Theme) => {
    setThemeState(newTheme)
  }

  return (
    <ThemeContext.Provider value={{ theme, setTheme, themes: THEMES }}>
      {children}
    </ThemeContext.Provider>
  )
}

export function useTheme() {
  const context = useContext(ThemeContext)
  if (!context) {
    throw new Error('useTheme must be used within a ThemeProvider')
  }
  return context
}
