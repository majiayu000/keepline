import { memo } from 'react'
import styles from './TabNav.module.css'

export type TabId = 'sessions' | 'work' | 'analytics' | 'projects' | 'memory' | 'plans' | 'terminal'

interface Tab {
  id: TabId
  label: string
  icon: string
}

const TABS: Tab[] = [
  { id: 'sessions', label: 'Sessions', icon: '>' },
  { id: 'work', label: 'Work', icon: '+' },
  { id: 'analytics', label: 'Analytics', icon: '$' },
  { id: 'projects', label: 'Projects', icon: '#' },
  { id: 'memory', label: 'Memory', icon: '@' },
  { id: 'plans', label: 'Plans', icon: '%' },
  { id: 'terminal', label: 'Terminal', icon: '~' },
]

interface TabNavProps {
  activeTab: TabId
  onTabChange: (tab: TabId) => void
}

export const TabNav = memo(function TabNav({ activeTab, onTabChange }: TabNavProps) {
  return (
    <nav className={styles.nav} role="tablist" aria-label="Main navigation">
      {TABS.map((tab) => (
        <button
          key={tab.id}
          className={`${styles.tab} ${activeTab === tab.id ? styles.active : ''}`}
          onClick={() => onTabChange(tab.id)}
          role="tab"
          aria-selected={activeTab === tab.id}
          aria-controls={`panel-${tab.id}`}
        >
          <span className={styles.icon}>{tab.icon}</span>
          <span className={styles.label}>{tab.label}</span>
        </button>
      ))}
    </nav>
  )
})
